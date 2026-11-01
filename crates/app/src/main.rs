use gpa_window::GpaWindow;
use gtk4::gdk;
use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Ensure the global Tokio runtime is initialized
    let _rt = gpa_sys_utils::runtime();
    let mcp_port = gpa_sys_utils::get_mcp_port();

    // Register compiled GResources
    gio::resources_register_include!("compiled.gresource")
        .expect("Failed to register compiled.gresource");

    let app = adw::Application::builder()
        .application_id("dev.boxxy.GpaIconsAnimator")
        .build();

    app.connect_startup(|_| {
        sourceview5::init();

        // Force dark studio color scheme by default (matching Shaper & Image 1)
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::PreferDark);

        let provider = gtk4::CssProvider::new();
        provider.load_from_resource("/dev/boxxy/GpaIconsAnimator/style.css");
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    app.connect_activate(move |app| {
        let win = GpaWindow::new(app, mcp_port);
        win.present();
    });

    app.run();
}
