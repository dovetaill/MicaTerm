//! Smoke coverage for the selected runtime profile contract.

use std::fs;

#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;

use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn mainline_profile_requests_skia_selector_lock() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("skia"));
    assert!(profile.prefers_direct3d());
}

#[test]
fn main_entrypoint_reads_packaged_profile_and_dynamic_renderer() {
    let content = fs::read_to_string("src/main.rs").expect("read main");

    assert!(content.contains("AppRuntimeProfile::packaged()"));
    assert!(content.contains("renderer_selection_attempts(profile)"));
    assert!(content.contains(".backend_name(backend_name.into())"));
    assert!(content.contains(".renderer_name(attempt.renderer_mode.renderer_name().into())"));
    assert!(content.contains("selector = selector.require_d3d();"));
    assert!(content.contains("renderer_fallback_chain()"));
    assert!(content.contains("preferred_graphics_api()"));
    assert!(content.contains("attempt.renderer_mode.renderer_name()"));
    assert!(content.contains("fallback_level"));
    assert!(!content.contains("renderer_name(\"software\".into())"));
}

#[test]
fn run_with_profile_accepts_external_async_handle_for_ssh_services() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("pub fn run_with_profile(\n    profile: AppRuntimeProfile,\n    async_runtime_handle: tokio::runtime::Handle,"),
        "run_with_profile should explicitly accept the async runtime handle used by shell services"
    );
    assert!(
        content.contains("let session_bridge = build_session_bridge(")
            && content.contains("async_runtime_handle.clone(),"),
        "run_with_profile should thread the supplied runtime handle into session bridge construction"
    );
    assert!(
        !content.contains("    _async_runtime_handle: tokio::runtime::Handle,"),
        "the runtime handle should be consumed for ssh services instead of being ignored"
    );
}

#[test]
fn build_win_x64_wrapper_keeps_retained_native_as_the_only_live_windows_subsystem() {
    let content = fs::read_to_string("build-win-x64.sh").expect("read build wrapper");

    assert!(
        !content.contains(&retired_windows_subsystem::retired_kebab_name()),
        "build-win-x64.sh should stop documenting or exporting the retired Windows software presenter as a live packaged path"
    );
    assert!(
        !content.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"),
        "build-win-x64.sh should stop exporting a packaged terminal subsystem override once retained-native is the only supported Windows path"
    );
    assert!(
        content.contains("retained-native"),
        "build-win-x64.sh help text should continue describing retained-native as the live packaged Windows path"
    );
}

#[test]
fn bootstrap_logs_render_fallback_labels_without_terminal_subsystem_switching() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !content.contains("terminal_subsystem = profile.terminal_subsystem_mode_label()"),
        "bootstrap should stop logging a terminal subsystem label once Windows exposes only the retained-native path"
    );
    assert!(
        content.contains("fallback_render_mode = TerminalRenderMode::Bitmap.as_str()"),
        "bootstrap should keep fallback render mode labels visible in diagnostics when a presenter downgrade happens"
    );
}

#[test]
fn bootstrap_profile_source_keeps_host_surface_geometry_sync_hook() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("workspace_blocks_native_terminal_surface(window)"),
        "bootstrap should keep an explicit helper for modal states that truly block the native terminal surface"
    );
    assert!(
        bootstrap_source.contains("sync_workspace_native_terminal_surface_geometry"),
        "bootstrap should keep the workspace native terminal geometry sync hook so host-surface layout stays authoritative during overlay transitions"
    );
}

#[test]
fn native_surface_diagnostics_smoke_reports_child_host_relationship() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let diagnostics_source =
        fs::read_to_string("src/app/terminal_renderer/diagnostics.rs").expect("read diagnostics");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame");

    assert!(
        bootstrap_source.contains("window_scale_factor(window)"),
        "bootstrap should keep logging the live window scale factor so child-host geometry stays debuggable in physical pixels"
    );
    assert!(
        !bootstrap_source.contains("host_hwnd = diagnostics.host_hwnd.unwrap_or_default()")
            && !bootstrap_source
                .contains("host_surface_hwnd = diagnostics.host_surface_hwnd.unwrap_or_default()")
            && !bootstrap_source.contains(
                "host_surface_visible = diagnostics.host_surface_visible.unwrap_or(false)"
            )
            && !bootstrap_source
                .contains("host_surface_ready = diagnostics.host_surface_ready.unwrap_or(false)"),
        "bootstrap should not emit per-frame host/child HWND diagnostics logs once the retained-native trace spam is retired"
    );
    assert!(
        diagnostics_source.contains("pub host_hwnd: Option<isize>")
            && diagnostics_source.contains("pub host_surface_hwnd: Option<isize>"),
        "native surface diagnostics should expose dedicated host and host-surface HWND fields"
    );
    assert!(
        windows_frame_source.contains("pub fn native_surface_host_hwnd(")
            && windows_frame_source.contains("pub fn native_surface_host_surface_hwnd("),
        "windows frame helpers should expose stable accessors for host and host-surface HWND diagnostics"
    );
}

