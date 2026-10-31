//! The flags beside the languages' own names in the language picker. The images are
//! assets/flags/png, rendered from the flag-icons SVGs, so the app needs no SVG loader.

use gtk::prelude::*;
use gtk::{gdk, glib};

macro_rules! flag_bytes {
    ($($code:literal),+) => {
        /// The PNG of an ISO code's flag, if the app bundles it.
        fn bytes(code: &str) -> Option<&'static [u8]> {
            match code {
                $($code => Some(include_bytes!(concat!("../../../assets/flags/png/", $code, ".png"))),)+
                _ => None,
            }
        }
        #[cfg(test)]
        const CODES: &[&str] = &[$($code),+];
    };
}

flag_bytes!(
    "gb", "se", "jp", "mx", "br", "fr", "de", "it", "pl", "ru", "tr", "id", "vn", "th", "kr",
    "cn", "tw"
);

/// A flag as a 20 by 15 picture with rounded corners and a hairline edge drawn over it
/// (`.language-flag` in the stylesheet). It is decoration: it has no role for the screen reader,
/// and the row is read by the language's name. `None` for a code the app does not bundle.
pub fn picture(code: &str) -> Option<gtk::Widget> {
    let texture = gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes(code)?)).ok()?;
    let picture = gtk::Picture::builder()
        .paintable(&texture)
        .content_fit(gtk::ContentFit::Fill)
        .width_request(20)
        .height_request(15)
        .css_classes(["language-flag"])
        .build();
    let edge =
        gtk::Box::builder().css_classes(["language-flag-edge"]).can_target(false).build();
    let flag = gtk::Overlay::builder()
        .child(&picture)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    flag.add_overlay(&edge);
    Some(flag.upcast())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_code_is_a_png() {
        for code in super::CODES {
            let bytes = super::bytes(code).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{code}");
        }
        assert!(super::bytes("xx").is_none());
    }
}
