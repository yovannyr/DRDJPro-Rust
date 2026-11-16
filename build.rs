fn main() {
    // Track the actual checkout (including worktree ref paths), not a build-time
    // timestamp. Source archives without Git explicitly identify an unknown commit.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_owned())
    };
    for name in [
        "HEAD".to_owned(),
        git(&["symbolic-ref", "-q", "HEAD"]).unwrap_or_default(),
        "packed-refs".to_owned(),
    ] {
        if !name.is_empty() {
            if let Some(path) = git(&["rev-parse", "--git-path", &name]) {
                if std::path::Path::new(&path).exists() {
                    println!("cargo:rerun-if-changed={path}");
                }
            }
        }
    }
    let commit = git(&["rev-parse", "HEAD"])
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=DRDJ_BUILD_COMMIT={commit}");
    println!(
        "cargo:rustc-env=DRDJ_BUILD_TARGET={}",
        std::env::var("TARGET").expect("Cargo target")
    );
    println!(
        "cargo:rustc-env=DRDJ_BUILD_PROFILE={}",
        std::env::var("PROFILE").expect("Cargo profile")
    );
    // Headless accessibility tests must also work without CI environment variables.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/app.slint", config).expect("Failed to compile Slint UI");
}
