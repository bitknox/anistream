//! Cover and banner image rendering.
//!
//! The headline capability: real graphics in the terminal via Kitty, Sixel or iTerm2, with
//! a halfblocks fallback where none of those exist. `ratatui-image` handles protocol
//! detection; this module handles everything around it.
//!
//! Two rules, both about never blocking the UI:
//!
//! - **Decode and resize happen off the event-loop thread.** A 1 MB JPEG takes long enough
//!   that doing it inline would visibly stutter scrolling through a cover grid.
//! - **Every failure is silent and local.** A missing or corrupt image renders as an empty
//!   plate; it must never fail a screen, because cover art is decoration and the title
//!   underneath it is not.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

/// What the terminal can actually do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graphics {
    /// A real graphics protocol is available.
    Native,
    /// Unicode halfblocks — coarse, but present everywhere.
    Halfblocks,
    /// Explicitly disabled by config.
    Disabled,
}

impl Graphics {
    pub const fn describes_real_images(self) -> bool {
        matches!(self, Self::Native)
    }
}

/// Detects protocol support and builds render protocols.
pub struct ImageEngine {
    picker: Option<Picker>,
    graphics: Graphics,
    cache_dir: PathBuf,
    /// Font cell aspect, used to size plates so covers are not stretched.
    font_size: (u16, u16),
}

impl std::fmt::Debug for ImageEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageEngine")
            .field("graphics", &self.graphics)
            .field("font_size", &self.font_size)
            .finish_non_exhaustive()
    }
}

impl ImageEngine {
    /// Query the terminal and prepare an engine.
    ///
    /// Detection can fail — a pipe, a CI runner, a terminal that ignores the query — and
    /// that is not an error. It degrades to halfblocks, which work anywhere.
    pub fn detect(enabled: bool) -> Self {
        if !enabled {
            return Self::disabled();
        }
        match Picker::from_query_stdio() {
            Ok(picker) => {
                let fs = picker.font_size();
                let font_size = (fs.width, fs.height);
                tracing::info!(?font_size, "terminal graphics available");
                Self {
                    picker: Some(picker),
                    graphics: Graphics::Native,
                    cache_dir: PathBuf::new(),
                    font_size,
                }
            }
            Err(e) => {
                tracing::info!(error = %e, "no graphics protocol; using halfblocks");
                // A sensible default cell aspect for halfblocks, which pack two vertical
                // pixels per cell.
                Self {
                    picker: Some(Picker::halfblocks()),
                    graphics: Graphics::Halfblocks,
                    cache_dir: PathBuf::new(),
                    font_size: (8, 16),
                }
            }
        }
    }

    pub fn disabled() -> Self {
        Self {
            picker: None,
            graphics: Graphics::Disabled,
            cache_dir: PathBuf::new(),
            font_size: (8, 16),
        }
    }

    /// Halfblocks, without asking the terminal anything.
    ///
    /// [`Self::detect`] writes a query to stdout and waits for a reply on stdin. That is fine in
    /// front of a real terminal and wrong everywhere else: under a test harness stdio is captured,
    /// and on Windows the query opens the console handles directly and waits on a reply that never
    /// arrives — the wait restarts whenever the parser reports itself busy, so it does not
    /// reliably time out. Anything that wants the rendering pipeline rather than the detection
    /// should start here.
    pub fn halfblocks() -> Self {
        Self {
            picker: Some(Picker::halfblocks()),
            graphics: Graphics::Halfblocks,
            cache_dir: PathBuf::new(),
            font_size: (8, 16),
        }
    }

    pub fn with_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = dir.into();
        self
    }

    pub fn graphics(&self) -> Graphics {
        self.graphics
    }

    pub fn font_size(&self) -> (u16, u16) {
        self.font_size
    }

    pub fn is_enabled(&self) -> bool {
        self.picker.is_some()
    }

    /// Cell dimensions that preserve an image's aspect ratio inside `available`.
    ///
    /// Terminal cells are roughly twice as tall as they are wide, so treating them as
    /// square is the classic way to end up with grotesquely stretched cover art.
    pub fn fit(&self, image_px: (u32, u32), available: Rect) -> Rect {
        let (cw, ch) = self.font_size;
        if cw == 0 || ch == 0 || image_px.0 == 0 || image_px.1 == 0 {
            return available;
        }
        let img_aspect = image_px.0 as f32 / image_px.1 as f32;
        let cell_aspect = cw as f32 / ch as f32;

        // Width in cells if we use the full available height.
        let width_from_height =
            ((available.height as f32) * img_aspect / cell_aspect).round() as u16;

        if width_from_height <= available.width {
            Rect { width: width_from_height.max(1), ..available }
        } else {
            let height_from_width =
                ((available.width as f32) * cell_aspect / img_aspect).round() as u16;
            Rect { height: height_from_width.max(1).min(available.height), ..available }
        }
    }

    /// On-disk path for a cached image.
    pub fn cache_path(&self, url: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.img", stable_hash(url)))
    }
}

