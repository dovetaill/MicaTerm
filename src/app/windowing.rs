#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    MicaAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAppearance {
    pub no_frame: bool,
    pub material: MaterialKind,
}

pub fn window_appearance() -> WindowAppearance {
    WindowAppearance {
        no_frame: true,
        material: MaterialKind::MicaAlt,
    }
}
