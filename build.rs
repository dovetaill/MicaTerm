#[path = "build_support/xwin_link.rs"]
mod xwin_link;

// Cargo build script that compiles the Slint UI and embeds the Windows application icon.

fn main() {
    println!("cargo:rerun-if-changed=ui/fonts/SarasaTermSCNerd-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/SarasaUiSC/SarasaUiSC-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/SarasaUiSC/SarasaUiSC-SemiBold.ttf");
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
