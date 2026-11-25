//! Polychrome brand icon helpers rendered via GPUI `img()` (`Arc<Image>`) so multi-stop
//! gradients (Aurora Teal + Amber Pulse Cursor) preserve full RGBA fidelity.

use std::sync::{Arc, LazyLock};

use gpui_kit::{AnyElement, Image, ImageFormat, IntoElement as _, Styled as _, img, px};

static APP_ICON_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../assets/icons/png/mica-term-gpui-128.png").to_vec(),
    ))
});

static MARK_ICON_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../assets/icons/png/mica-term-gpui-mark-64.png").to_vec(),
    ))
});

pub fn brand_app_icon_element(size_px: f32) -> AnyElement {
    img(APP_ICON_IMAGE.clone())
        .size(px(size_px))
        .flex_shrink_0()
        .into_any_element()
}

pub fn brand_mark_icon_element(size_px: f32) -> AnyElement {
    img(MARK_ICON_IMAGE.clone())
        .size(px(size_px))
        .flex_shrink_0()
        .into_any_element()
}