impl ImageEngine {
    /// Build a render protocol for a decoded image.
    ///
    /// The protocol resizes and encodes lazily, at render time, against whatever area it is
    /// given — which is why [`ImageStore`] has to hand out `&mut` during a draw.
    pub fn protocol_for(&self, image: image::DynamicImage) -> Option<StatefulProtocol> {
        self.picker.as_ref().map(|p| p.new_resize_protocol(image))
    }
}

/// Decoded images, ready to render.
///
/// Interior mutability is deliberate rather than incidental. `ratatui-image` resizes and
/// encodes during the draw call, so a protocol genuinely needs `&mut` while rendering. A
/// `RefCell` confines that to the image cache, which lets [`crate::screens::render`] keep
/// taking `&App` — so a render still cannot change any *application* state.
pub struct ImageStore {
    engine: ImageEngine,
    protocols: std::cell::RefCell<HashMap<String, StatefulProtocol>>,
    requests: ImageRequests,
}

impl std::fmt::Debug for ImageStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageStore")
            .field("engine", &self.engine)
            .field("cached", &self.protocols.borrow().len())
            .finish()
    }
}

impl ImageStore {
    pub fn new(engine: ImageEngine) -> Self {
        Self {
            engine,
            protocols: std::cell::RefCell::new(HashMap::new()),
            requests: ImageRequests::new(),
        }
    }

    pub fn engine(&self) -> &ImageEngine {
        &self.engine
    }

    pub fn requests(&self) -> &ImageRequests {
        &self.requests
    }

    /// Whether an image should be fetched. Claims it so it is only fetched once.
    pub fn should_fetch(&self, url: &str) -> bool {
        self.engine.is_enabled() && self.requests.claim(url)
    }

    /// Store a decoded image.
    pub fn insert(&self, url: &str, image: image::DynamicImage) {
        match self.engine.protocol_for(image) {
            Some(protocol) => {
                self.protocols.borrow_mut().insert(url.to_owned(), protocol);
                self.requests.mark(url, RequestState::Ready);
            }
            None => self.requests.mark(url, RequestState::Failed),
        }
    }

    pub fn mark_failed(&self, url: &str) {
        self.requests.mark(url, RequestState::Failed);
    }

    pub fn has(&self, url: &str) -> bool {
        self.protocols.borrow().contains_key(url)
    }

    pub fn len(&self) -> usize {
        self.protocols.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Draw a cached image into `area`, returning whether anything was drawn.
    ///
    /// `false` means the caller should fall back to its reserved plate. Every failure path
    /// lands here rather than propagating: cover art is decoration, and the title beneath
    /// it is not.
    pub fn render_into(
        &self,
        url: &str,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) -> bool {
        if area.width == 0 || area.height == 0 {
            return false;
        }
        let Ok(mut protocols) = self.protocols.try_borrow_mut() else {
            // Already borrowed: a nested render. Skip rather than panic.
            return false;
        };
        let Some(protocol) = protocols.get_mut(url) else {
            return false;
        };

        ratatui::widgets::StatefulWidget::render(
            ratatui_image::StatefulImage::default(),
            area,
            buf,
            protocol,
        );
        true
    }
}

/// Shrink an image before it is handed to a protocol.
///
/// A banner arrives at ~1900px wide but will never occupy more than a couple of hundred
/// terminal pixels. Keeping the original resident would cost far more memory than the
/// rendered result can ever use, multiplied by every cover on screen.
pub fn downscale(image: image::DynamicImage, max_edge: u32) -> image::DynamicImage {
    use image::GenericImageView;
    let (w, h) = image.dimensions();
    if w.max(h) <= max_edge {
        return image;
    }
    // `Triangle` is a good quality/speed tradeoff at this scale, and this runs off the UI
    // thread anyway.
    image.resize(max_edge, max_edge, image::imageops::FilterType::Triangle)
}

/// Stable, filesystem-safe hash of a URL.
///
/// Not cryptographic — this only needs to avoid collisions between cover URLs and produce
/// the same name across runs so the cache survives a restart.
pub fn stable_hash(input: &str) -> String {
    // FNV-1a: tiny, stable across releases, and good enough for cache keys.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Whether a cached file is still usable.
pub fn cache_is_fresh(path: &Path, max_age_secs: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    meta.modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age.as_secs() < max_age_secs)
}

/// Tracks which images have been requested, so the same cover is fetched once.
///
/// Scrolling a grid re-renders constantly; without this, every frame would re-request every
/// visible cover.
#[derive(Debug, Clone, Default)]
pub struct ImageRequests {
    inner: Arc<Mutex<HashMap<String, RequestState>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    Pending,
    Ready,
    /// Failed; do not retry this session. A broken cover URL will stay broken, and
    /// retrying every frame would hammer the CDN for nothing.
    Failed,
}

impl ImageRequests {
    pub fn new() -> Self {
        Self::default()
    }

