//! Source-level contract coverage for the packaged memory baseline playbook.

use std::fs;

#[test]
fn packaged_memory_baseline_docs_define_windows_matrix_and_primary_metrics() {
    let readme = fs::read_to_string("readme.md").expect("read readme");
    let verification =
        fs::read_to_string("docs/plans/2026-06-09-memory-footprint-reduction/verification.md")
            .expect("read memory verification playbook");

    assert!(
        readme.contains("docs/plans/2026-06-09-memory-footprint-reduction/verification.md"),
        "README should point engineers at the packaged memory baseline playbook instead of leaving the Windows repro flow as an orphaned snippet"
    );
    assert!(
        verification.contains("./build-win-x64.sh")
            && verification.contains("./build-win-x64-software.sh"),
        "the baseline playbook should split the Windows mainline and software compatibility package wrappers"
    );
    assert!(
        verification.contains("requested_build_flavor")
            && verification.contains("requested_terminal_render_mode")
            && verification.contains("requested_native_present_path")
            && verification.contains("fallback_level"),
        "the baseline playbook should record renderer/path identity fields so packaged fallback runs do not get mixed together"
    );
    assert!(
        verification.contains("Private Bytes")
            && verification.contains("Page File Bytes")
            && verification.contains("Working Set - Private")
            && verification.contains("Working Set")
            && verification.contains("Handle Count")
            && verification.contains("Thread Count"),
        "the baseline playbook should lock the Windows counter set needed to distinguish working-set drops from private/commit movement"
    );
    assert!(
        verification.contains("冷启动空载")
            && verification.contains("欢迎页空载 30 秒 / 60 秒")
            && verification.contains("首次打开终端")
            && verification.contains("大量输出")
            && verification.contains("重滚动")
            && verification.contains("3 到 5 个 session")
            && verification.contains("关闭全部 session 后立即 / 30 秒 / 60 秒")
            && verification.contains("重启应用"),
        "the baseline playbook should pin the staged Windows memory scenarios before any optimization claims are made"
    );
    assert!(
        verification.contains("不能只凭 working set 回落宣称成功")
            || verification.contains("不能只看 working set"),
        "the baseline playbook should explicitly reject working-set-only success claims"
    );
}

#[test]
fn main_entrypoint_logs_requested_packaged_identity_for_renderer_baselines() {
    let main_source = fs::read_to_string("src/main.rs").expect("read main");

    assert!(
        main_source.contains("requested_build_flavor"),
        "renderer selection logs should record the requested build flavor so packaged baseline captures can distinguish mainline from software compatibility runs"
    );
    assert!(
        main_source.contains("requested_terminal_render_mode"),
        "renderer selection logs should record the requested terminal render mode before any runtime fallback happens"
    );
    assert!(
        main_source.contains("requested_native_present_path"),
        "renderer selection logs should record the requested native present path so retained-native and other presentation paths stay distinguishable in field logs"
    );
    assert!(
        main_source.contains("fallback_level"),
        "renderer selection logs should continue exposing fallback_level so runtime fallback runs remain visible during baseline capture"
    );
}
