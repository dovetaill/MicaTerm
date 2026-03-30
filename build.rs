// Cargo build script that compiles the Slint UI and embeds the Windows application icon.

fn main() {
    println!("cargo:rerun-if-changed=ui/fonts/MapleMonoNormalNL-NF-CN-Regular.ttf");

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