    /// Claim a URL for fetching. Returns `true` only for the first caller.
    pub fn claim(&self, url: &str) -> bool {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.contains_key(url) {
            return false;
        }
        guard.insert(url.to_owned(), RequestState::Pending);
        true
    }

    pub fn mark(&self, url: &str, state: RequestState) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(url.to_owned(), state);
    }

    pub fn state(&self, url: &str) -> Option<RequestState> {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get(url).copied()
    }

    pub fn pending_count(&self) -> usize {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.values().filter(|s| **s == RequestState::Pending).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> ImageEngine {
        // Fixed font metrics so aspect maths is deterministic in tests.
        ImageEngine {
            picker: None,
            graphics: Graphics::Halfblocks,
            cache_dir: PathBuf::from("/tmp/anistream-test"),
            font_size: (8, 16),
        }
    }

    fn rect(w: u16, h: u16) -> Rect {
        Rect { x: 0, y: 0, width: w, height: h }
    }

    #[test]
    #[cfg(not(windows))]
    fn detection_degrades_to_halfblocks_rather_than_failing() {
        // Under `cargo test` stdout is not a terminal, which is the same situation as a
        // pipe or a CI runner. It must not be an error.
        //
        // Not run on Windows: there the query opens the console handles directly and blocks
        // waiting for a reply that captured stdio will never send. See `ImageEngine::halfblocks`.
        let engine = ImageEngine::detect(true);
        assert!(engine.graphics() != Graphics::Disabled);
        assert!(engine.is_enabled());
    }

    #[test]
    fn images_can_be_switched_off_entirely() {
        let engine = ImageEngine::detect(false);
        assert_eq!(engine.graphics(), Graphics::Disabled);
        assert!(!engine.is_enabled());
        assert!(!engine.graphics().describes_real_images());
    }

    #[test]
    fn a_tall_cover_is_fitted_by_height_not_stretched() {
        // Anime covers are portrait. Treating cells as square is the classic way to end up
        // with a grotesquely wide, squashed image.
        let e = engine();
        let fitted = e.fit((460, 650), rect(40, 20));
        assert!(fitted.height <= 20);
        assert!(fitted.width < 40, "a portrait cover must not use the full width");
        assert!(fitted.width > 0);
    }

    #[test]
    fn a_wide_banner_is_fitted_by_width() {
        let e = engine();
        let fitted = e.fit((1900, 400), rect(80, 30));
        assert_eq!(fitted.width, 80, "a wide banner should use the full width");
        assert!(fitted.height < 30);
        assert!(fitted.height > 0);
    }

    #[test]
    fn fitting_accounts_for_the_cell_aspect_ratio() {
        // A perfectly square image should occupy about half as many rows as columns,
        // because cells are about twice as tall as they are wide.
        let e = engine();
        let fitted = e.fit((100, 100), rect(40, 40));
        assert!(
            fitted.height < fitted.width,
            "square image rendered as {}x{}, cells are not square",
            fitted.width,
            fitted.height
        );
    }

    #[test]
    fn fitting_never_returns_a_zero_dimension() {
        let e = engine();
        for available in [rect(1, 1), rect(1, 40), rect(40, 1)] {
            let fitted = e.fit((460, 650), available);
            assert!(fitted.width >= 1 && fitted.height >= 1);
            assert!(fitted.width <= available.width.max(1));
        }
    }

    #[test]
    fn degenerate_image_dimensions_fall_back_to_the_available_area() {
        let e = engine();
        assert_eq!(e.fit((0, 0), rect(10, 5)), rect(10, 5));
    }

    #[test]
    fn cache_keys_are_stable_and_distinct() {
        // Stable across runs, or the disk cache is useless.
        let a = stable_hash("https://s4.anilist.co/cover/154587.jpg");
        assert_eq!(a, stable_hash("https://s4.anilist.co/cover/154587.jpg"));
        assert_ne!(a, stable_hash("https://s4.anilist.co/cover/154588.jpg"));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "must be filename-safe");
    }

    #[test]
    fn a_url_is_claimed_by_exactly_one_caller() {
        // Scrolling re-renders constantly; without this every frame would re-fetch every
        // visible cover.
        let requests = ImageRequests::new();
        let url = "https://example.test/a.jpg";
        assert!(requests.claim(url));
        assert!(!requests.claim(url), "second claim must be refused");
        assert_eq!(requests.state(url), Some(RequestState::Pending));
        assert_eq!(requests.pending_count(), 1);
    }

