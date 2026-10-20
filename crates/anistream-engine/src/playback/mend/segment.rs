//! Where a segment's media starts: the search for a container signature at the head of
//! what the host served, past whatever it was dressed in, and the structural checks that
//! keep a stray byte of the disguise from passing for the start of the stream.

/// How much of a segment's head to search for a container signature.
///
/// The wrapper is a decoy meant to be cheap for the host to prepend, so it is small — the
/// observed PNG is 70 bytes. 64 KiB is far beyond any plausible prefix while keeping the
/// search bounded.
const SNIFF_LIMIT: usize = 64 * 1024;

/// MPEG-TS packets are exactly this long, and each begins with `0x47`.
pub(super) const TS_PACKET: usize = 188;

/// Consecutive TS packets required before an offset is believed.
///
/// One `0x47` is a coincidence — it is a perfectly ordinary byte. Four in a row at exactly
/// 188-byte spacing is a transport stream.
const TS_CONFIRMATIONS: usize = 4;

/// Where the real payload starts in `data`, if it does not start at the beginning.
///
/// Returns `0` for anything already valid or simply unrecognised, which makes the caller's
/// slice a no-op — "pass it through" is the safe default in both cases.
///
/// **Not what `infer` or `file-format` do.** Those identify a buffer by its leading magic
/// bytes, and here the leading bytes are a *genuine* PNG — they would answer "this is an
/// image", correctly and uselessly. The question is where the media begins, which needs a
/// search rather than a lookup, and structural confirmation rather than a signature: a lone
/// `0x47` is the letter G, four of them 188 bytes apart is a transport stream.
pub fn payload_start(data: &[u8]) -> usize {
    // Text formats are recognised up front so a subtitle track is never mistaken for a
    // container with a prefix on it.
    if data.starts_with(b"WEBVTT") || data.starts_with(b"#EXTM3U") {
        return 0;
    }

    let limit = data.len().min(SNIFF_LIMIT);
    // Two passes, earliest match within each. The first insists a transport stream open the way
    // a segment does, which is what stops a stray sync byte inside the wrapper from claiming an
    // offset 188 bytes early. The second accepts any structurally valid packet, so a source that
    // starts mid-stream is still mended rather than refused.
    for offset in 0..limit {
        if is_iso_bmff(data, offset) || is_transport_stream(data, offset, true) {
            return offset;
        }
    }
    for offset in 0..limit {
        if is_transport_stream(data, offset, false) {
            return offset;
        }
    }
    0
}

/// Whether a transport stream starts at `offset`: `0x47` every 188 bytes, several times over.
fn is_transport_stream(data: &[u8], offset: usize, strict: bool) -> bool {
    if data.get(offset) != Some(&0x47) {
        return false;
    }
    // The tail of a segment is not a place to start looking: a lone `0x47` near the end could
    // not be confirmed, and confirming is the whole point.
    let cadence = (1..=TS_CONFIRMATIONS).all(|packet| {
        let at = offset + packet * TS_PACKET;
        // Missing bytes count as confirmation only when the data genuinely ended, so a short
        // final segment still validates.
        at >= data.len() || data.get(at) == Some(&0x47)
    }) && offset + TS_PACKET < data.len();
    cadence && is_packet_header(data, offset, strict)
}

/// Whether the four bytes at `offset` are a plausible TS packet header.
///
/// **The cadence alone cannot be trusted, and the arithmetic says why.** A wrapper of length `w`
/// puts the real payload at `w`; a stray `0x47` at `w - 188` then has *genuine* packet
/// boundaries at every position the cadence check probes, so it passes on the strength of the
/// real stream that follows it. One byte in 256 — a coin flip across a full episode — and the
/// result is a segment served with 188 bytes of wrapper still attached, which a decoder
/// recovers from by dropping whatever it cannot parse. That is a cut and a slip in sync, not a
/// clean failure, so it has to be excluded structurally rather than noticed.
///
/// `strict` additionally demands the packet look like the *first* of a segment: HLS transport
/// streams open on a PAT (PID 0) or the SDT beside it, with the payload-unit-start flag set.
/// Wrapper bytes clear all four tests about once in 65,000 rather than once in 256.
fn is_packet_header(data: &[u8], offset: usize, strict: bool) -> bool {
    let Some(header) = data.get(offset..offset + 4) else {
        return false;
    };
    // A set transport-error bit means the sender itself marked the packet corrupt, and an
    // adaptation-field control of zero is reserved — neither belongs at a payload's start.
    if header[1] & 0x80 != 0 || (header[3] >> 4) & 0b11 == 0 {
        return false;
    }
    if !strict {
        return true;
    }
    let pid = (u16::from(header[1] & 0x1F) << 8) | u16::from(header[2]);
    header[1] & 0x40 != 0 && pid <= 0x1F
}

