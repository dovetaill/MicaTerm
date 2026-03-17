// Cargo build script that compiles the Slint UI and embeds the Windows application icon.

fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Windows installers and taskbar integration rely on a native resource section instead of
        // the SVG/ICO assets used elsewhere in the repository.
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/windows/mica-term.ico");
        res.compile()
            .expect("failed to compile Windows icon resources");
    }
}
