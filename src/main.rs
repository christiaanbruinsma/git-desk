mod app;
mod git;
mod i18n;
mod services;
mod ui;

fn main() -> gtk::glib::ExitCode {
    i18n::init();
    app::run()
}