/// Whether an ISO base media file box starts at `offset`.
///
/// fMP4 segments begin with one of a small set of box types. The size field is checked for
/// plausibility as well, because four ASCII letters alone appear inside compressed video often
/// enough to matter.
fn is_iso_bmff(data: &[u8], offset: usize) -> bool {
    let Some(header) = data.get(offset..offset + 8) else {
        return false;
    };
    let kind = &header[4..8];
    let known: [&[u8]; 6] = [b"ftyp", b"styp", b"moof", b"sidx", b"moov", b"emsg"];
    if !known.contains(&kind) {
        return false;
    }
    let size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
    // `1` means the real size is a following 64-bit field; `0` means "to end of file". Both
    // are legal, and anything else must at least be a header's worth.
    size == 0 || size == 1 || size >= 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::mend::tests::transport_stream;

    /// The observed disguise: a complete 1×1 PNG, then the real video.
    fn png_wrapped(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend(b"\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89");
        data.extend(b"\0\0\0\rIDATx\xdacd\xf8\xcfP\x0f\0\x03\x86\x01\x80Z4}k");
        data.extend(b"\0\0\0\0IEND\xaeB`\x82");
        data.extend(payload);
        data
    }

    #[test]
    fn a_healthy_transport_stream_is_left_alone() {
        // The property that makes routing every stream through this safe: an untouched
        // segment passes through byte-for-byte.
        assert_eq!(payload_start(&transport_stream()), 0);
    }

    #[test]
    fn video_hidden_behind_an_image_header_is_found() {
        // The case this exists for. The prefix is a genuine, complete PNG — so "is it a PNG"
        // is the wrong question, and only looking for the video answers it.
        let wrapped = png_wrapped(&transport_stream());
        let start = payload_start(&wrapped);
        assert_ne!(start, 0, "the disguise was not seen through");
        assert_eq!(&wrapped[start..], &transport_stream()[..]);
    }

    #[test]
    fn an_fmp4_segment_is_recognised_wrapped_or_not() {
        let mut fmp4 = Vec::new();
        fmp4.extend(&[0, 0, 0, 0x18]);
        fmp4.extend(b"ftypiso5");
        fmp4.extend(std::iter::repeat_n(0u8, 16));
        assert_eq!(payload_start(&fmp4), 0);

        let wrapped = png_wrapped(&fmp4);
        assert_eq!(&wrapped[payload_start(&wrapped)..], &fmp4[..]);
    }

    #[test]
    fn a_sync_byte_one_packet_before_the_payload_does_not_steal_the_offset() {
        // The failure this exists for, and it is not hypothetical: a wrapper of length `w` puts
        // a stray `0x47` at `w - 188` exactly 188 bytes from the real start, so every boundary
        // the cadence check probes is a *genuine* packet. One byte in 256 — near certain across
        // an episode's worth of segments — and the reward for believing it is 188 bytes of
        // wrapper left on the front: a cut and a slip in sync rather than an honest failure.
        // 252 bytes is the wrapper length observed in the wild, and the reason the decoy lands
        // at 64: 252 - 188. The decoy is deliberately a *plausible* packet header — error bit
        // clear, adaptation field valid — so only "does this open a segment" separates it from
        // the real thing.
        const WRAPPER: usize = 252;
        let payload = transport_stream();
        let mut wrapped = vec![0u8; WRAPPER];
        wrapped[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        wrapped[WRAPPER - TS_PACKET..][..4].copy_from_slice(&[0x47, 0x01, 0x64, 0x10]);
        wrapped.extend_from_slice(&payload);

        assert_eq!(
            payload_start(&wrapped),
            WRAPPER,
            "a decoy sync byte took the offset one packet early"
        );
        assert_eq!(&wrapped[payload_start(&wrapped)..], &payload[..]);
    }

    #[test]
    fn a_stream_that_starts_mid_flow_is_still_mended() {
        // The strict pass wants a segment opening — PAT, payload-unit-start. A source whose
        // segments begin mid-stream has neither, and must still be served rather than refused.
        let mut payload = Vec::new();
        for _ in 0..6 {
            payload.extend([0x47, 0x01, 0x64, 0x10]);
            payload.extend(std::iter::repeat_n(0xCD, TS_PACKET - 4));
        }
        let wrapped = png_wrapped(&payload);
        assert_eq!(&wrapped[payload_start(&wrapped)..], &payload[..]);
    }

    #[test]
    fn a_lone_sync_byte_is_not_mistaken_for_a_stream() {
        // 0x47 is the letter G and an ordinary byte in compressed video; one occurrence must
        // never truncate a segment. Only the 188-byte cadence is believed.
        let mut data = vec![0u8; 2000];
        data[5] = 0x47;
        data[100] = 0x47;
        assert_eq!(payload_start(&data), 0, "a stray 0x47 must not be treated as a payload");
    }

    #[test]
    fn unrecognised_bytes_pass_through_untouched() {
        // Refusing to serve what we merely do not recognise would break more than it fixes;
        // the player is the better judge.
        let noise: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        assert_eq!(payload_start(&noise), 0);
    }

    #[test]
    fn subtitles_and_playlists_are_never_sliced() {
        assert_eq!(payload_start(b"WEBVTT\n\n00:00.000 --> 00:02.000\nhello\n"), 0);
        assert_eq!(payload_start(b"#EXTM3U\n#EXT-X-VERSION:3\n"), 0);
    }
}
