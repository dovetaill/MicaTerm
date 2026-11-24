use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.cargo/config.toml");
    println!("cargo:rerun-if-changed=assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf");
    println!(
        "cargo:rerun-if-changed=assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf"
    );
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=LIB");
    println!("cargo:rerun-if-env-changed=XWIN_CACHE_DIR");

    let Ok(out_dir_raw) = env::var("OUT_DIR") else {
        return;
    };
    let out_dir = PathBuf::from(out_dir_raw);
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "linux" {
        prepare_linux_runtime_so_shims(&out_dir);
    } else if target_os == "windows" && target_env == "msvc" {
        // Reserve 16 MiB virtual stack on Windows (MSVC default is only 1 MiB, which overflows
        // on deep GPUI layout/render trees + SSH handshake frames; pages commit lazily on demand).
        println!("cargo:rustc-link-arg-bins=/STACK:16777216");
        // Ensure lld-link /OPT:REF retains AWS-LC's .CRT$XCU static constructor so
        // OPENSSL_cpuid_setup() initializes CPU capability flags before main().
        if target_arch == "x86_64" || target_arch == "aarch64" {
            println!("cargo:rustc-link-arg-bins=/INCLUDE:aws_lc_0_47_0_library_init_constructor");
        }
        if env::consts::OS != "windows" {
            prepare_xwin_case_shims(&out_dir, &target_arch);
        }
    }
}

#[cfg(unix)]
fn prepare_linux_runtime_so_shims(out_dir: &Path) {
    use std::os::unix::fs::symlink;

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

#[cfg(not(unix))]
fn prepare_linux_runtime_so_shims(_out_dir: &Path) {}

fn prepare_xwin_case_shims(out_dir: &Path, target_arch: &str) {
    let msvc_arch = match target_arch {
        "x86_64" => "x86_64",
        "x86" | "i686" => "x86",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => other,
    };

    let mut search_dirs = Vec::new();
    if let Some(cache_dir) = env::var_os("XWIN_CACHE_DIR").map(PathBuf::from) {
        push_unique(
            &mut search_dirs,
            cache_dir
                .join("xwin")
                .join("sdk")
                .join("lib")
                .join("um")
                .join(msvc_arch),
        );
    }
    if let Some(home_dir) = env::var_os("HOME").map(PathBuf::from) {
        push_unique(
            &mut search_dirs,
            home_dir
                .join(".cache")
                .join("cargo-xwin")
                .join("xwin")
                .join("sdk")
                .join("lib")
                .join("um")
                .join(msvc_arch),
        );
    }
    if let Some(lib_env) = env::var_os("LIB") {
        for dir in env::split_paths(&lib_env) {
            push_unique(&mut search_dirs, dir);
        }
    }

    let shim_dir = out_dir.join("xwin-link-shims");
    if fs::create_dir_all(&shim_dir).is_err() {
        return;
    }

    // Mixed-case Win32 / DirectX import libraries frequently requested by Rust crates on Linux hosts.
    let aliases = [
        "Advapi32.lib",
        "Bcrypt.lib",
        "BCrypt.lib",
        "Cfgmgr32.lib",
        "CfgMgr32.lib",
        "Comctl32.lib",
        "ComCtl32.lib",
        "Comdlg32.lib",
        "ComDlg32.lib",
        "Crypt32.lib",
        "D2d1.lib",
        "D2D1.lib",
        "D3d11.lib",
        "D3D11.lib",
        "D3d12.lib",
        "D3D12.lib",
        "D3dcompiler.lib",
        "D3DCompiler.lib",
        "Dcomp.lib",
        "DComp.lib",
        "Dwmapi.lib",
        "DwmApi.lib",
        "Dwrite.lib",
        "DWrite.lib",
        "Dxgi.lib",
        "DXGI.lib",
        "Dxguid.lib",
        "Gdi32.lib",
        "Imm32.lib",
        "Iphlpapi.lib",
        "Kernel32.lib",
        "Ncrypt.lib",
        "NCrypt.lib",
        "Netapi32.lib",
        "NetApi32.lib",
        "Ntdll.lib",
        "Ole32.lib",
        "OleAut32.lib",
        "Oleaut32.lib",
        "OneCore.lib",
        "OneCoreUap.lib",
        "Pdh.lib",
        "PowrProf.lib",
        "Powrprof.lib",
        "Propsys.lib",
        "PropSys.lib",
        "Psapi.lib",
        "RuntimeObject.lib",
        "Secur32.lib",
        "Setupapi.lib",
        "SetupAPI.lib",
        "Shcore.lib",
        "SHCore.lib",
        "Shell32.lib",
        "Shlwapi.lib",
        "Synchronization.lib",
        "UIAutomationCore.lib",
        "Uiautomationcore.lib",
        "User32.lib",
        "Userenv.lib",
        "UserEnv.lib",
        "Uuid.lib",
        "UxTheme.lib",
        "Uxtheme.lib",
        "Version.lib",
        "Winhttp.lib",
        "WinHttp.lib",
        "Winmm.lib",
        "WinMM.lib",
        "Winspool.lib",
        "WinSpool.lib",
        "Wtsapi32.lib",
        "Ws2_32.lib",
        "WS2_32.lib",
    ];

    let mut shimmed_any = false;
    for alias in aliases {
        let dest = shim_dir.join(alias);
        if dest.exists() {
            shimmed_any = true;
            continue;
        }
        if let Some(src) = find_case_insensitive_lib(&search_dirs, alias)
            && link_or_copy(&src, &dest).is_ok()
        {
            shimmed_any = true;
        }
    }

    if shimmed_any {
        println!("cargo:rustc-link-search=native={}", shim_dir.display());
    }
}

fn push_unique(dirs: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !dirs.iter().any(|existing| existing == &candidate) {
        dirs.push(candidate);
    }
}

fn find_case_insensitive_lib(search_dirs: &[PathBuf], target_name: &str) -> Option<PathBuf> {
    for dir in search_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(target_name)
            {
                return Some(entry.path());
            }
        }
    }
    None
}

fn link_or_copy(src: &Path, dest: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(src, dest).is_ok() {
            return Ok(());
        }
    }
    fs::copy(src, dest).map(|_| ())
}
