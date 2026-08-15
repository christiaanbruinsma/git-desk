use std::{path::PathBuf, rc::Rc, sync::Once};

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::i18n::{tr, tr_args};
use crate::services::recent::RecentProjects;
use crate::ui::alerts::{AlertEyebrow, apply_alert_eyebrow};

fn install_welcome_surface_style() {
    static STYLE: Once = Once::new();

    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
.recent-projects-container {
    border-radius: 12px;
}

.recent-projects-list,
.recent-projects-list > row {
    border-radius: 0;
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

pub struct WelcomeView {
    pub root: gtk::Box,
    recents: RecentProjects,
    recent_list: gtk::ListBox,
    setup_box: gtk::Box,
    setup_title: gtk::Label,
    setup_path: gtk::Label,
    setup_button: gtk::Button,
    setup_cancel: gtk::Button,
    pending_path: std::cell::RefCell<Option<PathBuf>>,
}

impl WelcomeView {
    pub fn new() -> Rc<Self> {
        install_welcome_surface_style();

        let root = gtk::Box::new(Orientation::Vertical, 0);

        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("Git Desk", "");
        header.set_title_widget(Some(&title));
        header.pack_end(&crate::ui::app_menu_button());
        root.append(&header);

        let clamp = adw::Clamp::builder()
            .maximum_size(900)
            .tightening_threshold(650)
            .build();

        let content = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(24)
            .margin_top(42)
            .margin_bottom(42)
            .margin_start(24)
            .margin_end(24)
            .vexpand(true)
            .build();

        let app_title = gtk::Label::builder()
            .label(tr("Git Desk"))
            .xalign(0.0)
            .build();
        app_title.add_css_class("title-1");

        let slogan = gtk::Label::builder()
            .label(tr("Easy to start. Powerful enough to stay."))
            .xalign(0.0)
            .build();
        slogan.add_css_class("title-4");
        slogan.add_css_class("dim-label");

        let identity = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();
        identity.append(&app_title);
        identity.append(&slogan);
        content.append(&identity);

        let actions = gtk::Box::new(Orientation::Horizontal, 12);
        actions.set_homogeneous(true);

        let open = action_button(
            "document-open-symbolic",
            &tr("Open Project"),
            &tr("Open an existing Git repository or project folder"),
        );
        let setup = action_button(
            "list-add-symbolic",
            &tr("Set Up Git"),
            &tr("Choose a project folder and start version control"),
        );
        let clone = action_button(
            "edit-copy-symbolic",
            &tr("Clone Repository"),
            &tr("Clone an existing Git repository to this computer"),
        );

        actions.append(&open);
        actions.append(&setup);
        actions.append(&clone);
        content.append(&actions);

        let guide_text = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();
        let guide_title = gtk::Label::builder()
            .label(tr("Git Guide"))
            .xalign(0.0)
            .build();
        guide_title.add_css_class("heading");
        let guide_subtitle = gtk::Label::builder()
            .label(tr("Learn Git concepts and workflows at your own pace."))
            .xalign(0.0)
            .wrap(true)
            .build();
        guide_subtitle.add_css_class("dim-label");
        guide_text.append(&guide_title);
        guide_text.append(&guide_subtitle);

        let guide_action = gtk::Label::new(Some(&tr("Open Git Guide")));
        guide_action.add_css_class("heading");
        guide_action.set_valign(Align::Center);

        let guide_content = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(18)
            .margin_top(14)
            .margin_bottom(14)
            .margin_start(16)
            .margin_end(16)
            .build();
        guide_content.append(&guide_text);
        guide_content.append(&guide_action);

        let guide_card = gtk::Button::new();
        guide_card.add_css_class("card");
        guide_card.set_widget_name("open-git-guide");
        guide_card.set_child(Some(&guide_content));
        content.append(&guide_card);

        let setup_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(12)
            .margin_end(12)
            .visible(false)
            .build();
        setup_box.add_css_class("card");

        let setup_content = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();

        let setup_title = gtk::Label::builder().xalign(0.0).build();
        setup_title.add_css_class("heading");
        let setup_path = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        setup_path.add_css_class("dim-label");

        let explanation = gtk::Label::builder()
            .label(tr("Git Desk will create repository metadata in a .git folder. Your existing project files are not replaced."))
            .xalign(0.0)
            .wrap(true)
            .build();

        let setup_actions = gtk::Box::new(Orientation::Horizontal, 8);
        let setup_button = gtk::Button::with_label(&tr("Set Up Git"));
        setup_button.add_css_class("suggested-action");
        let setup_cancel = gtk::Button::with_label(&tr("Cancel"));
        setup_actions.append(&setup_button);
        setup_actions.append(&setup_cancel);

        setup_content.append(&setup_title);
        setup_content.append(&setup_path);
        setup_content.append(&explanation);
        setup_content.append(&setup_actions);
        setup_box.append(&setup_content);
        content.append(&setup_box);

        let recent_title = gtk::Label::builder()
            .label(tr("Recent Projects"))
            .xalign(0.0)
            .build();
        recent_title.add_css_class("title-3");
        content.append(&recent_title);

        let recent_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();
        recent_list.add_css_class("boxed-list");
        recent_list.add_css_class("recent-projects-list");
        recent_list.set_show_separators(true);

        let recent_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .child(&recent_list)
            .build();

        let recent_container = gtk::Box::new(Orientation::Vertical, 0);
        recent_container.add_css_class("recent-projects-container");
        recent_container.set_overflow(gtk::Overflow::Hidden);
        recent_container.append(&recent_scroller);

        content.append(&recent_container);

        clamp.set_vexpand(true);
        clamp.set_child(Some(&content));
        root.append(&clamp);

        let view = Rc::new(Self {
            root,
            recents: RecentProjects::new(),
            recent_list,
            setup_box,
            setup_title,
            setup_path,
            setup_button,
            setup_cancel,
            pending_path: std::cell::RefCell::new(None),
        });

        view.reload_recents();

        // Selection is exposed through custom signals installed by the window.
        open.set_widget_name("open-project");
        setup.set_widget_name("setup-git");
        clone.set_widget_name("clone-repository");

        view
    }

    pub fn reload_recents(&self) {
        populate_recents(&self.recents, &self.recent_list);
    }

    pub fn remove_recent(&self, path: &std::path::Path) {
        self.recents.remove(path);
        self.reload_recents();
    }

    pub fn replace_recent_path(&self, old_path: &std::path::Path, new_path: &std::path::Path) {
        self.recents.replace_path(old_path, new_path);
        self.reload_recents();
    }

    pub fn show_setup(&self, path: PathBuf) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| tr("project"));
        self.setup_title
            .set_label(&tr_args("Set up Git for {name}", &[("name", name.clone())]));
        self.setup_path.set_label(&tr("Local project folder"));
        self.setup_path
            .set_tooltip_text(Some(&path.to_string_lossy()));
        *self.pending_path.borrow_mut() = Some(path);
        self.setup_box.set_visible(true);
    }

    pub fn hide_setup(&self) {
        self.setup_box.set_visible(false);
        self.pending_path.borrow_mut().take();
    }

    pub fn pending_path(&self) -> Option<PathBuf> {
        self.pending_path.borrow().clone()
    }

    pub fn setup_button(&self) -> gtk::Button {
        self.setup_button.clone()
    }

    pub fn setup_cancel(&self) -> gtk::Button {
        self.setup_cancel.clone()
    }

    pub fn recent_list(&self) -> gtk::ListBox {
        self.recent_list.clone()
    }

    pub fn recents(&self) -> RecentProjects {
        self.recents.clone()
    }
}

