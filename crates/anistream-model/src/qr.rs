//! A QR code as plain modules, for every front end to draw its own way.
//!
//! The model makes the matrix and the terminal's half-block rendering of it; an app draws the
//! modules with its own graphics. The quiet zone is part of the value, so a front end that draws
//! `size` modules plus the zone on a plate of its own colour gets a code a camera can read.

use qrcode::{Color, EcLevel, QrCode};

/// The quiet zone a QR code needs around it, in modules.
pub const QUIET_ZONE: u32 = 4;

/// A QR code: `size` by `size` modules, row-major, true for a dark one. Draw it with `quiet_zone`
/// modules of light all round.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct QrMatrix {
    pub size: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub modules: Vec<bool>,
    pub quiet_zone: u32,
}

impl QrMatrix {
    /// Whether the module at `x`, `y` is dark. Off the code is light.
    pub fn dark(&self, x: u32, y: u32) -> bool {
        x < self.size && y < self.size && self.modules[(y * self.size + x) as usize]
    }
}

/// The QR code for `text`, at medium error correction. Text too long for any QR code gives an
/// empty matrix (size 0), which a front end draws as nothing.
pub fn qr_matrix(text: &str) -> QrMatrix {
    let Ok(code) = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M) else {
        return QrMatrix { quiet_zone: QUIET_ZONE, ..QrMatrix::default() };
    };
    QrMatrix {
        size: code.width() as u32,
        modules: code.to_colors().into_iter().map(|c| c == Color::Dark).collect(),
        quiet_zone: QUIET_ZONE,
    }
}

/// The matrix for a terminal, two module rows to a line of `▀`, `▄`, `█` and space, with the
/// quiet zone drawn in. A dark module is a filled block, so the terminal's foreground draws the
/// code: print it on a light terminal, or in reverse video on a dark one.
pub fn qr_half_blocks(matrix: &QrMatrix) -> Vec<String> {
    if matrix.size == 0 {
        return Vec::new();
    }
    let zone = matrix.quiet_zone as i64;
    let full = matrix.size as i64 + 2 * zone;
    let dark = |x: i64, y: i64| {
        let (x, y) = (x - zone, y - zone);
        x >= 0
            && y >= 0
            && x < matrix.size as i64
            && y < matrix.size as i64
            && matrix.modules[(y * matrix.size as i64 + x) as usize]
    };
    (0..full)
        .step_by(2)
        .map(|y| {
            (0..full)
                .map(|x| {
                    let (top, bottom) = (dark(x, y), dark(x, y + 1));
                    match (top, bottom) {
                        (true, true) => '█',
                        (true, false) => '▀',
                        (false, true) => '▄',
                        (false, false) => ' ',
                    }
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVITE: &str = "https://store.example/join#invite=abc123def456";

    /// Draw the matrix with its quiet zone, four pixels to a module, and read it back.
    fn decode(matrix: &QrMatrix) -> String {
        let scale = 4usize;
        let side = (matrix.size + 2 * matrix.quiet_zone) as usize * scale;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(side, side, |x, y| {
            let mx = (x / scale) as i64 - matrix.quiet_zone as i64;
            let my = (y / scale) as i64 - matrix.quiet_zone as i64;
            let dark = mx >= 0 && my >= 0 && matrix.dark(mx as u32, my as u32);
            if dark { 0 } else { 255 }
        });
        let grids = image.detect_grids();
        assert_eq!(grids.len(), 1, "one code in the image");
        let (_, content) = grids[0].decode().expect("the code decodes");
        content
    }

    #[test]
    fn an_invite_link_decodes_back() {
        assert_eq!(decode(&qr_matrix(INVITE)), INVITE);
    }

    #[test]
    fn the_matrix_size_is_stable() {
        let matrix = qr_matrix(INVITE);
        // 46 bytes at medium correction is a version 4 code, 33 modules a side.
        assert_eq!(matrix.size, 33);
        assert_eq!(matrix.modules.len(), 33 * 33);
        assert_eq!(matrix.quiet_zone, 4);
        assert_eq!(qr_matrix(INVITE), matrix, "the same text gives the same code");
    }

    #[test]
    fn text_too_long_for_a_code_gives_none() {
        let matrix = qr_matrix(&"x".repeat(4000));
        assert_eq!(matrix.size, 0);
        assert!(qr_half_blocks(&matrix).is_empty());
    }

    #[test]
    fn half_blocks_pair_two_rows_per_line_and_keep_the_quiet_zone() {
        let matrix = qr_matrix(INVITE);
        let lines = qr_half_blocks(&matrix);
        let full = (matrix.size + 2 * matrix.quiet_zone) as usize;
        assert_eq!(lines.len(), full.div_ceil(2));
        assert!(lines.iter().all(|line| line.chars().count() == full));
        // The quiet zone is light on the first rows and the first columns.
        assert!(lines[0].chars().all(|c| c == ' '));
        assert!(lines[1].starts_with("    "));
    }
}
