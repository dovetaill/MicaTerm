//! Source-level contract coverage for the memory reduction dead-code audit.

use std::fs;

const AUDIT_DOC: &str = "docs/plans/2026-06-09-memory-footprint-reduction/dead-code-audit.md";

#[test]
fn memory_dead_code_audit_doc_stays_candidate_only_with_explicit_proof_bar() {
    let audit = fs::read_to_string(AUDIT_DOC).expect("read dead-code audit doc");

    assert!(
        audit.contains("只做候选清单") && audit.contains("不马上删除"),
        "the dead-code audit should stay scoped to a candidate list instead of claiming that runtime code or dependencies are safe to delete immediately"
    );
    assert!(
        audit.contains("生产路径")
            && audit.contains("build features")
            && audit.contains("wrappers")
            && audit.contains("tests")
            && audit.contains("docs"),
        "the dead-code audit should spell out the proof bar for future deletions so runtime, packaging, test, and documentation references all have to be cleared first"
    );
    assert!(
        audit.contains("当前没有满足证明链的 runtime 删除候选"),
        "the audit should state plainly when no runtime deletion candidate has cleared the proof bar yet"
    );
}

#[test]
fn memory_dead_code_audit_doc_records_live_dependencies_and_low_value_followups() {
    let audit = fs::read_to_string(AUDIT_DOC).expect("read dead-code audit doc");

    for required in [
        "vendor/i-slint-renderer-skia",
        "vendor/i-slint-backend-winit",
        "fontdb",
        "windows-sys",
        "tests/support/retired_windows_subsystem.rs",
    ] {
        assert!(
            audit.contains(required),
            "the dead-code audit should record the reviewed candidate or blocker: {required}"
        );
    }

    assert!(
        audit.contains("不是候选") && audit.contains("待补证明"),
        "the audit should distinguish live dependencies that are not removal candidates from low-value cleanup follow-ups that still need proof"
    );
}