fn populate_recents(recents: &RecentProjects, recent_list: &gtk::ListBox) {
    while let Some(child) = recent_list.first_child() {
        recent_list.remove(&child);
    }

    let items = recents.load();
    if items.is_empty() {
        let row = adw::ActionRow::builder()
            .title(tr("No projects yet"))
            .subtitle(tr("Open a project or set up Git to get started."))
            .build();
        row.set_sensitive(false);
        recent_list.append(&row);
        return;
    }

    for item in items {
        let name = item
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| tr("Project"));
        let available = item.path.is_dir();
        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(&if available {
                tr("Local project folder")
            } else {
                tr("Project folder not found")
            })
            .activatable(true)
            .build();
        row.set_tooltip_text(Some(&item.path.to_string_lossy()));

        if !available {
            let warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
            warning.add_css_class("warning");
            warning.set_tooltip_text(Some(&tr("Project folder not found")));
            row.add_prefix(&warning);
        }

        let popover = gtk::Popover::new();

        let actions = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();

        if !available {
            let locate = gtk::Button::with_label(&tr("Locate Project…"));
            locate.add_css_class("flat");
            locate.set_halign(Align::Fill);

            let recents_for_locate = recents.clone();
            let list_for_locate = recent_list.clone();
            let old_path = item.path.clone();
            let locate_for_parent = locate.clone();
            let popover_for_locate = popover.clone();
            locate.connect_clicked(move |_| {
                let parent = locate_for_parent
                    .root()
                    .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
                popover_for_locate.popdown();

                let dialog = gtk::FileDialog::builder()
                    .title(tr("Locate Project"))
                    .modal(true)
                    .build();
                let recents = recents_for_locate.clone();
                let list = list_for_locate.clone();
                let old_path = old_path.clone();

                gtk::glib::spawn_future_local(async move {
                    let Ok(file) = dialog.select_folder_future(parent.as_ref()).await else {
                        return;
                    };
                    let Some(new_path) = file.path() else {
                        return;
                    };
                    recents.replace_path(&old_path, &new_path);
                    populate_recents(&recents, &list);
                });
            });
            actions.append(&locate);
        }

        let remove = gtk::Button::with_label(&tr("Remove from Recent Projects"));
        remove.add_css_class("destructive-action");
        remove.set_halign(Align::Fill);
        let recents_for_remove = recents.clone();
        let list_for_remove = recent_list.clone();
        let remove_path = item.path.clone();
        let remove_name = name.to_string();
        let remove_for_parent = remove.clone();
        let popover_for_remove = popover.clone();
        remove.connect_clicked(move |_| {
            let parent = remove_for_parent
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            popover_for_remove.popdown();

            let dialog = remove_recent_confirmation(&remove_name);
            let recents = recents_for_remove.clone();
            let list = list_for_remove.clone();
            let path = remove_path.clone();

            gtk::glib::spawn_future_local(async move {
                if dialog.choose_future(parent.as_ref()).await.as_str() != "remove" {
                    return;
                }
                recents.remove(&path);
                populate_recents(&recents, &list);
            });
        });
        actions.append(&remove);

        popover.set_child(Some(&actions));

        let menu = gtk::MenuButton::builder()
            .label(tr("Actions"))
            .valign(Align::Center)
            .tooltip_text(tr("Project actions"))
            .build();
        menu.add_css_class("flat");
        menu.set_popover(Some(&popover));
        row.add_suffix(&menu);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        recent_list.append(&row);
    }
}

pub(crate) fn remove_recent_confirmation(name: &str) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::builder()
        .heading(tr("Remove from Recent Projects?"))
        .body(tr_args(
            "Remove {name} from Git Desk’s Recent Projects list? This does not delete the project folder or any files.",
            &[("name", name.to_string())],
        ))
        .build();
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("remove", &tr("Remove"));
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
    dialog
}

fn action_button(icon: &str, title: &str, subtitle: &str) -> gtk::Button {
    let box_ = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .halign(Align::Center)
        .valign(Align::Center)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(12)
        .margin_end(12)
        .build();

    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(24);
    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("heading");
    let subtitle_label = gtk::Label::builder()
        .label(subtitle)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .max_width_chars(28)
        .build();
    subtitle_label.add_css_class("dim-label");
    subtitle_label.add_css_class("caption");

    box_.append(&image);
    box_.append(&title_label);
    box_.append(&subtitle_label);

    let button = gtk::Button::new();
    button.add_css_class("card");
    button.set_child(Some(&box_));
    button
}
