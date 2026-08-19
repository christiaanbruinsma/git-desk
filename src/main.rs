mod app;
mod git;
mod i18n;
mod services;
mod ui;
mod validate;

fn main() -> gtk::glib::ExitCode {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .format_timestamp(None)
        .init();
    
    // Initialize theme support (including dark mode)
    ui::install_theme_support();
    
    i18n::init();
    app::run()
}
