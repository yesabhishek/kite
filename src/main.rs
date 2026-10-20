#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod appearance;
mod brand;
mod document;
mod editor;
mod gutter;
mod icons;
mod palette;
mod preview;
mod settings;
mod terminal;
mod workspace;

fn main() -> iced::Result {
    if std::env::args().any(|arg| arg == "--version") {
        println!("Kite {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let restore =
        !std::env::args().any(|arg| arg == "--no-restore" || arg == "--temporary-session");
    let initial_zoom = if restore {
        settings::Settings::load().app_zoom.scale_factor()
    } else {
        1.
    };
    let window = iced::window::Settings {
        size: (1280.0 / initial_zoom, 800.0 / initial_zoom).into(),
        min_size: Some((800.0, 520.0).into()),
        icon: brand::window_icon(),
        exit_on_close_request: false,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    let window = iced::window::Settings {
        platform_specific: iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        ..window
    };
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title(app::App::title)
        .subscription(app::App::subscription)
        .theme(app::App::theme)
        .scale_factor(app::App::scale_factor)
        .window(window)
        .run()
}
