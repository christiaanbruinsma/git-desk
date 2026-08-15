pub mod alerts;
pub mod diff_view;
pub mod git_guide;
pub mod repository;
pub mod welcome;
pub mod window;
use gtk::prelude::*;

use crate::i18n::tr;

pub fn install_scrollbar_style() {
    use std::sync::Once;

    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
scrollbar slider {
    min-width: 6px;
    min-height: 6px;
    border-radius: 999px;
}
"#,
        );

        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
pub fn app_menu_button() -> gtk::MenuButton {
    let menu_model = gtk::gio::Menu::new();
    menu_model.append(Some(&tr("About Git Desk")), Some("app.about"));
    menu_model.append(Some(&tr("Quit")), Some("app.quit"));

    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(tr("Main Menu"))
        .build();
    button.add_css_class("flat");
    button.set_menu_model(Some(&menu_model));
    button
}
