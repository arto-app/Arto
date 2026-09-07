use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    stamp_version();
    compress_frontend();
}

fn stamp_version() {
    // 1. If ARTO_BUILD_VERSION is already set (e.g., by Nix), use it as-is
    println!("cargo:rerun-if-env-changed=ARTO_BUILD_VERSION");
    if let Ok(v) = std::env::var("ARTO_BUILD_VERSION") {
        if !v.is_empty() {
            println!("cargo:rustc-env=ARTO_BUILD_VERSION={v}");
            return;
        }
    }

    // 2. Try VERSION file (used by CI and Nix builds to override git describe)
    println!("cargo:rerun-if-changed=VERSION");
    if let Ok(v) = std::fs::read_to_string("VERSION") {
        let v = v.trim();
        let v = v.strip_prefix('v').unwrap_or(v);
        if !v.is_empty() {
            println!("cargo:rustc-env=ARTO_BUILD_VERSION={v}");
            return;
        }
    }

    // 3. Try git describe (works in dev and CI macOS)
    if let Some(version) = git(&["describe", "--tags", "--always", "--dirty"]) {
        // Strip 'v' prefix (e.g., "v0.15.3" -> "0.15.3")
        let version = version.strip_prefix('v').unwrap_or(&version);
        println!("cargo:rustc-env=ARTO_BUILD_VERSION={version}");
        // Rerun when git state changes. This crate sits below the repository
        // root and may be checked out as a worktree, so ask git where these
        // files really live instead of assuming `.git/` next to Cargo.toml
        // (a missing path would make cargo re-run this script on every build).
        for file in ["HEAD", "refs/tags", "packed-refs"] {
            if let Some(path) = git(&["rev-parse", "--git-path", file]) {
                println!("cargo:rerun-if-changed={path}");
            }
        }
        return;
    }

    // 4. Fallback to Cargo.toml version
    println!(
        "cargo:rustc-env=ARTO_BUILD_VERSION={}",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
}

/// The frontend files a release binary carries, gzipped into `OUT_DIR`.
///
/// A released Arto has to be one file that runs from wherever it is put, so
/// the stylesheet and the script are compiled into it rather than looked up
/// next to the executable. Raw they add about 8 MB to a 26 MB binary,
/// compressed about 2.5 MB.
///
/// A debug build embeds nothing — `src/assets/frontend.rs` reads the same
/// files from the source tree at run time — and nothing is watched here
/// either, so editing a stylesheet stays a job for the development loop's
/// watcher rather than a reason to relink the app.
fn compress_frontend() {
    if std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some() {
        return;
    }

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    for name in ["main.js", "main.css"] {
        let source = Path::new("assets/frontend").join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        compress(&source, &out_dir.join(format!("{name}.gz")));
    }
}

fn compress(source: &Path, destination: &Path) {
    let bytes = std::fs::read(source).unwrap_or_else(|error| {
        panic!(
            "{}: {error}. Run `just frontend::assets` to build the bundle first.",
            source.display()
        )
    });

    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder
        .write_all(&bytes)
        .and_then(|()| encoder.finish())
        .and_then(|compressed| std::fs::write(destination, compressed))
        .unwrap_or_else(|error| panic!("{}: {error}", destination.display()));
}

/// Run `git` with `args` and return its trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
