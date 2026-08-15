use adw::prelude::*;
use gtk::gio;

use crate::i18n::tr;
use crate::ui::window::GitDeskWindow;

pub const BASE_APP_ID: &str = "io.github.christiaanbruinsma.GitDesk";
pub const APP_ID: &str = match option_env!("GIT_DESK_APP_ID") {
    Some(app_id) => app_id,
    None => BASE_APP_ID,
};
pub const APP_NAME: &str = "Git Desk";
pub const VERSION: &str = "0.9.0";

pub fn run() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();

    let about_action = gio::SimpleAction::new("about", None);
    let app_for_about = app.clone();
    about_action.connect_activate(move |_, _| {
        let dialog = adw::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon(APP_ID)
            .version(VERSION)
            .developer_name("Christiaan Bruinsma")
            .comments(tr("Easy to start. Powerful enough to stay."))
            .website("https://github.com/christiaanbruinsma/git-desk")
            .issue_url("https://github.com/christiaanbruinsma/git-desk/issues")
            .license_type(gtk::License::Gpl30)
            .build();
        let parent = app_for_about.active_window();
        dialog.present(parent.as_ref());
    });
    app.add_action(&about_action);

    let quit_action = gio::SimpleAction::new("quit", None);
    let app_for_quit = app.clone();
    quit_action.connect_activate(move |_, _| app_for_quit.quit());
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);

    app.connect_activate(|app| {
        let window = GitDeskWindow::new(app);
        window.present();
    });

    app.run()
}
