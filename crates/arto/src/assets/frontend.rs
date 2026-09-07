//! The stylesheet and the script every window loads, and where they come from.
//!
//! A release binary carries them: `dioxus`'s `asset!` embeds a hashed path
//! rather than a file, and the file itself is put in place by `dx bundle`, so
//! a binary from a plain `cargo build` had a stylesheet and a script that
//! resolved to nothing. Carrying them is what makes one file enough to run.
//!
//! A debug build reads the same files from the source tree instead, so that
//! editing a stylesheet stays a job for the development loop's watcher rather
//! than a reason to relink the app.

use std::borrow::Cow;

/// One file the app serves to its own WebViews.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bundled {
    /// The name it is requested by, which is also its name in `assets/frontend`.
    pub name: &'static str,
    pub mime: &'static str,
}

pub const MAIN_SCRIPT: Bundled = Bundled {
    name: "main.js",
    mime: "text/javascript; charset=utf-8",
};

pub const MAIN_STYLE: Bundled = Bundled {
    name: "main.css",
    mime: "text/css; charset=utf-8",
};

const BUNDLE: [Bundled; 2] = [MAIN_SCRIPT, MAIN_STYLE];

/// The file a request names, or `None` when it names nothing the app serves.
pub fn bundled(name: &str) -> Option<Bundled> {
    BUNDLE.into_iter().find(|file| file.name == name)
}

#[cfg(not(debug_assertions))]
mod source {
    use super::{Bundled, MAIN_SCRIPT, MAIN_STYLE};
    use std::borrow::Cow;
    use std::io::Read;
    use std::sync::OnceLock;

    const MAIN_SCRIPT_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.js.gz"));
    const MAIN_STYLE_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.css.gz"));

    /// The file's bytes, decompressed once for the whole process rather than
    /// once per window.
    ///
    /// Matched against the constants themselves rather than re-typing their
    /// names, which would be a second spelling of `Bundled::name` free to
    /// drift from the first — and a name that drifted would answer 404 in a
    /// release while every debug build kept working.
    pub fn bytes(file: Bundled) -> Option<Cow<'static, [u8]>> {
        static SCRIPT: OnceLock<Vec<u8>> = OnceLock::new();
        static STYLE: OnceLock<Vec<u8>> = OnceLock::new();

        let (cell, compressed) = match file {
            MAIN_SCRIPT => (&SCRIPT, MAIN_SCRIPT_GZ),
            MAIN_STYLE => (&STYLE, MAIN_STYLE_GZ),
            _ => return None,
        };
        Some(Cow::Borrowed(
            cell.get_or_init(|| decompress(compressed)).as_slice(),
        ))
    }

    fn decompress(compressed: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(compressed)
            .read_to_end(&mut bytes)
            .expect("the embedded bundle was compressed by this build");
        bytes
    }
}

#[cfg(debug_assertions)]
mod source {
    use super::Bundled;
    use std::borrow::Cow;
    use std::path::PathBuf;

    /// The file as it is on disk right now, so that a rebuilt bundle shows up
    /// in a running window without rebuilding the app.
    pub fn bytes(file: Bundled) -> Option<Cow<'static, [u8]>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/frontend")
            .join(file.name);
        match std::fs::read(&path) {
            Ok(bytes) => Some(Cow::Owned(bytes)),
            Err(error) => {
                tracing::error!(?path, %error, "Frontend bundle is missing; run `just frontend::assets`");
                None
            }
        }
    }
}

/// The bytes to answer a request for `file` with.
pub fn bytes(file: Bundled) -> Option<Cow<'static, [u8]>> {
    source::bytes(file)
}
