use std::{path::PathBuf, rc::Rc};

use adw::prelude::*;
use gtk::{PackType, glib};

use crate::i18n::{tr, tr_args};
use crate::{
    git::backend::GitBackend,
    services::portal::resolve_host_path,
    ui::{
        alerts::{AlertEyebrow, apply_alert_eyebrow},
        git_guide::GitGuideView,
        install_scrollbar_style,
        repository::RepositoryView,
        welcome::{WelcomeView, remove_recent_confirmation},
    },
};

pub struct GitDeskWindow {
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    welcome: Rc<WelcomeView>,
    repository: std::cell::RefCell<Option<Rc<RepositoryView>>>,
}

impl GitDeskWindow {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        install_scrollbar_style();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(160)
            .build();

        let welcome = WelcomeView::new();
        stack.add_named(&welcome.root, Some("welcome"));
        stack.set_visible_child_name("welcome");

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(tr("Git Desk"))
            .default_width(1380)
            .default_height(860)
            .width_request(820)
            .height_request(600)
            .content(&stack)
            .build();

        let this = Rc::new(Self {
            window,
            stack,
            welcome,
            repository: std::cell::RefCell::new(None),
        });

        this.connect_welcome();
        this
    }

    pub fn present(&self) {
        self.window.present();
    }

    fn connect_welcome(self: &Rc<Self>) {
        let this = self.clone();
        if let Some(button) = find_named_button(
            &self.welcome.root.clone().upcast::<gtk::Widget>(),
            "open-project",
        ) {
            button.connect_clicked(move |_| this.choose_folder(false));
        }

        let this = self.clone();
        if let Some(button) = find_named_button(
            &self.welcome.root.clone().upcast::<gtk::Widget>(),
            "setup-git",
        ) {
            button.connect_clicked(move |_| this.choose_folder(true));
        }

        let this = self.clone();
        if let Some(button) = find_named_button(
            &self.welcome.root.clone().upcast::<gtk::Widget>(),
            "clone-repository",
        ) {
            button.connect_clicked(move |_| this.clone_repository_dialog());
        }

        let this = self.clone();
        if let Some(button) = find_named_button(
            &self.welcome.root.clone().upcast::<gtk::Widget>(),
            "open-git-guide",
        ) {
            button.connect_clicked(move |_| this.open_git_guide());
        }

        let this = self.clone();
        self.welcome.setup_button().connect_clicked(move |_| {
            if let Some(path) = this.welcome.pending_path() {
                this.init_repository(path);
            }
        });

        let this = self.clone();
        self.welcome.setup_cancel().connect_clicked(move |_| {
            this.welcome.hide_setup();
        });

        let this = self.clone();
        self.welcome
            .recent_list()
            .connect_row_activated(move |_, row| {
                if let Some(path) = row.tooltip_text() {
                    let path = PathBuf::from(path.as_str());
                    if path.is_dir() {
                        this.open_path(path);
                    } else {
                        this.show_missing_recent(path);
                    }
                }
            });
    }

    fn open_git_guide(self: &Rc<Self>) {
        if let Some(old) = self.stack.child_by_name("standalone-guide") {
            self.stack.remove(&old);
        }

        let guide = GitGuideView::new();

        let guide_split = adw::OverlaySplitView::new();
        guide_split.set_vexpand(true);
        guide_split.set_hexpand(true);
        guide_split.set_sidebar_position(PackType::End);
        guide_split.set_min_sidebar_width(300.0);
        guide_split.set_max_sidebar_width(440.0);
        guide_split.set_sidebar_width_fraction(0.28);
        guide_split.set_sidebar(Some(&guide.sidebar));
        guide_split.set_content(Some(&guide.root));
        guide_split.set_pin_sidebar(true);

        guide
            .sidebar_toggle
            .bind_property("active", &guide_split, "show-sidebar")
            .bidirectional()
            .sync_create()
            .build();

        guide.sidebar_toggle.set_visible(false);
        guide_split.set_show_sidebar(false);

        let split_for_stack = guide_split.clone();
        let toggle_for_stack = guide.sidebar_toggle.clone();
        guide.stack.connect_visible_child_name_notify(move |stack| {
            let detail_visible = stack.visible_child_name().as_deref() == Some("detail");
            toggle_for_stack.set_visible(detail_visible);
            if detail_visible {
                toggle_for_stack.set_active(true);
            } else {
                split_for_stack.set_show_sidebar(false);
            }
        });

        let split_for_outline = guide_split.clone();
        guide.outline_list.connect_row_activated(move |_, _| {
            if split_for_outline.is_collapsed() {
                split_for_outline.set_show_sidebar(false);
            }
        });

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("flat");
        back.set_tooltip_text(Some(&tr("Back to Git Desk")));
        header.pack_start(&back);
        header.set_title_widget(Some(&adw::WindowTitle::new(&tr("Git Guide"), "")));
        header.pack_end(&crate::ui::app_menu_button());
        root.append(&header);
        root.append(&guide_split);

        self.stack.add_named(&root, Some("standalone-guide"));
        self.stack.set_visible_child_name("standalone-guide");

        let this = self.clone();
        back.connect_clicked(move |_| {
            this.show_welcome();
            if let Some(child) = this.stack.child_by_name("standalone-guide") {
                this.stack.remove(&child);
            }
        });
    }

    fn clone_repository_dialog(self: &Rc<Self>) {
        let entry = gtk::Entry::builder()
            .placeholder_text(tr("https://github.com/owner/repository.git"))
            .activates_default(true)
            .build();

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Clone Repository"))
            .body(tr("Enter a Git repository URL. Git Desk will then ask where to create the cloned repository."))
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("continue", &tr("Continue"));
        dialog.set_response_appearance("continue", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("continue"));
        dialog.set_response_enabled("continue", false);

        let dialog_for_entry = dialog.clone();
        entry.connect_changed(move |entry| {
            dialog_for_entry.set_response_enabled("continue", !entry.text().trim().is_empty());
        });

        let window = self.window.clone();
        let this = self.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(Some(&window)).await.as_str() != "continue" {
                return;
            }

            let url = entry.text().trim().to_string();
            if url.is_empty() {
                return;
            }

            let folder_dialog = gtk::FileDialog::builder()
                .title(tr("Choose Location for Cloned Repository"))
                .modal(true)
                .build();
            let Ok(folder) = folder_dialog.select_folder_future(Some(&window)).await else {
                return;
            };
            let Some(parent) = folder.path() else {
                return;
            };
            let parent = resolve_host_path(parent).await;

            match GitBackend::clone_repository(url, parent).await {
                Ok(backend) => this.open_repository(backend.path().to_path_buf()),
                Err(error) => this.show_error(&error.to_string()),
            }
        });
    }

    fn choose_folder(self: &Rc<Self>, force_setup: bool) {
        let dialog = gtk::FileDialog::builder()
            .title(&if force_setup {
                tr("Choose Project Folder")
            } else {
                tr("Open Project")
            })
            .modal(true)
            .build();

        let window = self.window.clone();
        let this = self.clone();
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.select_folder_future(Some(&window)).await else {
                return;
            };
            let Some(path) = file.path() else {
                return;
            };

            if force_setup {
                this.inspect_for_setup(path);
            } else {
                this.open_path(path);
            }
        });
    }

    fn inspect_for_setup(self: &Rc<Self>, path: PathBuf) {
        let this = self.clone();
        glib::spawn_future_local(async move {
            let path = resolve_host_path(path).await;
            match GitBackend::discover(path.clone()).await {
                Ok(Some(root)) => this.open_repository(root),
                Ok(None) => this.welcome.show_setup(path),
                Err(_) => this.welcome.show_setup(path),
            }
        });
    }

    fn open_path(self: &Rc<Self>, path: PathBuf) {
        let this = self.clone();
        glib::spawn_future_local(async move {
            let original_path = path.clone();
            let path = resolve_host_path(path).await;
            if path != original_path {
                this.welcome.replace_recent_path(&original_path, &path);
            }
            match GitBackend::discover(path.clone()).await {
                Ok(Some(root)) => this.open_repository(root),
                Ok(None) => this.welcome.show_setup(path),
                Err(error) => this.show_error(&error.to_string()),
            }
        });
    }

    fn show_missing_recent(self: &Rc<Self>, path: PathBuf) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| tr("project"));
        let dialog = adw::AlertDialog::builder()
            .heading(tr("Project folder not found"))
            .body(tr_args(
                "Git Desk can’t find {name} at its saved location. The folder may have been moved or deleted.",
                &[("name", name.clone())],
            ))
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Warning);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("remove", &tr("Remove from Recent Projects"));
        dialog.add_response("locate", &tr("Locate Project…"));
        dialog.set_response_appearance("locate", adw::ResponseAppearance::Suggested);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("locate"));

        let window = self.window.clone();
        let this = self.clone();
        glib::spawn_future_local(async move {
            match dialog.choose_future(Some(&window)).await.as_str() {
                "locate" => this.locate_recent(path),
                "remove" => {
                    let confirmation = remove_recent_confirmation(&name);
                    if confirmation.choose_future(Some(&window)).await.as_str() == "remove" {
                        this.welcome.remove_recent(&path);
                    }
                }
                _ => {}
            }
        });
    }

    fn locate_recent(self: &Rc<Self>, old_path: PathBuf) {
        let dialog = gtk::FileDialog::builder()
            .title(tr("Locate Project"))
            .modal(true)
            .build();

        let window = self.window.clone();
        let this = self.clone();
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.select_folder_future(Some(&window)).await else {
                return;
            };
            let Some(new_path) = file.path() else {
                return;
            };
            let new_path = resolve_host_path(new_path).await;

            this.welcome.replace_recent_path(&old_path, &new_path);
        });
    }

    fn init_repository(self: &Rc<Self>, path: PathBuf) {
        let this = self.clone();
        glib::spawn_future_local(async move {
            match GitBackend::init(path).await {
                Ok(backend) => this.open_repository(backend.path().to_path_buf()),
                Err(error) => this.show_error(&error.to_string()),
            }
        });
    }

    fn open_repository(self: &Rc<Self>, path: PathBuf) {
        self.welcome.recents().add(&path);
        self.welcome.reload_recents();
        self.welcome.hide_setup();

        if let Some(old) = self.repository.borrow_mut().take() {
            self.stack.remove(&old.root);
        }

        let repository = RepositoryView::new(path);
        self.stack.add_named(&repository.root, Some("repository"));
        self.stack.set_visible_child_name("repository");
        repository.initial_load();

        // The back button is found through the widget tree because the repository
        // view intentionally exposes only its top-level widget.
        if let Some(back) = find_named_button(
            &repository.root.clone().upcast::<gtk::Widget>(),
            "back-to-projects",
        ) {
            let this = self.clone();
            back.connect_clicked(move |_| this.show_welcome());
        }

        *self.repository.borrow_mut() = Some(repository);
    }

    fn show_welcome(&self) {
        self.welcome.reload_recents();
        self.stack.set_visible_child_name("welcome");
    }

    fn show_error(&self, message: &str) {
        let dialog = adw::AlertDialog::builder()
            .heading(tr("Git Desk"))
            .body(message)
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Error);
        dialog.add_response("close", &tr("Close"));
        dialog.set_default_response(Some("close"));
        dialog.present(Some(&self.window));
    }
}

fn find_named_button(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    if let Ok(button) = widget.clone().downcast::<gtk::Button>()
        && button.widget_name() == name
    {
        return Some(button);
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find_named_button(&current, name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
