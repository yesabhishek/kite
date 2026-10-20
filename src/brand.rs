//! The shared, embedded application identity used by the UI and desktop window.
use iced::{Element, widget::svg};
use std::sync::OnceLock;

const BRAND_PNG: &[u8] = include_bytes!("../assets/package/kite.png");

/// Unboxed vector artwork keeps the workspace identity crisp at every zoom.
pub fn mark<'a, M: 'a>(size: f32, appearance: crate::appearance::Appearance) -> Element<'a, M> {
    static LIGHT: OnceLock<svg::Handle> = OnceLock::new();
    static DARK: OnceLock<svg::Handle> = OnceLock::new();
    let handle = if appearance == crate::appearance::Appearance::Porcelain {
        LIGHT.get_or_init(|| svg::Handle::from_memory(include_bytes!("../assets/brand/mark.svg")))
    } else {
        DARK.get_or_init(|| {
            svg::Handle::from_memory(include_bytes!("../assets/brand/mark-paper.svg"))
        })
    };
    svg(handle.clone()).width(size).height(size).into()
}

/// Outlined lettering gives every platform the same wordmark without a bundled font.
pub fn wordmark<'a, M: 'a>(
    width: f32,
    appearance: crate::appearance::Appearance,
) -> Element<'a, M> {
    static LIGHT: OnceLock<svg::Handle> = OnceLock::new();
    static DARK: OnceLock<svg::Handle> = OnceLock::new();
    let source = include_str!("../assets/brand/wordmark.svg");
    let handle = if appearance == crate::appearance::Appearance::Porcelain {
        LIGHT.get_or_init(|| svg::Handle::from_memory(source.as_bytes()))
    } else {
        DARK.get_or_init(|| {
            svg::Handle::from_memory(source.replace("#232323", "#F4F2EC").into_bytes())
        })
    };
    svg(handle.clone()).width(width).into()
}

/// Use the same identity for window managers and task switchers.
pub fn window_icon() -> Option<iced::window::Icon> {
    let pixels = ::image::load_from_memory_with_format(BRAND_PNG, ::image::ImageFormat::Png)
        .ok()?
        .resize(256, 256, ::image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (width, height) = pixels.dimensions();
    iced::window::icon::from_rgba(pixels.into_raw(), width, height).ok()
}
