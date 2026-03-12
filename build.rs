fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");
    let manifest_dir =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"));
    let repro_input = manifest_dir.join("ui/windows-theme-repro.slint");
    let repro_output = out_dir.join("windows_theme_repro.rs");

    let repro_dependencies = slint_build::compile_with_output_path(
        &repro_input,
        &repro_output,
        slint_build::CompilerConfiguration::new(),
    )
    .expect("failed to compile windows theme repro Slint UI");

    for dependency in repro_dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/windows/mica-term.ico");
        res.compile()
            .expect("failed to compile Windows icon resources");
    }
}
