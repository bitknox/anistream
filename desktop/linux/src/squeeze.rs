//! A place in a row or a column that can close up: it holds its child at the child's own size
//! and asks for that size plus a gap, times a fraction from 0 to 1, clipping what is left over.
//! A card that leaves fades out, then its place closes on the standard token so the cards after
//! it close the gap; one that comes back gets its room first, then fades in.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::motion;

mod imp {
    use super::*;

    pub struct Squeeze {
        /// How much of its size it asks for, 0 to 1.
        pub fraction: Cell<f64>,
        /// Added to the child's size along the axis, and closed with it.
        pub gap: Cell<i32>,
        pub along: Cell<gtk::Orientation>,
        /// On its way to closed: its place goes once the spring ends.
        pub closing: Cell<bool>,
        pub running: RefCell<Option<adw::SpringAnimation>>,
    }

    impl Default for Squeeze {
        fn default() -> Self {
            Self {
                fraction: Cell::new(1.0),
                gap: Cell::new(0),
                along: Cell::new(gtk::Orientation::Horizontal),
                closing: Cell::new(false),
                running: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Squeeze {
        const NAME: &'static str = "AnistreamSqueeze"; // i18n-skip
        type Type = super::Squeeze;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Squeeze {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Squeeze {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else { return (0, 0, -1, -1) };
            let along = self.along.get();
            if orientation == along {
                let (_, natural, _, _) = child.measure(orientation, for_size);
                let size = (f64::from(natural + self.gap.get()) * self.fraction.get()).round();
                let size = size.max(0.0) as i32;
                (size, size, -1, -1)
            } else if along == gtk::Orientation::Horizontal {
                // A card: as tall as it is at its own width.
                let (_, width, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                let (min, natural, _, _) = child.measure(orientation, width);
                (min, natural, -1, -1)
            } else {
                let (min, natural, _, _) = child.measure(orientation, -1);
                (min, natural, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(child) = self.obj().first_child() else { return };
            if self.along.get() == gtk::Orientation::Horizontal {
                let (_, natural, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(natural, height, -1, None);
            } else {
                let (_, natural, _, _) = child.measure(gtk::Orientation::Vertical, width);
                child.allocate(width, natural, -1, None);
            }
        }
    }
}

glib::wrapper! {
    pub struct Squeeze(ObjectSubclass<imp::Squeeze>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Squeeze {
    /// `child` in a place along `along`, open, with `gap` after it.
    pub fn new(child: &impl IsA<gtk::Widget>, along: gtk::Orientation, gap: i32) -> Self {
        let squeeze: Self = glib::Object::new();
        squeeze.imp().along.set(along);
        squeeze.imp().gap.set(gap);
        child.set_parent(&squeeze);
        squeeze
    }

    pub fn child(&self) -> Option<gtk::Widget> {
        self.first_child()
    }

    /// Put `child` in the place of the one there.
    pub fn set_child(&self, child: &impl IsA<gtk::Widget>) {
        while let Some(old) = self.first_child() {
            old.unparent();
        }
        child.set_parent(self);
        self.queue_resize();
    }

    pub fn is_closing(&self) -> bool {
        self.imp().closing.get()
    }

    /// On its way to closed from now, before the spring starts: while its card fades out.
    pub fn mark_closing(&self) {
        self.imp().closing.set(true);
    }

    /// Start closed, to open later.
    pub fn set_closed(&self) {
        self.imp().fraction.set(0.0);
        self.queue_resize();
    }

    /// Close on the standard token, then run `done`, unless it was opened again meanwhile.
    pub fn close(&self, done: impl Fn() + 'static) {
        self.imp().closing.set(true);
        let squeeze = self.downgrade();
        self.run(0.0, move || {
            if let Some(squeeze) = squeeze.upgrade()
                && squeeze.is_closing()
            {
                done();
            }
        });
    }

    /// Open on the standard token, then run `done`.
    pub fn open(&self, done: impl Fn() + 'static) {
        self.imp().closing.set(false);
        self.run(1.0, done);
    }

    fn run(&self, to: f64, done: impl Fn() + 'static) {
        if let Some(old) = self.imp().running.take() {
            old.pause();
        }
        // Off screen, as on a page not showing, the place simply takes its new size. One not
        // placed yet waits until it is on screen.
        let off_screen = !self.is_mapped() && self.parent().is_some_and(|p| !p.is_mapped());
        if off_screen {
            self.imp().fraction.set(to);
            self.queue_resize();
            done();
            return;
        }
        // A place just made is not on screen yet, and its spring would skip to the end.
        let squeeze = self.downgrade();
        motion::once_mapped(self, move || {
            let Some(squeeze) = squeeze.upgrade() else { return };
            let from = squeeze.imp().fraction.get();
            let target = adw::CallbackAnimationTarget::new({
                let squeeze = squeeze.downgrade();
                move |value| {
                    if let Some(squeeze) = squeeze.upgrade() {
                        squeeze.imp().fraction.set(value.clamp(0.0, 1.0));
                        squeeze.queue_resize();
                    }
                }
            });
            let animation = motion::standard(&squeeze, from, to, target);
            animation.connect_done(move |_| done());
            animation.play();
            squeeze.imp().running.replace(Some(animation));
        });
    }
}
