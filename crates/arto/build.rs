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
    if let Ok(output) = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
    {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Strip 'v' prefix (e.g., "v0.15.3" -> "0.15.3")
            let version = version.strip_prefix('v').unwrap_or(&version);
            println!("cargo:rustc-env=ARTO_BUILD_VERSION={version}");
            // Rerun when git state changes
            println!("cargo:rerun-if-changed=.git/HEAD");
            println!("cargo:rerun-if-changed=.git/refs/tags");
            println!("cargo:rerun-if-changed=.git/packed-refs");
            return;
        }
    }

    // 4. Fallback to Cargo.toml version
    println!(
        "cargo:rustc-env=ARTO_BUILD_VERSION={}",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
}
