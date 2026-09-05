fn main() {
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

/// Run `git` with `args` and return its trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
