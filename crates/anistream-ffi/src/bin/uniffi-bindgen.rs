//! The bindings generator, from the same UniFFI release as the scaffolding compiled into the
//! library, which the generated code checks when it loads the library. Reads the interface from
//! the built library itself:
//!
//! ```text
//! cargo run -p anistream-ffi --features cli --bin uniffi-bindgen -- \
//!     generate --library <target>/debug/libanistream_ffi.dylib --language swift --out-dir <dir>
//! ```
//!
//! `apple/build.sh` runs it for the Swift package.

fn main() {
    uniffi::uniffi_bindgen_main();
}
