//! The capability clips: two seconds of a grey frame, video only, in each format a TV may have no
//! decoder for. A TV that can't decode the picture of a file with sound plays the sound alone and
//! reports nothing wrong, as the Nest Hub does with HEVC, so a file in such a format is preceded,
//! once per TV, by the clip: a refusal says the TV can't, a moving position says it can. Made by
//! `clips/make.sh`.

use crate::formats::{VideoFormat, height_class};

/// One clip: its name, its content type and its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    pub name: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

macro_rules! clip {
    ($name:literal, $type:literal) => {
        Clip {
            name: $name,
            content_type: $type,
            bytes: include_bytes!(concat!("../clips/", $name)),
        }
    };
}

/// The clip that asks a TV about `video` at `height`: the 1080-line clip up to 1080 lines, and
/// the 2160-line clip above. H.264 needs one only above 1080 lines.
pub fn clip(video: &VideoFormat, height: Option<u32>) -> Option<Clip> {
    let tall = height_class(height) > 1080;
    Some(match (video, tall) {
        (VideoFormat::H264, false) => return None,
        (VideoFormat::H264, true) => clip!("h264-2160.mp4", "video/mp4"),
        (VideoFormat::Hevc, false) => clip!("hevc-1080.mp4", "video/mp4"),
        (VideoFormat::Hevc, true) => clip!("hevc-2160.mp4", "video/mp4"),
        (VideoFormat::HevcMain10, false) => clip!("hevc-main10-1080.mp4", "video/mp4"),
        (VideoFormat::HevcMain10, true) => clip!("hevc-main10-2160.mp4", "video/mp4"),
        (VideoFormat::Vp9, false) => clip!("vp9-1080.webm", "video/webm"),
        (VideoFormat::Vp9, true) => clip!("vp9-2160.webm", "video/webm"),
        (VideoFormat::Av1, false) => clip!("av1-1080.mp4", "video/mp4"),
        (VideoFormat::Av1, true) => clip!("av1-2160.mp4", "video/mp4"),
        (VideoFormat::Other(_), _) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{formats::MediaInfo, probe::read_sample_entries};

    #[test]
    fn each_clip_is_the_format_and_height_it_asks_about() {
        for (video, height) in [
            (VideoFormat::H264, 2160),
            (VideoFormat::Hevc, 1080),
            (VideoFormat::Hevc, 2160),
            (VideoFormat::HevcMain10, 1080),
            (VideoFormat::HevcMain10, 2160),
            (VideoFormat::Av1, 1080),
            (VideoFormat::Av1, 2160),
        ] {
            let clip = clip(&video, Some(height)).unwrap();
            let mut info = MediaInfo::default();
            read_sample_entries(clip.bytes, &mut info);
            assert_eq!((info.video, info.height), (Some(video), Some(height)), "{}", clip.name);
            assert!(info.audio.is_empty(), "video only");
            assert!(clip.bytes.len() < 16 * 1024);
        }
        assert_eq!(clip(&VideoFormat::Vp9, Some(720)).unwrap().name, "vp9-1080.webm");
        assert!(clip(&VideoFormat::H264, Some(1080)).is_none());
        assert!(clip(&VideoFormat::Other("MPEG-2".into()), None).is_none());
    }
}