    #[test]
    fn a_failed_image_is_not_retried() {
        // A broken cover URL stays broken; retrying each frame would hammer the CDN.
        let requests = ImageRequests::new();
        let url = "https://example.test/missing.jpg";
        requests.claim(url);
        requests.mark(url, RequestState::Failed);
        assert!(!requests.claim(url));
        assert_eq!(requests.state(url), Some(RequestState::Failed));
        assert_eq!(requests.pending_count(), 0);
    }

    #[test]
    fn unrequested_urls_have_no_state() {
        assert_eq!(ImageRequests::new().state("never asked"), None);
    }

    #[test]
    fn a_missing_or_empty_cache_file_is_not_fresh() {
        assert!(!cache_is_fresh(Path::new("/definitely/not/here.img"), 3600));

        let dir = std::env::temp_dir().join("anistream-cache-test");
        std::fs::create_dir_all(&dir).ok();
        let empty = dir.join("empty.img");
        std::fs::write(&empty, b"").ok();
        assert!(!cache_is_fresh(&empty, 3600), "a zero-byte file is a failed download");
        std::fs::remove_file(&empty).ok();
    }

    #[test]
    fn cache_paths_live_under_the_configured_directory() {
        let e = engine();
        let path = e.cache_path("https://example.test/a.jpg");
        assert!(path.starts_with("/tmp/anistream-test"));
        assert!(path.extension().is_some_and(|x| x == "img"));
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;

    fn store() -> ImageStore {
        // Constructed directly rather than detected: what is under test is the store, and
        // probing the terminal from a test harness buys nothing and hangs on Windows.
        ImageStore::new(ImageEngine::halfblocks())
    }

    fn sample(w: u32, h: u32) -> image::DynamicImage {
        image::DynamicImage::ImageRgb8(image::RgbImage::new(w, h))
    }

    #[test]
    fn an_inserted_image_becomes_renderable() {
        let s = store();
        assert!(!s.has("u"));
        s.insert("u", sample(40, 60));
        assert!(s.has("u"));
        assert_eq!(s.requests().state("u"), Some(RequestState::Ready));
    }

    #[test]
    fn rendering_an_unknown_url_reports_that_it_drew_nothing() {
        // The caller relies on `false` to fall back to its reserved plate.
        let s = store();
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 10, 4));
        assert!(!s.render_into("never fetched", buf.area, &mut buf));
    }

    #[test]
    fn rendering_into_a_zero_sized_area_is_refused() {
        let s = store();
        s.insert("u", sample(40, 60));
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 10, 4));
        assert!(!s.render_into("u", Rect::new(0, 0, 0, 0), &mut buf));
    }

    #[test]
    fn a_cached_image_actually_paints() {
        let s = store();
        s.insert("u", sample(40, 60));
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        assert!(s.render_into("u", area, &mut buf));

        // Evidence of painting is a changed *style*, not a changed symbol: halfblocks
        // renders a uniform region as spaces carrying a background colour, so checking
        // for non-space glyphs would miss a perfectly good image.
        let default = ratatui::buffer::Cell::default();
        let painted = (0..area.width).flat_map(|x| (0..area.height).map(move |y| (x, y))).any(
            |(x, y)| {
                let cell = &buf[(x, y)];
                cell.symbol() != default.symbol()
                    || cell.fg != default.fg
                    || cell.bg != default.bg
            },
        );
        assert!(painted, "render_into claimed success but drew nothing");
    }

    #[test]
    fn a_disabled_engine_never_claims_a_fetch() {
        // With images off, nothing should spend bandwidth on covers.
        let s = ImageStore::new(ImageEngine::disabled());
        assert!(!s.should_fetch("u"));
        s.insert("u", sample(10, 10));
        assert!(!s.has("u"));
        assert_eq!(s.requests().state("u"), Some(RequestState::Failed));
    }

    #[test]
    fn a_url_is_only_claimed_once_even_across_re_renders() {
        let s = store();
        assert!(s.should_fetch("u"));
        assert!(!s.should_fetch("u"), "scrolling must not re-request the same cover");
    }

    #[test]
    fn downscaling_bounds_the_longest_edge_and_keeps_aspect() {
        use image::GenericImageView;
        let (w, h) = downscale(sample(1900, 400), 900).dimensions();
        assert!(w.max(h) <= 900);
        // 1900:400 is 4.75:1; the result must stay close to that.
        let aspect = w as f32 / h as f32;
        assert!((aspect - 4.75).abs() < 0.2, "aspect drifted to {aspect}");
    }

    #[test]
    fn a_small_image_is_left_alone() {
        use image::GenericImageView;
        assert_eq!(downscale(sample(100, 150), 900).dimensions(), (100, 150));
    }
}
