#[path = "build_support/xwin_link.rs"]
mod xwin_link;

// Cargo build script that compiles the Slint UI and embeds the Windows application icon.

fn main() {
    emit_build_metadata();
    println!("cargo:rerun-if-changed=assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf");
    println!(
        "cargo:rerun-if-changed=assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf"
    );
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=LIB");
    println!("cargo:rerun-if-env-changed=XWIN_CACHE_DIR");

    std::thread::Builder::new()
        .name("mica-term-build".to_string())
        // Slint codegen walks a very large UI tree; Windows build-script threads can otherwise
        // overflow their default stack before code generation completes.
        .stack_size(32 * 1024 * 1024)
        .spawn(run_build)
        .expect("failed to spawn build worker")
        .join()
        .expect("build worker panicked")
        .expect("build worker failed");
}

fn run_build() -> Result<(), String> {
    let shim_config = xwin_link::ShimConfig::from_env()?;
    if let Some(shim_dir) = xwin_link::maybe_prepare_advapi32_shim(&shim_config)
        .map_err(|err| format!("failed to prepare xwin Advapi32 shim: {err}"))?
    {
        println!("cargo:rustc-link-search=native={}", shim_dir.display());
    }

    let enable_debug_info = std::env::var("PROFILE")
        .map(|profile| profile != "release")
        .unwrap_or(true);
    let config = slint_build::CompilerConfiguration::new().with_debug_info(enable_debug_info);
    slint_build::compile_with_config("ui/app-window.slint", config)
        .map_err(|err| format!("failed to compile Slint UI: {err}"))?;

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Windows installers and taskbar integration rely on a native resource section instead of
        // the SVG/ICO assets used elsewhere in the repository.
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/windows/mica-term.ico");
        res.compile()
            .map_err(|err| format!("failed to compile Windows icon resources: {err}"))?;
    }

    Ok(())
}

fn emit_build_metadata() {
    use std::process::Command;
    fn git(args: &[&str]) -> Option<String> {
        let output = Command::new("git").args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    // Resolve these paths through Git so linked worktrees are tracked correctly too.
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=vendor/tattoy-wezterm-term/src");
    println!("cargo:rerun-if-env-changed=MICA_TERM_RELEASE_DATE");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let built_at = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => {
            chrono::DateTime::from_timestamp(value.parse().expect("invalid SOURCE_DATE_EPOCH"), 0)
                .expect("SOURCE_DATE_EPOCH out of range")
        }
        Err(_) => chrono::Utc::now(),
    };
    let mut revision =
        git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    if git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty()) {
        revision.push_str(" (modified)");
    }
    let released_at =
        std::env::var("MICA_TERM_RELEASE_DATE").unwrap_or_else(|_| "Unreleased / 未发布".into());
    println!("cargo:rustc-env=MICA_TERM_REVISION={revision}");
    println!(
        "cargo:rustc-env=MICA_TERM_BUILT_AT={}",
        built_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("cargo:rustc-env=MICA_TERM_RELEASE_DATE={released_at}");
}
