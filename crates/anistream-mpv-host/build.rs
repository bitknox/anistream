//! Links libmpv when the `libmpv` feature is on.
//!
//! - Linux and macOS: the system's, through pkg-config. The Flatpak bundles its own, and a
//!   distribution's package is found the same way.
//! - Windows with MSVC: libmpv for Windows comes only as a mingw build, a DLL with no import
//!   library MSVC's linker reads. `ANISTREAM_LIBMPV_DIR` names the folder that holds the DLL
//!   (`desktop/windows/scripts/libmpv.ps1` fetches it), and the import library is made here with
//!   `lib.exe` from a definition file of the functions `src/sys.rs` declares, so the two cannot
//!   drift. The DLL itself has to sit beside the program that loads it.

fn main() {
    #[cfg(feature = "libmpv")]
    link();
}

#[cfg(feature = "libmpv")]
fn link() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        msvc::link();
    } else if let Err(e) = pkg_config::Config::new().atleast_version("2.0").probe("mpv") {
        panic!("the libmpv feature needs libmpv's development files (pkg-config mpv): {e}");
    }
}

#[cfg(feature = "libmpv")]
mod msvc {
    use std::{
        env,
        path::{Path, PathBuf},
    };

    pub fn link() {
        println!("cargo:rerun-if-env-changed=ANISTREAM_LIBMPV_DIR");
        println!("cargo:rerun-if-changed=src/sys.rs");
        let dir = env::var_os("ANISTREAM_LIBMPV_DIR").map(PathBuf::from).unwrap_or_else(|| {
            panic!(
                "the libmpv feature with MSVC needs ANISTREAM_LIBMPV_DIR: the folder with \
                 libmpv-2.dll, as desktop/windows/scripts/libmpv.ps1 fetches it"
            )
        });
        let dll = dll_in(&dir);
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
        let def = out.join("mpv.def");
        std::fs::write(&def, definitions(&dll)).expect("the definition file is written");

        let target = env::var("TARGET").expect("cargo sets TARGET");
        let machine = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "ARM64",
            Ok("x86_64") => "X64",
            Ok("x86") => "X86",
            other => panic!("no import library for {other:?}"),
        };
        let mut lib = find_msvc_tools::find(&target, "lib.exe")
            .expect("lib.exe from the Visual Studio Build Tools");
        let status = lib
            .arg("/nologo")
            .arg(format!("/def:{}", def.display()))
            .arg(format!("/machine:{machine}"))
            .arg(format!("/out:{}", out.join("mpv.lib").display()))
            .status()
            .expect("lib.exe runs");
        assert!(status.success(), "lib.exe could not make mpv.lib: {status}");
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=dylib=mpv");
    }

    /// The DLL in `dir`: `libmpv-2.dll`, or another of libmpv's names.
    fn dll_in(dir: &Path) -> String {
        let names = ["libmpv-2.dll", "mpv-2.dll", "libmpv.dll"];
        names
            .iter()
            .find(|name| dir.join(name).is_file())
            .map(|name| (*name).to_owned())
            .unwrap_or_else(|| {
                panic!("ANISTREAM_LIBMPV_DIR ({}) has none of {names:?}", dir.display())
            })
    }

    /// A definition file for `dll` exporting every `mpv_` function `src/sys.rs` declares.
    fn definitions(dll: &str) -> String {
        let sys = std::fs::read_to_string("src/sys.rs").expect("src/sys.rs reads");
        let mut def = format!("LIBRARY {dll}\nEXPORTS\n");
        for line in sys.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("pub fn mpv_") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                def.push_str(&format!("    mpv_{name}\n"));
            }
        }
        def
    }
}
