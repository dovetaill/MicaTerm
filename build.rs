fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::symlink;
        use std::path::PathBuf;

        let Ok(out_dir_raw) = std::env::var("OUT_DIR") else {
            return;
        };
        let out_dir = PathBuf::from(out_dir_raw);

        // Bridge versioned runtime shared objects on Debian/Ubuntu hosts when -dev symlinks are absent.
        let lib_dirs = [
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib/aarch64-linux-gnu",
            "/usr/lib64",
            "/usr/lib",
        ];
        let candidates = [
            ("libxkbcommon.so", "libxkbcommon.so.0"),
            ("libxkbcommon-x11.so", "libxkbcommon-x11.so.0"),
            ("libX11-xcb.so", "libX11-xcb.so.1"),
            ("libxcb-xkb.so", "libxcb-xkb.so.1"),
        ];

        let mut linked_any = false;
        for (unversioned, versioned) in candidates {
            let link_path = out_dir.join(unversioned);
            if link_path.exists() {
                linked_any = true;
                continue;
            }
            for dir in lib_dirs {
                let direct = PathBuf::from(dir).join(unversioned);
                if direct.exists() {
                    break;
                }
                let target = PathBuf::from(dir).join(versioned);
                if target.exists() {
                    let _ = symlink(&target, &link_path);
                    linked_any = true;
                    break;
                }
            }
        }

        if linked_any {
            println!("cargo:rustc-link-search=native={}", out_dir.display());
        }
    }
}