#[test]
fn terminal_completion_docs_state_current_core_and_migration_boundaries_honestly() {
    let audit_doc = fs::read_to_string(
        "docs/plans/2026-04-07-terminal-subsystem-completion-audit-and-corrective-design.md",
    )
    .expect("read completion audit design");

    assert!(
        audit_doc.contains("still drives the default terminal core"),
        "completion audit should state that WezTerm still drives the default core today"
    );
    assert!(
        audit_doc.contains("experimental adapter seam"),
        "completion audit should state that the Alacritty path is still experimental"
    );
    assert!(
        audit_doc.contains("architectural reference"),
        "completion audit should state that Rio is an architectural reference rather than migrated runtime code"
    );
}

#[test]
fn readme_states_current_terminal_core_and_reference_boundaries_honestly() {
    let readme = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        readme.contains("WezTerm-backed terminal core remains the shipped default today"),
        "readme should state that the shipped default core is still WezTerm-backed so the repo does not overclaim the migration status"
    );
    assert!(
        readme.contains("Alacritty adapter path is experimental"),
        "readme should state that the Alacritty path is still experimental"
    );
    assert!(
        readme.contains("Rio remains an architectural reference"),
        "readme should state that Rio is a reference rather than migrated runtime code"
    );
    assert!(
        readme.contains(
            "real `alacritty_terminal` core now exists behind the experimental adapter boundary"
        ),
        "readme should state that the experimental Alacritty path now binds to the real upstream core rather than staying a WezTerm wrapper seam"
    );
}

#[test]
fn readme_describes_retained_native_as_the_only_live_windows_subsystem() {
    let readme = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        !readme.contains(&format!(
            "{} presenter",
            retired_windows_subsystem::retired_kebab_name()
        )),
        "readme should stop describing the retired Windows software presenter as a live path once retained-native is the only supported Windows path"
    );
    assert!(
        !readme.contains(&retired_windows_subsystem::retired_rollout_env_snippet()),
        "readme should stop documenting a retired Windows subsystem override once that path is deleted"
    );
    assert!(
        readme.contains("retained-native"),
        "readme should describe retained-native as the live Windows terminal path"
    );
    assert!(
        readme.contains("Windows mainline") && readme.contains("Windows software compatibility"),
        "readme should keep both Windows package flavors documented while describing retained-native as the live subsystem"
    );
}

#[test]
fn terminal_renderer_readme_documents_runtime_fallback_diagnostics() {
    let content = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        !content.contains("selected terminal subsystem"),
        "readme should stop documenting a selected terminal subsystem diagnostic once Windows no longer exposes subsystem switching"
    );
    assert!(
        content.contains("requested render mode")
            && content.contains("active presenter mode")
            && content.contains("fallback transitions"),
        "readme should describe the runtime diagnostics emitted for requested render mode, active presenter mode, and fallback transitions"
    );
}

#[test]
fn current_cleanup_plan_describes_retained_native_only_windows_path() {
    let plan =
        fs::read_to_string("docs/plans/2026-04-11-default-retained-native-and-log-cleanup-plan.md")
            .expect("read retained-native cleanup plan");

    assert!(
        !plan.contains(&retired_windows_subsystem::retired_kebab_name()),
        "the current retained-native cleanup plan should stop describing the retired Windows software presenter as a live path"
    );
    assert!(
        !plan.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"),
        "the current retained-native cleanup plan should stop describing the removed packaged terminal subsystem override"
    );
    assert!(
        plan.contains("retained-native"),
        "the current retained-native cleanup plan should describe retained-native as the live Windows path"
    );
}
