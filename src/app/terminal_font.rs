//! Runtime registration for the heavier terminal font so startup stays on the lightweight path.

use std::sync::{Arc, OnceLock};

use slint::fontique_07::fontique;

static REGISTER_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
static SARASA_BYTES: &[u8] = include_bytes!("../../ui/fonts/SarasaTermSCNerd-Regular.ttf");

pub fn ensure_terminal_font_registered() -> Result<(), String> {
    REGISTER_RESULT
        .get_or_init(|| {
            let blob = fontique::Blob::new(Arc::new(SARASA_BYTES.to_vec()));
            let mut collection = slint::fontique_07::shared_collection();
            let fonts = collection.register_fonts(blob, None);
            if fonts.is_empty() {
                return Err("Sarasa terminal font registration produced no faces".to_string());
            }
            Ok(())
        })
        .clone()
}
