use mica_term::shell::signature::{SignatureSurface, signature_surfaces};

#[test]
fn signature_surfaces_match_the_curated_highlights_set() {
    assert_eq!(
        signature_surfaces(),
        &[
            SignatureSurface::CommandEntry,
            SignatureSurface::ActiveTab,
            SignatureSurface::RightPanelSegmentedControl,
            SignatureSurface::WelcomeState,
            SignatureSurface::CommandPalette,
        ]
    );
}
