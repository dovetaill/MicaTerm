#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSurface {
    CommandEntry,
    ActiveTab,
    RightPanelSegmentedControl,
    WelcomeState,
    CommandPalette,
}

pub fn signature_surfaces() -> &'static [SignatureSurface] {
    &[
        SignatureSurface::CommandEntry,
        SignatureSurface::ActiveTab,
        SignatureSurface::RightPanelSegmentedControl,
        SignatureSurface::WelcomeState,
        SignatureSurface::CommandPalette,
    ]
}
