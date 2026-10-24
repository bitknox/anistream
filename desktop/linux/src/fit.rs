//! A frame that holds its child at one aspect ratio, height for width: what a cover or banner
//! needs, since a picture's own size is its image's and GtkAspectFrame asks no height for a width.

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Fit {
        /// Width over height.
        pub ratio: Cell<f64>,
        /// The width it asks for: a fixed width when `fixed`, else its natural width.
        pub width: Cell<i32>,
        pub fixed: Cell<bool>,
        /// The most height it takes, however wide; 0 for none.
        pub max_height: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Fit {
        const NAME: &'static str = "AnistreamFit";
        type Type = super::Fit;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Fit {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Fit {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let width = self.width.get();
            match orientation {
                gtk::Orientation::Horizontal if self.fixed.get() => (width, width, -1, -1),
                gtk::Orientation::Horizontal => (0, width, -1, -1),
                _ => {
                    let across = if for_size >= 0 { for_size } else { width };
                    let height = self.height_for(across);
                    (height, height, -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.allocate(width, height, baseline, None);
            }
        }
    }

    impl Fit {
        pub fn height_for(&self, width: i32) -> i32 {
            let ratio = self.ratio.get().max(0.01);
            let height = (f64::from(width) / ratio).round() as i32;
            match self.max_height.get() {
                0 => height,
                max => height.min(max),
            }
        }
    }
}

glib::wrapper! {
    pub struct Fit(ObjectSubclass<imp::Fit>) @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Fit {
    /// `child` at `ratio` (width over height), `width` wide: exactly when `fixed`, else as it is
    /// given, asking `width` and taking no more height than `max_height` (0 for none).
    pub fn new(
        child: &impl IsA<gtk::Widget>,
        ratio: f64,
        width: i32,
        fixed: bool,
        max_height: i32,
    ) -> Self {
        let fit: Self = glib::Object::new();
        let imp = fit.imp();
        imp.ratio.set(ratio);
        imp.width.set(width);
        imp.fixed.set(fixed);
        imp.max_height.set(max_height);
        fit.set_overflow(gtk::Overflow::Hidden);
        child.set_parent(&fit);
        fit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_follows_width_up_to_the_cap() {
        let imp = imp::Fit::default();
        imp.ratio.set(16.0 / 9.0);
        assert_eq!(imp.height_for(640), 360);
        imp.max_height.set(300);
        assert_eq!(imp.height_for(640), 300);
        imp.ratio.set(2.0 / 3.0);
        imp.max_height.set(0);
        assert_eq!(imp.height_for(140), 210);
    }
}
