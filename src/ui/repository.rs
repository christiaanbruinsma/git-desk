use std::{cell::RefCell, path::PathBuf, rc::Rc};

use adw::prelude::*;
use gtk::{Orientation, PackType, glib};

use crate::i18n::{ntr_args, tr, tr_args};

fn run_after_popover_closed<F>(popover: &gtk::Popover, action: F)
where
    F: FnOnce() + 'static,
{
    let action = Rc::new(RefCell::new(Some(action)));
    let action_for_closed = action.clone();
    let handler_id = Rc::new(RefCell::new(None));
    let handler_id_for_closed = handler_id.clone();

    let id = popover.connect_closed(move |popover| {
        let Some(action) = action_for_closed.borrow_mut().take() else {
            return;
        };

        if let Some(id) = handler_id_for_closed.borrow_mut().take() {
            popover.disconnect(id);
        }

        glib::idle_add_local_once(action);
    });
    *handler_id.borrow_mut() = Some(id);
    popover.popdown();
}
use crate::{
    git::{
        backend::{GitBackend, HistoryOperation, HistoryOperationKind, TagEntry},
        graph::{GraphRow, build_history_graph},
        models::{Branch, Change, ChangeArea, Commit, StashEntry},
    },
    services::watcher::RepositoryWatcher,
    ui::{
        alerts::{AlertEyebrow, apply_alert_eyebrow},
        diff_view::DiffView,
        git_guide::GitGuideView,
    },
};

pub struct RepositoryView {
    pub root: gtk::Box,
    backend: GitBackend,
    title: adw::WindowTitle,
    nav: gtk::ListBox,
    stack: gtk::Stack,
    content_stack: gtk::Stack,
    inspector_stack: gtk::Stack,
    inspector_split: adw::OverlaySplitView,
    git_guide: GitGuideView,

    changes_subtitle: gtk::Label,
    merge_group: gtk::Box,
    merge_status_row: adw::ActionRow,
    complete_merge_button: gtk::Button,
    abort_merge_button: gtk::Button,
    history_operation_group: gtk::Box,
    history_operation_status_row: adw::ActionRow,
    continue_history_operation_button: gtk::Button,
    skip_history_operation_button: gtk::Button,
    abort_history_operation_button: gtk::Button,
    outgoing_group: gtk::Box,
    outgoing_subtitle: gtk::Label,
    outgoing_list: gtk::ListBox,
    unstaged_list: gtk::ListBox,
    staged_list: gtk::ListBox,
    unstaged_group: gtk::Box,
    staged_group: gtk::Box,
    clean_label: gtk::Label,
    commit_buffer: gtk::TextBuffer,
    commit_editor: gtk::TextView,
    commit_button: gtk::Button,
    stage_all_button: gtk::Button,
    unstage_all_button: gtk::Button,

    history_subtitle: gtk::Label,
    history_list: gtk::ListBox,

    branches_subtitle: gtk::Label,
    local_branches: gtk::ListBox,
    remotes_list: gtk::ListBox,
    remote_branches: gtk::ListBox,
    new_branch_button: gtk::Button,
    add_remote_button: gtk::Button,

    stashes_subtitle: gtk::Label,
    stash_list: gtk::ListBox,
    stash_empty: gtk::Label,
    new_stash_button: gtk::Button,

    tags_subtitle: gtk::Label,
    tag_list: gtk::ListBox,
    tag_empty: gtk::Label,
    new_tag_button: gtk::Button,

    inspector_title: gtk::Label,
    inspector_subtitle: gtk::Label,
    inspector_commit_metadata: gtk::Box,
    inspector_message: gtk::Label,
    inspector_commit_actions: gtk::Box,
    edit_commit_message_button: gtk::Button,
    amend_commit_button: gtk::Button,
    undo_commit_button: gtk::Button,
    inspector_history_actions: gtk::Box,
    revert_commit_button: gtk::Button,
    cherry_pick_button: gtk::Button,
    inspector_stash_actions: gtk::Box,
    apply_stash_button: gtk::Button,
    pop_stash_button: gtk::Button,
    delete_stash_button: gtk::Button,
    inspector_tag_actions: gtk::Box,
    push_tag_button: gtk::Button,
    delete_tag_button: gtk::Button,
    inspector_empty: gtk::Box,
    inspector_body: gtk::Box,
    inspector_files: gtk::Box,
    diff_view: DiffView,
    toast_overlay: adw::ToastOverlay,

    fetch_button: gtk::Button,
    fetch_label: gtk::Label,
    pull_button: gtk::Button,
    pull_label: gtk::Label,
    push_button: gtk::Button,
    push_label: gtk::Label,

    current_status: RefCell<Option<crate::git::models::RepositoryStatus>>,
    history: RefCell<Vec<GraphRow>>,
    selected_change: RefCell<Option<Change>>,
    selected_history_commit: RefCell<Option<String>>,
    selected_outgoing_commit: RefCell<Option<String>>,
    outgoing_head_commit: RefCell<Option<String>>,
    selected_branch: RefCell<Option<String>>,
    selected_stash: RefCell<Option<StashEntry>>,
    selected_tag: RefCell<Option<TagEntry>>,
    watcher: RefCell<Option<RepositoryWatcher>>,
    commit_busy: RefCell<bool>,
    unpublished_action_busy: RefCell<bool>,
    stash_busy: RefCell<bool>,
    tag_busy: RefCell<bool>,
    merge_busy: RefCell<bool>,
    merge_in_progress: RefCell<bool>,
    merge_unresolved_count: RefCell<usize>,
    merge_conflicts_known: RefCell<bool>,
    history_action_busy: RefCell<bool>,
    history_operation: RefCell<Option<HistoryOperation>>,
    history_operation_unresolved_count: RefCell<usize>,
    history_operation_conflicts_known: RefCell<bool>,
    sync_busy: RefCell<Option<&'static str>>,
}

impl RepositoryView {
    pub fn new(path: PathBuf) -> Rc<Self> {
        let backend = GitBackend::new(path.clone());
        let project_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| tr("Repository"));

        let root = gtk::Box::new(Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();

        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("flat");
        back.set_tooltip_text(Some(&tr("Back to projects")));
        back.set_widget_name("back-to-projects");
        header.pack_start(&back);

        let title = adw::WindowTitle::new(&project_name, &tr("Local project folder"));
        header.set_title_widget(Some(&title));

        let open_folder = gtk::Button::from_icon_name("document-open-symbolic");
        open_folder.add_css_class("flat");
        open_folder.set_tooltip_text(Some(&tr("Open Repository Folder")));

        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.add_css_class("flat");
        refresh.set_tooltip_text(Some(&tr("Refresh repository")));
        header.pack_end(&crate::ui::app_menu_button());
        header.pack_end(&refresh);
        header.pack_end(&open_folder);

        let icon_theme =
            gtk::gdk::Display::default().map(|display| gtk::IconTheme::for_display(&display));
        let sync_button = |label: &str, icon_names: &[&str]| {
            let content = gtk::Box::new(Orientation::Horizontal, 6);
            content.set_halign(gtk::Align::Center);

            if let Some(icon_name) = icon_names.iter().copied().find(|icon_name| {
                icon_theme
                    .as_ref()
                    .is_some_and(|theme| theme.has_icon(icon_name))
            }) {
                content.append(&gtk::Image::from_icon_name(icon_name));
            }

            let label = gtk::Label::new(Some(label));
            // Optical alignment: the symbolic icons sit visually a touch lower
            // than the text even though GTK centers the row geometrically.
            label.set_margin_top(1);
            content.append(&label);

            let button = gtk::Button::new();
            button.set_child(Some(&content));
            (button, label)
        };

        let (push_button, push_label) = sync_button(&tr("Push"), &["go-up-symbolic"]);
        let (pull_button, pull_label) = sync_button(&tr("Pull"), &["go-down-symbolic"]);
        let (fetch_button, fetch_label) = sync_button(
            &tr("Fetch"),
            &["network-receive-symbolic", "view-refresh-symbolic"],
        );
        let sync_box = gtk::Box::new(Orientation::Horizontal, 0);
        sync_box.add_css_class("linked");
        for button in [&fetch_button, &pull_button, &push_button] {
            button.set_sensitive(false);
            sync_box.append(button);
        }
        header.pack_end(&sync_box);

        root.append(&header);

        let nav = make_nav();
        let sidebar = gtk::Box::new(Orientation::Vertical, 0);
        sidebar.set_size_request(220, -1);
        sidebar.set_margin_top(4);
        sidebar.set_margin_bottom(4);
        sidebar.append(&nav);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(140)
            .hexpand(true)
            .vexpand(true)
            .build();

        let (
            changes_page,
            changes_subtitle,
            merge_group,
            merge_status_row,
            complete_merge_button,
            abort_merge_button,
            history_operation_group,
            history_operation_status_row,
            continue_history_operation_button,
            skip_history_operation_button,
            abort_history_operation_button,
            outgoing_group,
            outgoing_subtitle,
            outgoing_list,
            unstaged_group,
            unstaged_list,
            staged_group,
            staged_list,
            clean_label,
            commit_buffer,
            commit_editor,
            commit_button,
            stage_all_button,
            unstage_all_button,
        ) = build_changes_page();
        let (history_page, history_subtitle, history_list) = build_history_page();
        let (
            branches_page,
            branches_subtitle,
            local_branches,
            remotes_list,
            remote_branches,
            new_branch_button,
            add_remote_button,
        ) = build_branches_page();
        let (stashes_page, stashes_subtitle, stash_list, stash_empty, new_stash_button) =
            build_stashes_page();
        let (tags_page, tags_subtitle, tag_list, tag_empty, new_tag_button) = build_tags_page();
        let git_guide = GitGuideView::new();

        stack.add_named(&changes_page, Some("changes"));
        stack.add_named(&history_page, Some("history"));
        stack.add_named(&branches_page, Some("branches"));
        stack.add_named(&stashes_page, Some("stashes"));
        stack.add_named(&tags_page, Some("tags"));
        stack.set_visible_child_name("changes");

        let inspector_title = gtk::Label::builder()
            .label(tr("Inspector"))
            .xalign(0.0)
            .margin_top(14)
            .margin_start(16)
            .margin_end(16)
            .build();
        inspector_title.add_css_class("title-4");

        let inspector_subtitle = gtk::Label::builder()
            .label(tr("Select an item to inspect it."))
            .xalign(0.0)
            .wrap(true)
            .margin_start(16)
            .margin_end(16)
            .build();
        inspector_subtitle.add_css_class("dim-label");

        let inspector_commit_metadata = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .visible(false)
            .build();

        let inspector_message = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .visible(false)
            .build();

        let inspector_commit_actions = gtk::Box::new(Orientation::Horizontal, 6);
        inspector_commit_actions.set_halign(gtk::Align::Start);
        inspector_commit_actions.set_visible(false);

        let edit_commit_message_button = gtk::Button::with_label(&tr("Edit Message…"));
        edit_commit_message_button.set_tooltip_text(Some(&tr(
            "Change the message of this unpublished commit without including staged changes.",
        )));
        inspector_commit_actions.append(&edit_commit_message_button);

        let amend_commit_button = gtk::Button::with_label(&tr("Amend"));
        amend_commit_button.set_tooltip_text(Some(&tr(
            "Add all currently staged changes to this unpublished commit.",
        )));
        inspector_commit_actions.append(&amend_commit_button);

        let undo_commit_button = gtk::Button::with_label(&tr("Undo Commit"));
        undo_commit_button.add_css_class("destructive-action");
        undo_commit_button.set_tooltip_text(Some(&tr(
            "Remove this unpublished commit from the branch and return its contents to staging.",
        )));
        inspector_commit_actions.append(&undo_commit_button);

        let inspector_history_actions = gtk::Box::new(Orientation::Horizontal, 6);
        inspector_history_actions.set_halign(gtk::Align::Start);
        inspector_history_actions.set_visible(false);

        let revert_commit_button = gtk::Button::with_label(&tr("Revert Commit…"));
        revert_commit_button.set_tooltip_text(Some(&tr(
            "Create a new commit that reverses this commit on the current branch.",
        )));
        inspector_history_actions.append(&revert_commit_button);

        let cherry_pick_button = gtk::Button::with_label(&tr("Cherry-pick…"));
        cherry_pick_button.set_tooltip_text(Some(&tr(
            "Apply this commit on top of the current branch as a new commit.",
        )));
        inspector_history_actions.append(&cherry_pick_button);

        let inspector_empty = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .margin_start(20)
            .margin_end(20)
            .build();

        let empty_title = gtk::Label::new(Some(&tr("Nothing selected")));
        empty_title.add_css_class("title-4");
        let empty_description = gtk::Label::builder()
            .label(tr(
                "Select a change, commit, branch, stash, or tag to inspect its details.",
            ))
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        empty_description.add_css_class("dim-label");
        inspector_empty.append(&empty_title);
        inspector_empty.append(&empty_description);

        let inspector_body = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(14)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .visible(false)
            .build();

        let inspector_stash_actions = gtk::Box::new(Orientation::Horizontal, 6);
        inspector_stash_actions.set_halign(gtk::Align::Start);
        inspector_stash_actions.set_visible(false);

        let apply_stash_button = gtk::Button::with_label(&tr("Apply"));
        apply_stash_button.set_tooltip_text(Some(&tr(
            "Apply this stash to the working tree and keep it in the stash list.",
        )));
        inspector_stash_actions.append(&apply_stash_button);

        let pop_stash_button = gtk::Button::with_label(&tr("Pop"));
        pop_stash_button.set_tooltip_text(Some(&tr(
            "Apply this stash and remove it from the stash list when successful.",
        )));
        inspector_stash_actions.append(&pop_stash_button);

        let delete_stash_button = gtk::Button::with_label(&tr("Delete Stash"));
        delete_stash_button.add_css_class("destructive-action");
        delete_stash_button.set_tooltip_text(Some(&tr(
            "Permanently remove this saved stash without applying it.",
        )));
        inspector_stash_actions.append(&delete_stash_button);

        let inspector_tag_actions = gtk::Box::new(Orientation::Horizontal, 6);
        inspector_tag_actions.set_halign(gtk::Align::Start);
        inspector_tag_actions.set_visible(false);

        let push_tag_button = gtk::Button::with_label(&tr("Push Tag…"));
        push_tag_button.set_tooltip_text(Some(&tr(
            "Push exactly this tag to one configured remote without force-updating an existing remote tag.",
        )));
        inspector_tag_actions.append(&push_tag_button);

        let delete_tag_button = gtk::Button::with_label(&tr("Delete Tag…"));
        delete_tag_button.add_css_class("destructive-action");
        delete_tag_button.set_tooltip_text(Some(&tr(
            "Delete this local tag. Tags already present on remotes are not deleted.",
        )));
        inspector_tag_actions.append(&delete_tag_button);

        inspector_body.append(&inspector_commit_metadata);
        inspector_body.append(&inspector_message);
        inspector_body.append(&inspector_commit_actions);
        inspector_body.append(&inspector_history_actions);
        inspector_body.append(&inspector_stash_actions);
        inspector_body.append(&inspector_tag_actions);

        let inspector_files = gtk::Box::new(Orientation::Vertical, 6);
        inspector_body.append(&inspector_files);

        let diff_view = DiffView::new();
        inspector_body.append(&diff_view.widget);

        let inspector_box = gtk::Box::new(Orientation::Vertical, 8);
        inspector_box.set_size_request(360, -1);
        inspector_box.append(&inspector_title);
        inspector_box.append(&inspector_subtitle);
        inspector_box.append(&inspector_empty);
        inspector_box.append(&inspector_body);

        let inspector_split = adw::OverlaySplitView::new();
        inspector_split.set_vexpand(true);
        inspector_split.set_hexpand(true);
        inspector_split.set_sidebar_position(PackType::End);
        inspector_split.set_min_sidebar_width(300.0);
        inspector_split.set_max_sidebar_width(440.0);
        inspector_split.set_sidebar_width_fraction(0.28);

        let inspector_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(120)
            .hexpand(true)
            .vexpand(true)
            .build();
        inspector_stack.add_named(&inspector_box, Some("git"));
        inspector_stack.add_named(&git_guide.sidebar, Some("guide"));
        inspector_stack.set_visible_child_name("git");
        inspector_split.set_sidebar(Some(&inspector_stack));
        inspector_split.set_pin_sidebar(true);

        let inspector_width_button = gtk::ToggleButton::new();
        inspector_width_button.set_icon_name("panel-right-symbolic");

        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme = gtk::IconTheme::for_display(&display);
            if !icon_theme.has_icon("panel-right-symbolic") {
                eprintln!("[Git Desk icon] MISSING: panel-right-symbolic");
            }
        }
        inspector_width_button.set_tooltip_text(Some(&tr("Expand Inspector")));
        inspector_width_button.set_halign(gtk::Align::End);
        inspector_width_button.set_valign(gtk::Align::Start);
        inspector_width_button.set_margin_top(12);
        inspector_width_button.set_margin_end(8);
        inspector_width_button.add_css_class("flat");

        let inspector_split_for_width = inspector_split.clone();
        let inspector_width_animations = Rc::new(RefCell::new(Vec::<adw::TimedAnimation>::new()));
        let inspector_width_animations_for_toggle = inspector_width_animations.clone();
        inspector_width_button.connect_toggled(move |button| {
            for animation in inspector_width_animations_for_toggle.borrow_mut().drain(..) {
                animation.pause();
            }

            let (target_fraction, target_min_width, target_max_width, tooltip) =
                if button.is_active() {
                    (0.60, 600.0, 900.0, tr("Restore Inspector Width"))
                } else {
                    (0.28, 300.0, 440.0, tr("Expand Inspector"))
                };

            let animation_specs = [
                (
                    "sidebar-width-fraction",
                    inspector_split_for_width.sidebar_width_fraction(),
                    target_fraction,
                ),
                (
                    "min-sidebar-width",
                    inspector_split_for_width.min_sidebar_width(),
                    target_min_width,
                ),
                (
                    "max-sidebar-width",
                    inspector_split_for_width.max_sidebar_width(),
                    target_max_width,
                ),
            ];

            let mut animations = inspector_width_animations_for_toggle.borrow_mut();
            for (property_name, from, to) in animation_specs {
                let target =
                    adw::PropertyAnimationTarget::new(&inspector_split_for_width, property_name);
                let animation =
                    adw::TimedAnimation::new(&inspector_split_for_width, from, to, 200, target);
                animation.play();
                animations.push(animation);
            }

            button.set_tooltip_text(Some(&tooltip));
        });

        let content_overlay = gtk::Overlay::new();
        content_overlay.set_child(Some(&stack));
        content_overlay.add_overlay(&inspector_width_button);

        let content_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(140)
            .hexpand(true)
            .vexpand(true)
            .build();
        content_stack.add_named(&content_overlay, Some("repository"));
        content_stack.add_named(&git_guide.root, Some("git-guide"));
        content_stack.set_visible_child_name("repository");
        inspector_split.set_content(Some(&content_stack));

        let main_split = adw::OverlaySplitView::new();
        main_split.set_vexpand(true);
        main_split.set_hexpand(true);
        main_split.set_min_sidebar_width(190.0);
        main_split.set_max_sidebar_width(260.0);
        main_split.set_sidebar_width_fraction(0.18);
        main_split.set_sidebar(Some(&sidebar));
        main_split.set_content(Some(&inspector_split));
        main_split.set_pin_sidebar(true);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_vexpand(true);
        toast_overlay.set_hexpand(true);
        toast_overlay.set_child(Some(&main_split));
        root.append(&toast_overlay);

        let view = Rc::new(Self {
            root,
            backend,
            title,
            nav,
            stack,
            content_stack,
            inspector_stack,
            inspector_split,
            git_guide,
            changes_subtitle,
            merge_group,
            merge_status_row,
            complete_merge_button,
            abort_merge_button,
            history_operation_group,
            history_operation_status_row,
            continue_history_operation_button,
            skip_history_operation_button,
            abort_history_operation_button,
            outgoing_group,
            outgoing_subtitle,
            outgoing_list,
            unstaged_list,
            staged_list,
            unstaged_group,
            staged_group,
            clean_label,
            commit_buffer,
            commit_editor,
            commit_button,
            stage_all_button,
            unstage_all_button,
            history_subtitle,
            history_list,
            branches_subtitle,
            local_branches,
            remotes_list,
            remote_branches,
            new_branch_button,
            add_remote_button,
            stashes_subtitle,
            stash_list,
            stash_empty,
            new_stash_button,
            tags_subtitle,
            tag_list,
            tag_empty,
            new_tag_button,
            inspector_title,
            inspector_subtitle,
            inspector_commit_metadata,
            inspector_message,
            inspector_commit_actions,
            edit_commit_message_button,
            amend_commit_button,
            undo_commit_button,
            inspector_history_actions,
            revert_commit_button,
            cherry_pick_button,
            inspector_stash_actions,
            apply_stash_button,
            pop_stash_button,
            delete_stash_button,
            inspector_tag_actions,
            push_tag_button,
            delete_tag_button,
            inspector_empty,
            inspector_body,
            inspector_files,
            diff_view,
            toast_overlay,
            fetch_button,
            fetch_label,
            pull_button,
            pull_label,
            push_button,
            push_label,
            current_status: RefCell::new(None),
            history: RefCell::new(Vec::new()),
            selected_change: RefCell::new(None),
            selected_history_commit: RefCell::new(None),
            selected_outgoing_commit: RefCell::new(None),
            outgoing_head_commit: RefCell::new(None),
            selected_branch: RefCell::new(None),
            selected_stash: RefCell::new(None),
            selected_tag: RefCell::new(None),
            watcher: RefCell::new(None),
            commit_busy: RefCell::new(false),
            unpublished_action_busy: RefCell::new(false),
            stash_busy: RefCell::new(false),
            tag_busy: RefCell::new(false),
            merge_busy: RefCell::new(false),
            merge_in_progress: RefCell::new(false),
            merge_unresolved_count: RefCell::new(0),
            merge_conflicts_known: RefCell::new(true),
            history_action_busy: RefCell::new(false),
            history_operation: RefCell::new(None),
            history_operation_unresolved_count: RefCell::new(0),
            history_operation_conflicts_known: RefCell::new(true),
            sync_busy: RefCell::new(None),
        });

        view.connect_signals(refresh, open_folder);
        view.install_watcher();
        view
    }

    fn connect_signals(self: &Rc<Self>, refresh: gtk::Button, open_folder: gtk::Button) {
        let this = self.clone();
        refresh.connect_clicked(move |_| this.refresh_all());

        let this = self.clone();
        open_folder.connect_clicked(move |_| this.open_repository_folder());

        let refresh_for_nav = refresh.clone();
        let open_folder_for_nav = open_folder.clone();
        let this = self.clone();
        self.nav.connect_row_selected(move |_, row| {
            let Some(row) = row else {
                return;
            };

            let git_actions_visible = row.index() != 5;
            this.fetch_button.set_visible(git_actions_visible);
            this.pull_button.set_visible(git_actions_visible);
            this.push_button.set_visible(git_actions_visible);
            refresh_for_nav.set_visible(git_actions_visible);
            open_folder_for_nav.set_visible(git_actions_visible);

            this.selected_change.borrow_mut().take();
            this.unstaged_list.unselect_all();
            this.staged_list.unselect_all();
            this.selected_history_commit.borrow_mut().take();
            this.history_list.unselect_all();
            this.selected_outgoing_commit.borrow_mut().take();
            this.outgoing_list.unselect_all();
            this.selected_branch.borrow_mut().take();
            this.local_branches.unselect_all();
            this.remote_branches.unselect_all();
            this.selected_stash.borrow_mut().take();
            this.stash_list.unselect_all();
            this.selected_tag.borrow_mut().take();
            this.tag_list.unselect_all();
            this.clear_inspector();

            match row.index() {
                0 => this.show_git_workspace("changes"),
                1 => this.show_git_workspace("history"),
                2 => this.show_git_workspace("branches"),
                3 => this.show_git_workspace("stashes"),
                4 => this.show_git_workspace("tags"),
                5 => {
                    this.content_stack.set_visible_child_name("git-guide");
                    this.inspector_stack.set_visible_child_name("guide");
                    this.sync_guide_context_sidebar();
                }
                _ => {}
            }
        });
        self.nav.select_row(self.nav.row_at_index(0).as_ref());

        let this = self.clone();
        self.history_list.connect_row_activated(move |_, row| {
            let index = row.index();
            if index <= 0 {
                return;
            }

            let commit = this
                .history
                .borrow()
                .get((index - 1) as usize)
                .map(|graph_row| graph_row.commit.clone());

            if let Some(commit) = commit {
                this.toggle_commit(commit);
            }
        });

        let this = self.clone();
        self.git_guide
            .stack
            .connect_visible_child_name_notify(move |_| this.sync_guide_context_sidebar());

        self.git_guide
            .sidebar_toggle
            .bind_property("active", &self.inspector_split, "show-sidebar")
            .bidirectional()
            .build();

        let this = self.clone();
        self.git_guide
            .outline_list
            .connect_row_activated(move |_, _| {
                if this.inspector_split.is_collapsed() {
                    this.inspector_split.set_show_sidebar(false);
                }
            });

        let this = self.clone();
        self.commit_button.connect_clicked(move |_| this.commit());

        let this = self.clone();
        self.commit_buffer
            .connect_changed(move |_| this.update_commit_button_state());

        let commit_keys = gtk::EventControllerKey::new();
        let this = self.clone();
        commit_keys.connect_key_pressed(move |_, key, _, state| {
            let ctrl_enter = state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && (key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter);
            if ctrl_enter {
                this.commit();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.commit_editor.add_controller(commit_keys);

        let this = self.clone();
        self.complete_merge_button
            .connect_clicked(move |_| this.complete_merge());

        let this = self.clone();
        self.abort_merge_button
            .connect_clicked(move |_| this.confirm_abort_merge());

        let this = self.clone();
        self.continue_history_operation_button
            .connect_clicked(move |_| this.continue_history_operation());

        let this = self.clone();
        self.skip_history_operation_button
            .connect_clicked(move |_| this.skip_cherry_pick());

        let this = self.clone();
        self.abort_history_operation_button
            .connect_clicked(move |_| this.confirm_abort_history_operation());

        let this = self.clone();
        self.edit_commit_message_button
            .connect_clicked(move |_| this.edit_unpublished_commit_message());

        let this = self.clone();
        self.amend_commit_button
            .connect_clicked(move |_| this.confirm_amend_unpublished_commit());

        let this = self.clone();
        self.undo_commit_button
            .connect_clicked(move |_| this.confirm_undo_unpublished_commit());

        let this = self.clone();
        self.revert_commit_button
            .connect_clicked(move |_| this.request_history_action(HistoryOperationKind::Revert));

        let this = self.clone();
        self.cherry_pick_button.connect_clicked(move |_| {
            this.request_history_action(HistoryOperationKind::CherryPick)
        });

        let this = self.clone();
        self.stage_all_button
            .connect_clicked(move |_| this.stage_all());

        let this = self.clone();
        self.unstage_all_button
            .connect_clicked(move |_| this.unstage_all());

        let this = self.clone();
        self.new_branch_button
            .connect_clicked(move |_| this.create_branch_dialog());

        let this = self.clone();
        self.add_remote_button
            .connect_clicked(move |_| this.add_remote_dialog());

        let this = self.clone();
        self.new_stash_button
            .connect_clicked(move |_| this.create_stash_dialog());

        let this = self.clone();
        self.apply_stash_button
            .connect_clicked(move |_| this.apply_selected_stash());

        let this = self.clone();
        self.pop_stash_button
            .connect_clicked(move |_| this.confirm_pop_selected_stash());

        let this = self.clone();
        self.delete_stash_button
            .connect_clicked(move |_| this.confirm_delete_selected_stash());

        let this = self.clone();
        self.new_tag_button
            .connect_clicked(move |_| this.create_tag_dialog());

        let this = self.clone();
        self.push_tag_button
            .connect_clicked(move |_| this.push_selected_tag());

        let this = self.clone();
        self.delete_tag_button
            .connect_clicked(move |_| this.confirm_delete_selected_tag());

        let this = self.clone();
        self.fetch_button
            .connect_clicked(move |_| this.sync("fetch"));

        let this = self.clone();
        self.pull_button.connect_clicked(move |_| this.sync("pull"));

        let this = self.clone();
        self.push_button.connect_clicked(move |_| this.push());
    }

    fn show_git_workspace(&self, page: &str) {
        self.content_stack.set_visible_child_name("repository");
        self.stack.set_visible_child_name(page);
        self.inspector_stack.set_visible_child_name("git");
        self.inspector_split.set_show_sidebar(true);
    }

    fn sync_guide_context_sidebar(&self) {
        if self.content_stack.visible_child_name().as_deref() != Some("git-guide") {
            return;
        }

        self.inspector_stack.set_visible_child_name("guide");
        let detail_visible = self.git_guide.stack.visible_child_name().as_deref() == Some("detail");
        self.git_guide.sidebar_toggle.set_visible(detail_visible);
        if detail_visible {
            self.git_guide.sidebar_toggle.set_active(true);
        } else {
            self.inspector_split.set_show_sidebar(false);
        }
    }

    fn open_repository_folder(self: &Rc<Self>) {
        let file = gtk::gio::File::for_path(self.backend.path());
        let launcher = gtk::FileLauncher::new(Some(&file));
        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();

        glib::spawn_future_local(async move {
            if let Err(error) = launcher.launch_future(parent.as_ref()).await {
                this.show_files_error_dialog(
                    &tr("Could Not Open Repository Folder"),
                    error.to_string(),
                )
                .await;
            }
        });
    }

    fn show_change_in_files(self: &Rc<Self>, change: Change) {
        let path = self.backend.path().join(&change.path);
        let show_parent = change.status == "deleted" || !path.exists();
        let target = if show_parent {
            path.parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| self.backend.path().to_path_buf())
        } else {
            path
        };

        let file = gtk::gio::File::for_path(target);
        let launcher = gtk::FileLauncher::new(Some(&file));
        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();

        glib::spawn_future_local(async move {
            let result = if show_parent {
                launcher.launch_future(parent.as_ref()).await
            } else {
                launcher
                    .open_containing_folder_future(parent.as_ref())
                    .await
            };

            if let Err(error) = result {
                this.show_files_error_dialog(&tr("Could Not Open Files"), error.to_string())
                    .await;
            }
        });
    }

    async fn show_files_error_dialog(&self, heading: &str, body: String) {
        let dialog = adw::AlertDialog::builder()
            .heading(tr(heading))
            .body(&body)
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Error);
        dialog.add_response("close", &tr("Close"));
        dialog.set_default_response(Some("close"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let _ = dialog.choose_future(parent.as_ref()).await;
    }

    fn install_watcher(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let callback = Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.refresh_all();
            }
        });
        *self.watcher.borrow_mut() = Some(RepositoryWatcher::new(
            self.backend.path().to_path_buf(),
            callback,
        ));
    }

    pub fn refresh_all(self: &Rc<Self>) {
        self.load_status();
        self.load_merge_state();
        self.load_history_operation_state();
        self.load_history();
        self.load_branches();
        self.load_remotes();
        self.load_stashes();
        self.load_tags();
        if let Some(watcher) = self.watcher.borrow().as_ref() {
            watcher.rebuild();
        }
    }

    pub fn initial_load(self: &Rc<Self>) {
        self.refresh_all();
    }

    fn load_status(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();

        glib::spawn_future_local(async move {
            match backend.status().await {
                Ok(status) => {
                    let unstaged = status
                        .changes
                        .iter()
                        .filter(|change| change.area == ChangeArea::Unstaged)
                        .count();
                    let staged = status
                        .changes
                        .iter()
                        .filter(|change| change.area == ChangeArea::Staged)
                        .count();

                    this.title.set_subtitle(&branch_subtitle(
                        &status.branch,
                        status.upstream.as_deref(),
                        status.ahead,
                        status.behind,
                        status.unborn,
                        status.detached,
                    ));

                    let subtitle = if status.changes.is_empty() {
                        tr("Working tree clean")
                    } else {
                        ntr_args(
                            "{unstaged} change · {staged} ready to commit",
                            "{unstaged} changes · {staged} ready to commit",
                            unstaged as u64,
                            &[
                                ("unstaged", unstaged.to_string()),
                                ("staged", staged.to_string()),
                            ],
                        )
                    };
                    this.changes_subtitle.set_label(&subtitle);

                    this.render_changes(&status.changes);
                    let branch = status.branch.clone();
                    let upstream = status.upstream.clone();
                    let ahead = status.ahead;
                    let unborn = status.unborn;
                    let detached = status.detached;
                    *this.current_status.borrow_mut() = Some(status);
                    this.update_commit_button_state();
                    this.update_merge_controls();
                    this.update_history_operation_controls();
                    this.update_unpublished_commit_actions();
                    this.update_history_commit_actions();
                    this.update_tag_action_state();
                    this.load_outgoing_commits(branch, upstream, ahead, unborn, detached);
                }
                Err(error) => {
                    this.current_status.borrow_mut().take();
                    this.update_commit_button_state();
                    this.update_history_operation_controls();
                    this.update_history_commit_actions();
                    this.update_tag_action_state();
                    this.changes_subtitle.set_label(&error.to_string());
                }
            }
        });
    }

    fn render_changes(self: &Rc<Self>, changes: &[Change]) {
        clear_list(&self.unstaged_list);
        clear_list(&self.staged_list);

        let selected_change = self.selected_change.borrow().clone();
        if selected_change
            .as_ref()
            .is_some_and(|selected| !changes.contains(selected))
        {
            self.selected_change.borrow_mut().take();
            self.clear_inspector();
        }

        let unstaged: Vec<_> = changes
            .iter()
            .filter(|change| change.area == ChangeArea::Unstaged)
            .cloned()
            .collect();
        let staged: Vec<_> = changes
            .iter()
            .filter(|change| change.area == ChangeArea::Staged)
            .cloned()
            .collect();

        let has_conflicts = unstaged.iter().any(|change| change.status == "conflicted");

        self.clean_label.set_visible(changes.is_empty());
        self.unstaged_group.set_visible(!unstaged.is_empty());
        self.staged_group.set_visible(!staged.is_empty());
        self.update_commit_button_state();
        self.stage_all_button
            .set_sensitive(!unstaged.is_empty() && !has_conflicts);
        let stage_all_tooltip = if has_conflicts {
            tr("Resolve conflicted files individually before staging all changes.")
        } else {
            tr("Stage all working-tree changes")
        };
        self.stage_all_button
            .set_tooltip_text(Some(&stage_all_tooltip));
        self.unstage_all_button.set_sensitive(!staged.is_empty());

        for change in unstaged {
            let is_selected = selected_change.as_ref() == Some(&change);
            let row = self.change_row(change);
            self.unstaged_list.append(&row);
            if is_selected {
                self.unstaged_list.select_row(Some(&row));
            }
        }
        for change in staged {
            let is_selected = selected_change.as_ref() == Some(&change);
            let row = self.change_row(change);
            self.staged_list.append(&row);
            if is_selected {
                self.staged_list.select_row(Some(&row));
            }
        }

        // A filesystem watcher refresh can leave the selected Change model
        // identical while the file contents — and therefore its diff — have
        // changed. Re-inspect the still-selected change so the Inspector stays
        // in sync with the working tree.
        if let Some(selected) = selected_change.filter(|selected| changes.contains(selected)) {
            self.inspect_change(selected);
        }
    }

    fn change_row(self: &Rc<Self>, change: Change) -> adw::ActionRow {
        let subtitle = if change.status == "conflicted" {
            if let Some(old) = &change.old_path {
                tr_args(
                    "Conflict · resolve the file, then mark resolved · from {old}",
                    &[("old", old.clone())],
                )
            } else {
                tr("Conflict · resolve the file, then mark resolved")
            }
        } else if let Some(old) = &change.old_path {
            tr_args(
                "{status} · from {old}",
                &[("status", capitalize(&change.status)), ("old", old.clone())],
            )
        } else {
            capitalize(&change.status)
        };

        let row = adw::ActionRow::builder()
            .title(&change.path)
            .subtitle(&subtitle)
            .activatable(true)
            .build();
        if change.status == "conflicted" {
            row.add_css_class("warning");
        }

        // Every selectable change exposes the same context-aware action set.
        // The visible Stage/Unstage button remains for the primary workflow;
        // right-click opens this popover as a native row context menu.
        let actions = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();

        let stage_label = match change.area {
            ChangeArea::Staged => tr("Unstage"),
            ChangeArea::Unstaged if change.status == "conflicted" => tr("Mark Resolved"),
            ChangeArea::Unstaged => tr("Stage"),
        };
        let context_stage = gtk::Button::with_label(&stage_label);
        if change.status == "conflicted" {
            context_stage.set_tooltip_text(Some(&tr(
                "Stage this resolved file so Git can continue the current operation.",
            )));
        }
        context_stage.add_css_class("flat");
        context_stage.set_halign(gtk::Align::Fill);
        actions.append(&context_stage);

        let destructive = if change.area == ChangeArea::Unstaged && change.status != "conflicted" {
            let label = if change.status == "untracked" {
                tr("Delete Untracked File…")
            } else {
                tr("Discard Changes…")
            };
            let button = gtk::Button::with_label(&label);
            button.add_css_class("destructive-action");
            button.set_halign(gtk::Align::Fill);
            actions.append(&button);
            Some(button)
        } else {
            None
        };

        actions.append(&gtk::Separator::new(Orientation::Horizontal));

        let show_label = if change.status == "deleted" {
            tr("Show Parent Folder in Files")
        } else {
            tr("Show in Files")
        };
        let show_in_files = gtk::Button::with_label(&show_label);
        show_in_files.add_css_class("flat");
        show_in_files.set_halign(gtk::Align::Fill);
        actions.append(&show_in_files);

        let popover = gtk::Popover::new();
        popover.set_child(Some(&actions));

        let menu = gtk::MenuButton::builder()
            .label(tr("Actions"))
            .valign(gtk::Align::Center)
            .tooltip_text(tr("Change actions"))
            .build();
        menu.add_css_class("flat");
        menu.set_popover(Some(&popover));
        row.add_suffix(&menu);

        let action = gtk::Button::with_label(&stage_label);
        if change.status == "conflicted" {
            action.set_tooltip_text(Some(&tr(
                "Stage this resolved file so Git can continue the current operation.",
            )));
            action.add_css_class("suggested-action");
        }
        action.set_valign(gtk::Align::Center);
        row.add_suffix(&action);

        let this = self.clone();
        let selected = change.clone();
        row.connect_activated(move |_| {
            this.toggle_change(selected.clone());
        });

        let this = self.clone();
        let action_change = change.clone();
        let action_button = action.clone();
        action.connect_clicked(move |_| {
            if action_change.status == "conflicted" {
                this.confirm_mark_resolved(action_change.clone(), action_button.clone());
            } else {
                this.apply_change_stage_action(action_change.clone(), action_button.clone());
            }
        });

        let this = self.clone();
        let context_change = change.clone();
        let context_button = context_stage.clone();
        let context_popover = popover.clone();
        context_stage.connect_clicked(move |_| {
            if context_change.status == "conflicted" {
                let this = this.clone();
                let context_change = context_change.clone();
                let context_button = context_button.clone();
                run_after_popover_closed(&context_popover, move || {
                    this.confirm_mark_resolved(context_change, context_button);
                });
            } else {
                context_popover.popdown();
                this.apply_change_stage_action(context_change.clone(), context_button.clone());
            }
        });

        if let Some(destructive) = destructive {
            let this = self.clone();
            let discard_change = change.clone();
            let discard_popover = popover.clone();
            destructive.connect_clicked(move |_| {
                discard_popover.popdown();
                this.confirm_discard(discard_change.clone());
            });
        }

        let this = self.clone();
        let files_change = change.clone();
        let files_popover = popover.clone();
        show_in_files.connect_clicked(move |_| {
            files_popover.popdown();
            this.show_change_in_files(files_change.clone());
        });

        // Secondary click selects/inspects the row first, then opens the same
        // action popover used by the explicit Actions button.
        let secondary_click = gtk::GestureClick::new();
        secondary_click.set_button(gtk::gdk::BUTTON_SECONDARY);
        let this = self.clone();
        let context_row = row.clone();
        let context_change = change.clone();
        let context_menu = menu.clone();
        secondary_click.connect_pressed(move |gesture, _, _, _| {
            let _ = gesture.set_state(gtk::EventSequenceState::Claimed);
            this.select_change_row(context_change.clone(), &context_row);
            context_menu.popup();
        });
        row.add_controller(secondary_click);

        row
    }

    fn confirm_mark_resolved(self: &Rc<Self>, change: Change, button: gtk::Button) {
        if change.status != "conflicted" || change.area != ChangeArea::Unstaged {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr_args(
                "Mark {path} as resolved?",
                &[("path", change.path.clone())],
            ))
            .body(tr(
                "This will stage the file as resolved so Git can continue the current operation. Make sure all conflict markers have been resolved.",
            ))
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Warning);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("resolve", &tr("Mark Resolved"));
        dialog.set_response_appearance("resolve", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        glib::spawn_future_local(async move {
            if dialog.choose_future(parent.as_ref()).await.as_str() != "resolve" {
                return;
            }

            this.apply_change_stage_action(change, button);
        });
    }

    fn apply_change_stage_action(self: &Rc<Self>, change: Change, button: gtk::Button) {
        button.set_sensitive(false);
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let was_conflict = change.status == "conflicted";
            let path = change.path.clone();
            let result = if change.area == ChangeArea::Staged {
                backend.unstage(change.path, change.old_path).await
            } else {
                backend.stage(change.path, change.old_path).await
            };
            match result {
                Ok(()) if was_conflict => {
                    this.show_toast(tr_args(
                        "Marked {path} as resolved",
                        &[("path", path.to_string())],
                    ));
                }
                Ok(()) => {}
                Err(error) => this.changes_subtitle.set_label(&error.to_string()),
            }
            this.refresh_all();
        });
    }

    fn stage_all(self: &Rc<Self>) {
        self.stage_all_button.set_sensitive(false);
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            if let Err(error) = backend.stage_all().await {
                this.changes_subtitle.set_label(&error.to_string());
            }
            this.refresh_all();
        });
    }

    fn unstage_all(self: &Rc<Self>) {
        self.unstage_all_button.set_sensitive(false);
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            if let Err(error) = backend.unstage_all().await {
                this.changes_subtitle.set_label(&error.to_string());
            }
            this.refresh_all();
        });
    }

    fn confirm_discard(self: &Rc<Self>, change: Change) {
        let untracked = change.status == "untracked";
        let heading = if untracked {
            tr_args("Delete {path}?", &[("path", change.path.clone())])
        } else {
            tr_args(
                "Discard changes to {path}?",
                &[("path", change.path.clone())],
            )
        };
        let body = if untracked {
            tr("This file is not tracked by Git. Deleting it cannot be undone by Git Desk.")
        } else {
            tr(
                "The working-tree version will be restored from Git’s index. Any staged version is kept. This cannot be undone by Git Desk.",
            )
        };
        let response_label = if untracked {
            tr("Delete File")
        } else {
            tr("Discard Changes")
        };

        let dialog = adw::AlertDialog::builder()
            .heading(&heading)
            .body(&body)
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("discard", &response_label);
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "discard" {
                return;
            }

            if let Err(error) = backend.discard_worktree(change.path, untracked).await {
                this.changes_subtitle.set_label(&error.to_string());
            }
            this.refresh_all();
        });
    }

    fn select_change_row(self: &Rc<Self>, change: Change, row: &adw::ActionRow) {
        *self.selected_change.borrow_mut() = Some(change.clone());
        match change.area {
            ChangeArea::Unstaged => {
                self.staged_list.unselect_all();
                self.unstaged_list.select_row(Some(row));
            }
            ChangeArea::Staged => {
                self.unstaged_list.unselect_all();
                self.staged_list.select_row(Some(row));
            }
        }
        self.inspect_change(change);
    }

    fn toggle_change(self: &Rc<Self>, change: Change) {
        let already_selected = self.selected_change.borrow().as_ref() == Some(&change);

        if already_selected {
            self.selected_change.borrow_mut().take();
            self.unstaged_list.unselect_all();
            self.staged_list.unselect_all();
            self.clear_inspector();
            return;
        }

        *self.selected_change.borrow_mut() = Some(change.clone());
        match change.area {
            ChangeArea::Unstaged => self.staged_list.unselect_all(),
            ChangeArea::Staged => self.unstaged_list.unselect_all(),
        }
        self.inspect_change(change);
    }

    fn inspect_change(self: &Rc<Self>, change: Change) {
        self.selected_history_commit.borrow_mut().take();
        self.history_list.unselect_all();
        self.selected_outgoing_commit.borrow_mut().take();
        self.outgoing_list.unselect_all();
        self.selected_branch.borrow_mut().take();
        self.local_branches.unselect_all();
        self.remote_branches.unselect_all();
        self.selected_stash.borrow_mut().take();
        self.stash_list.unselect_all();
        self.selected_tag.borrow_mut().take();
        self.tag_list.unselect_all();
        self.inspector_empty.set_visible(false);
        self.inspector_body.set_visible(true);
        clear_box(&self.inspector_files);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_commit_metadata.set_visible(false);
        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);
        self.inspector_commit_actions.set_visible(false);
        self.inspector_history_actions.set_visible(false);
        self.inspector_stash_actions.set_visible(false);
        self.inspector_tag_actions.set_visible(false);
        self.inspector_title.set_label(&change.path);
        self.inspector_subtitle.set_visible(true);
        let area = match change.area {
            ChangeArea::Staged => tr("Ready to commit"),
            ChangeArea::Unstaged => tr("Working tree"),
        };
        self.inspector_subtitle
            .set_label(&format!("{} · {}", capitalize(&change.status), area,));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend
                .working_diff(
                    change.path,
                    change.area == ChangeArea::Staged,
                    change.status == "untracked",
                )
                .await
            {
                Ok(patch) => this.diff_view.set_patch(&patch),
                Err(error) => this.diff_view.set_plain_text(&error.to_string()),
            }
        });
    }

    fn commit(self: &Rc<Self>) {
        if *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        if !self.commit_repository_state_allows() {
            if self.detached_head() {
                self.show_toast(tr("Create or switch to a local branch before committing."));
            }
            return;
        }

        let message = self.commit_message();
        if message.trim().is_empty() || !self.staged_group.is_visible() {
            return;
        }

        *self.commit_busy.borrow_mut() = true;
        self.update_commit_button_state();
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_stash_action_state();
        self.update_tag_action_state();
        let this = self.clone();
        let backend = self.backend.clone();

        glib::spawn_future_local(async move {
            match backend.commit(message).await {
                Ok(()) => this.commit_buffer.set_text(""),
                Err(error) => this.changes_subtitle.set_label(&error.to_string()),
            }
            *this.commit_busy.borrow_mut() = false;
            this.update_commit_button_state();
            this.update_history_commit_actions();
            this.update_history_operation_controls();
            this.update_stash_action_state();
            this.update_tag_action_state();
            this.refresh_all();
        });
    }

    fn commit_message(&self) -> String {
        let start = self.commit_buffer.start_iter();
        let end = self.commit_buffer.end_iter();
        self.commit_buffer.text(&start, &end, true).to_string()
    }

    fn detached_head(&self) -> bool {
        self.current_status
            .borrow()
            .as_ref()
            .is_some_and(|status| status.detached)
    }

    fn commit_repository_state_allows(&self) -> bool {
        self.current_status
            .borrow()
            .as_ref()
            .is_some_and(|status| !status.detached)
    }

    fn update_commit_composer_state(&self) {
        let merge_in_progress = *self.merge_in_progress.borrow();
        let history_operation_active = self.history_operation_active();
        let detached = self.detached_head();
        let repository_allows_commit = self.commit_repository_state_allows();
        let sensitive = repository_allows_commit && !merge_in_progress && !history_operation_active;

        self.commit_editor.set_sensitive(sensitive);
        if detached {
            let hint = tr("Create or switch to a local branch before committing.");
            self.commit_editor.set_tooltip_text(Some(&hint));
            self.commit_button.set_tooltip_text(Some(&hint));
        } else {
            self.commit_editor
                .set_tooltip_text(Some(&tr("Enter adds a new line · Ctrl+Enter commits")));
            self.commit_button
                .set_tooltip_text(Some(&tr("Commit staged changes (Ctrl+Enter)")));
        }
        eprintln!(
            "[Git Desk commit-composer diag] sensitive={sensitive} detached={detached} merge={merge_in_progress} history-operation={history_operation_active}"
        );
    }

    fn update_commit_button_state(&self) {
        let has_message = !self.commit_message().trim().is_empty();
        self.commit_button.set_sensitive(
            self.commit_repository_state_allows()
                && self.staged_group.is_visible()
                && has_message
                && !*self.commit_busy.borrow()
                && !*self.unpublished_action_busy.borrow()
                && !*self.stash_busy.borrow()
                && !*self.tag_busy.borrow()
                && !*self.merge_busy.borrow()
                && !*self.merge_in_progress.borrow()
                && !*self.history_action_busy.borrow()
                && !self.history_operation_active(),
        );
    }

    fn selected_unpublished_head(&self) -> Option<String> {
        let selected = self.selected_outgoing_commit.borrow().clone()?;
        if self.outgoing_head_commit.borrow().as_deref() == Some(selected.as_str()) {
            Some(selected)
        } else {
            None
        }
    }

    fn selected_history_commit_model(&self) -> Option<Commit> {
        let selected = self.selected_history_commit.borrow().clone()?;
        self.history
            .borrow()
            .iter()
            .find(|row| row.commit.id == selected)
            .map(|row| row.commit.clone())
    }

    fn history_operation_active(&self) -> bool {
        self.history_operation.borrow().is_some()
    }

    fn update_history_commit_actions(&self) {
        let selected = self.selected_history_commit_model();
        let visible = selected.is_some();
        let merge_commit = selected
            .as_ref()
            .is_some_and(|commit| commit.parents.len() > 1);
        let status_ready =
            self.current_status.borrow().as_ref().is_some_and(|status| {
                !status.unborn && !status.detached && status.changes.is_empty()
            });
        let busy = *self.history_action_busy.borrow()
            || self.history_operation_active()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some();
        let sensitive = visible && !merge_commit && status_ready && !busy;

        self.inspector_history_actions.set_visible(visible);
        self.revert_commit_button.set_sensitive(sensitive);
        self.cherry_pick_button.set_sensitive(sensitive);

        let tooltip = if merge_commit {
            tr(
                "Merge commits require choosing an explicit mainline parent. Git Desk does not guess that parent.",
            )
        } else if !status_ready {
            tr(
                "Switch to a normal branch with a clean working tree before applying a history action.",
            )
        } else if self.history_operation_active() {
            tr("Finish or abort the current history operation first.")
        } else {
            tr("Create a new commit on the current branch without rewriting existing history.")
        };
        self.revert_commit_button.set_tooltip_text(Some(&tooltip));
        let cherry_pick_tooltip = if merge_commit {
            tr(
                "Merge commits require choosing an explicit mainline parent. Git Desk does not guess that parent.",
            )
        } else if !status_ready {
            tr(
                "Switch to a normal branch with a clean working tree before cherry-picking a commit.",
            )
        } else if self.history_operation_active() {
            tr("Finish or abort the current history operation first.")
        } else {
            tr("Apply this commit on top of the current branch as a new commit.")
        };
        self.cherry_pick_button
            .set_tooltip_text(Some(&cherry_pick_tooltip));
    }

    fn update_unpublished_commit_actions(&self) {
        let is_head = self.selected_unpublished_head().is_some();
        let busy = *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some();
        let has_staged = self.current_status.borrow().as_ref().is_some_and(|status| {
            status
                .changes
                .iter()
                .any(|change| change.area == ChangeArea::Staged)
        });

        self.inspector_commit_actions.set_visible(is_head);
        self.edit_commit_message_button
            .set_sensitive(is_head && !busy);
        self.amend_commit_button
            .set_sensitive(is_head && has_staged && !busy);
        self.undo_commit_button.set_sensitive(is_head && !busy);
        let amend_tooltip = if has_staged {
            tr("Add all currently staged changes to this unpublished commit.")
        } else {
            tr("Stage changes first to amend this unpublished commit.")
        };
        self.amend_commit_button
            .set_tooltip_text(Some(&amend_tooltip));
    }

    fn set_unpublished_action_busy(&self, busy: bool) {
        *self.unpublished_action_busy.borrow_mut() = busy;
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_stash_action_state();
        self.update_tag_action_state();
        self.update_commit_button_state();

        if busy {
            self.fetch_button.set_sensitive(false);
            self.pull_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
            self.pull_button.remove_css_class("suggested-action");
            self.push_button.remove_css_class("suggested-action");
        }
    }

    fn edit_unpublished_commit_message(self: &Rc<Self>) {
        let Some(expected_head) = self.selected_unpublished_head() else {
            return;
        };
        if *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let message = match backend.commit_message(expected_head.clone()).await {
                Ok(message) => message,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Load Commit Message"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };

            if this.selected_unpublished_head().as_deref() != Some(expected_head.as_str()) {
                return;
            }

            let editor = gtk::TextView::builder()
                .wrap_mode(gtk::WrapMode::WordChar)
                .top_margin(8)
                .bottom_margin(8)
                .left_margin(8)
                .right_margin(8)
                .build();
            let buffer = editor.buffer();
            buffer.set_text(&message);

            let scroller = gtk::ScrolledWindow::builder()
                .hexpand(true)
                .min_content_height(120)
                .max_content_height(240)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .child(&editor)
                .build();

            let dialog = adw::AlertDialog::builder()
                .heading(tr("Edit Commit Message"))
                .body(tr("Change the message of this unpublished commit. Currently staged changes will not be included."))
                .build();
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("save", &tr("Save Message"));
            dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("save"));
            dialog.set_extra_child(Some(&scroller));
            dialog.set_response_enabled("save", !message.trim().is_empty());

            let dialog_for_buffer = dialog.clone();
            buffer.connect_changed(move |buffer| {
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let text = buffer.text(&start, &end, true);
                dialog_for_buffer.set_response_enabled("save", !text.trim().is_empty());
            });

            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "save" {
                return;
            }

            let start = buffer.start_iter();
            let end = buffer.end_iter();
            let new_message = buffer.text(&start, &end, true).to_string();
            if new_message.trim().is_empty() {
                return;
            }

            this.set_unpublished_action_busy(true);
            match backend
                .amend_commit_message(expected_head, new_message)
                .await
            {
                Ok(()) => this.show_toast(tr("Commit message updated")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Edit Commit"), error.to_string())
                        .await;
                }
            }
            this.set_unpublished_action_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_amend_unpublished_commit(self: &Rc<Self>) {
        let Some(expected_head) = self.selected_unpublished_head() else {
            return;
        };
        let has_staged = self.current_status.borrow().as_ref().is_some_and(|status| {
            status
                .changes
                .iter()
                .any(|change| change.area == ChangeArea::Staged)
        });
        if !has_staged
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Amend Unpublished Commit?"))
            .body(tr("Add all currently staged changes to this commit? Its commit ID will change. Nothing will be pushed automatically."))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("amend", &tr("Amend Commit"));
        dialog.set_response_appearance("amend", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("amend"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "amend" {
                return;
            }

            this.set_unpublished_action_busy(true);
            match backend.amend_staged_changes(expected_head).await {
                Ok(()) => this.show_toast(tr("Staged changes amended into commit")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Amend Commit"), error.to_string())
                        .await;
                }
            }
            this.set_unpublished_action_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_undo_unpublished_commit(self: &Rc<Self>) {
        let Some(expected_head) = self.selected_unpublished_head() else {
            return;
        };
        if *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Undo Unpublished Commit?"))
            .body(tr("Remove this commit from the branch and return its contents to the staging area? No files will be deleted. Existing staged changes remain staged."))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("undo", &tr("Undo Commit"));
        dialog.set_response_appearance("undo", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "undo" {
                return;
            }

            this.set_unpublished_action_busy(true);
            match backend.undo_head_commit(expected_head).await {
                Ok(()) => this.show_toast(tr("Commit undone — changes returned to staging")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Undo Commit"), error.to_string())
                        .await;
                }
            }
            this.set_unpublished_action_busy(false);
            this.refresh_all();
        });
    }

    fn set_history_action_busy(&self, busy: bool) {
        *self.history_action_busy.borrow_mut() = busy;
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_merge_controls();
        self.update_commit_button_state();
        self.update_unpublished_commit_actions();
        self.update_stash_action_state();
        self.update_tag_action_state();

        if busy {
            self.new_branch_button.set_sensitive(false);
            self.fetch_button.set_sensitive(false);
            self.pull_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
            self.pull_button.remove_css_class("suggested-action");
            self.push_button.remove_css_class("suggested-action");
        }
    }

    fn request_history_action(self: &Rc<Self>, kind: HistoryOperationKind) {
        let Some(commit) = self.selected_history_commit_model() else {
            return;
        };
        if commit.parents.len() > 1
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.history_operation().await {
                Ok(None) => {}
                Ok(Some(_)) => {
                    this.show_git_warning_dialog(
                    &tr("History Operation Already in Progress"),
                        tr("Finish or abort the current Revert/Cherry-pick operation in Changes before starting another one."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check History State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            match backend.merge_in_progress().await {
                Ok(false) => {}
                Ok(true) => {
                    this.show_git_warning_dialog(
                        &tr("Merge in Progress"),
                        tr("Complete or abort the current merge before applying a history action."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check Merge State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };

            if status.detached || status.unborn {
                this.show_git_warning_dialog(
                    &tr("Cannot Apply History Action Here"),
                    tr("Switch to a normal local branch with at least one commit before reverting or cherry-picking."),
                )
                .await;
                return;
            }
            if !status.changes.is_empty() {
                this.show_git_warning_dialog(
                    &tr("Commit or Stash Changes First"),
                    tr("Git Desk starts Revert and Cherry-pick from a clean working tree. Commit your work or save it in Stashes, then try again."),
                )
                .await;
                return;
            }

            if kind == HistoryOperationKind::Revert {
                match backend.commit_is_ancestor_of_head(commit.id.clone()).await {
                    Ok(true) => {}
                    Ok(false) => {
                        this.show_git_notice_dialog(
                    &tr("Commit Is Not on the Current Branch"),
                            tr("Git Desk only reverts commits that are part of the current branch history. Use Cherry-pick when you want to apply a commit from another branch."),
                        )
                        .await;
                        return;
                    }
                    Err(error) => {
                        this.show_git_error_dialog(
                            &tr("Could Not Verify Commit Ancestry"),
                            error.to_string(),
                        )
                        .await;
                        return;
                    }
                }
            }

            if kind == HistoryOperationKind::CherryPick {
                match backend.head_commit_id().await {
                    Ok(Some(head)) if head == commit.id => {
                        this.show_git_notice_dialog(
                    &tr("Commit Is Already HEAD"),
                            tr("The selected commit is already the current commit, so there is nothing to cherry-pick."),
                        )
                        .await;
                        return;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        this.show_git_error_dialog(
                            &tr("Could Not Verify Current Commit"),
                            error.to_string(),
                        )
                        .await;
                        return;
                    }
                }
            }

            let short = commit.id.chars().take(8).collect::<String>();
            let (heading, body, response, response_label) = match kind {
                HistoryOperationKind::Revert => (
                    tr_args(
                        "Revert ‘{subject}’?",
                        &[("subject", commit.subject.clone())],
                    ),
                    tr_args(
                        "Create a new commit on '{branch}' that reverses {subject} ({short})? Existing history will not be rewritten. If conflicts occur, Git Desk will keep the Revert open in Changes so you can resolve or abort it.",
                        &[
                            ("branch", status.branch.clone()),
                            ("subject", commit.subject.clone()),
                            ("short", short.clone()),
                        ],
                    ),
                    "revert",
                    tr("Revert Commit"),
                ),
                HistoryOperationKind::CherryPick => (
                    tr_args(
                        "Cherry-pick ‘{subject}’?",
                        &[("subject", commit.subject.clone())],
                    ),
                    tr_args(
                        "Apply {subject} ({short}) on top of '{branch}' as a new commit? Existing history will not be rewritten. If conflicts occur, Git Desk will keep the Cherry-pick open in Changes so you can resolve or abort it.",
                        &[
                            ("subject", commit.subject.clone()),
                            ("short", short.clone()),
                            ("branch", status.branch.clone()),
                        ],
                    ),
                    "pick",
                    tr("Cherry-pick Commit"),
                ),
            };

            let dialog = adw::AlertDialog::builder()
                .heading(&heading)
                .body(&body)
                .build();
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response(response, &response_label);
            dialog.set_response_appearance(response, adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some(response));

            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != response {
                return;
            }

            if this.selected_history_commit.borrow().as_deref() != Some(commit.id.as_str()) {
                return;
            }

            let current = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            let state_changed = current.detached
                || current.unborn
                || current.branch != status.branch
                || !current.changes.is_empty();
            let merge_started = match backend.merge_in_progress().await {
                Ok(value) => value,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check Merge State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            let history_started = match backend.history_operation().await {
                Ok(value) => value.is_some(),
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check History State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            if state_changed || merge_started || history_started {
                this.show_git_warning_dialog(
                    &tr("Repository Changed"),
                    tr("The current branch or working tree changed while the confirmation was open. Review the repository and try again."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_history_action_busy(true);
            let result = match kind {
                HistoryOperationKind::Revert => backend.revert_commit(commit.id.clone()).await,
                HistoryOperationKind::CherryPick => {
                    backend.cherry_pick_commit(commit.id.clone()).await
                }
            };

            match result {
                Ok(()) => match kind {
                    HistoryOperationKind::Revert => this.show_toast(tr("Commit reverted")),
                    HistoryOperationKind::CherryPick => this.show_toast(tr("Commit cherry-picked")),
                },
                Err(error) => match backend.history_operation().await {
                    Ok(Some(operation)) if operation.kind == kind => {
                        eprintln!(
                            "[Git Desk history operation] {:?} needs attention: {}",
                            kind, error
                        );
                        this.nav.select_row(this.nav.row_at_index(0).as_ref());
                        let (action, abort) = match kind {
                            HistoryOperationKind::Revert => (tr("Revert"), tr("Abort Revert")),
                            HistoryOperationKind::CherryPick => {
                                (tr("Cherry-pick"), tr("Abort Cherry-pick"))
                            }
                        };
                        let heading =
                            tr_args("{action} Needs Attention", &[("action", action.clone())]);
                        // The active history operation already exposes structured recovery
                        // controls in Changes. Keep the alert focused and do not duplicate
                        // Git's verbose stderr/hint stream in the user-facing body.
                        let body = match kind {
                            HistoryOperationKind::Revert => tr_args(
                                "Git could not complete the {action} automatically. Review the operation in Changes. Resolve and mark any conflicted files, then continue. You can also use {abort}.\n\n{error}",
                                &[
                                    ("action", action),
                                    ("abort", abort),
                                    ("error", String::new()),
                                ],
                            ),
                            HistoryOperationKind::CherryPick => tr_args(
                                "Git could not complete the {action} automatically. Review the operation in Changes. Resolve and mark any conflicted files, then continue. If an empty Cherry-pick is reported, use Skip Cherry-pick. You can also use {abort}.\n\n{error}",
                                &[
                                    ("action", action),
                                    ("abort", abort),
                                    ("error", String::new()),
                                ],
                            ),
                        }
                        .trim_end()
                        .to_string();
                        this.show_git_warning_dialog(&heading, body).await;
                    }
                    Ok(Some(_)) => {
                        this.show_git_error_dialog(
                            &tr("History Action Failed"),
                            tr_args(
                                "{error}\n\nA different history operation is now active. Review Changes before continuing.",
                                &[("error", error.to_string())],
                            ),
                        )
                        .await;
                    }
                    Ok(None) => {
                        let heading = match kind {
                            HistoryOperationKind::Revert => tr("Revert Failed"),
                            HistoryOperationKind::CherryPick => tr("Cherry-pick Failed"),
                        };
                        this.show_git_error_dialog(&heading, error.to_string())
                            .await;
                    }
                    Err(state_error) => {
                        this.show_git_error_dialog(
                            &tr("History Action Failed"),
                            tr_args(
                                "{error}\n\nGit Desk could not verify whether a history operation remains active: {state_error}",
                                &[
                                    ("error", error.to_string()),
                                    ("state_error", state_error.to_string()),
                                ],
                            ),
                        )
                        .await;
                    }
                },
            }
            this.set_history_action_busy(false);
            this.refresh_all();
        });
    }

    fn continue_history_operation(self: &Rc<Self>) {
        let Some(operation) = self.history_operation.borrow().clone() else {
            return;
        };
        if *self.history_action_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.history_operation().await {
                Ok(Some(current)) if current == operation => {}
                Ok(Some(_)) => {
                    this.show_git_warning_dialog(
                    &tr("History Operation Changed"),
                        tr("A different history operation is now active. Refresh and review Changes before continuing."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Ok(None) => {
                    this.show_git_notice_dialog(
                        &tr("No History Operation in Progress"),
                        tr("There is no Revert or Cherry-pick operation to continue."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check History State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            let unresolved = match backend.unresolved_conflicts().await {
                Ok(paths) => paths,
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Check Conflicts"), error.to_string())
                        .await;
                    return;
                }
            };
            if !unresolved.is_empty() {
                this.show_git_warning_dialog(
                    &tr("Conflicts Still Need Resolution"),
                    ntr_args(
                        "Resolve and mark all conflicted files before continuing. {count} file is still unresolved.",
                        "Resolve and mark all conflicted files before continuing. {count} files are still unresolved.",
                        unresolved.len() as u64,
                        &[("count", unresolved.len().to_string())],
                    ),
                )
                .await;
                this.refresh_all();
                return;
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            let has_tracked_unstaged = status
                .changes
                .iter()
                .any(|change| change.area == ChangeArea::Unstaged && change.status != "untracked");
            if has_tracked_unstaged {
                this.show_git_warning_dialog(
                    &tr("Stage Remaining Changes"),
                    tr("Tracked unstaged changes would not be included when the history operation continues. Stage the resolved changes first."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_history_action_busy(true);
            let result = backend.continue_history_operation(operation.kind).await;
            match result {
                Ok(()) => match operation.kind {
                    HistoryOperationKind::Revert => this.show_toast(tr("Revert completed")),
                    HistoryOperationKind::CherryPick => {
                        this.show_toast(tr("Cherry-pick completed"))
                    }
                },
                Err(error) => {
                    let heading = match operation.kind {
                        HistoryOperationKind::Revert => tr("Could Not Continue Revert"),
                        HistoryOperationKind::CherryPick => tr("Could Not Continue Cherry-pick"),
                    };
                    this.show_git_error_dialog(&heading, error.to_string())
                        .await;
                }
            }
            this.set_history_action_busy(false);
            this.refresh_all();
        });
    }

    fn skip_cherry_pick(self: &Rc<Self>) {
        let Some(operation) = self.history_operation.borrow().clone() else {
            return;
        };
        if operation.kind != HistoryOperationKind::CherryPick
            || *self.history_action_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.history_operation().await {
                Ok(Some(current)) if current == operation => {}
                Ok(Some(_)) => {
                    this.show_git_warning_dialog(
                    &tr("History Operation Changed"),
                        tr("A different history operation is now active. Refresh and review Changes before skipping."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Ok(None) => {
                    this.show_git_notice_dialog(
                        &tr("No Cherry-pick in Progress"),
                        tr("There is no Cherry-pick operation to skip."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check History State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            let unresolved = match backend.unresolved_conflicts().await {
                Ok(paths) => paths,
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Check Conflicts"), error.to_string())
                        .await;
                    return;
                }
            };
            if !unresolved.is_empty() {
                this.show_git_warning_dialog(
                    &tr("Cherry-pick Still Has Conflicts"),
                    tr("Skip is only offered for an empty Cherry-pick. Resolve the conflicts or abort the operation."),
                )
                .await;
                this.refresh_all();
                return;
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            let has_tracked_changes = status
                .changes
                .iter()
                .any(|change| change.status != "untracked");
            if has_tracked_changes {
                this.show_git_warning_dialog(
                    &tr("Cherry-pick Is No Longer Empty"),
                    tr("Tracked changes are present. Review and stage the resolution to continue, or abort the Cherry-pick."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_history_action_busy(true);
            match backend.skip_cherry_pick().await {
                Ok(()) => this.show_toast(tr("Empty Cherry-pick skipped")),
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Skip Cherry-pick"),
                        error.to_string(),
                    )
                    .await;
                }
            }
            this.set_history_action_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_abort_history_operation(self: &Rc<Self>) {
        let Some(operation) = self.history_operation.borrow().clone() else {
            return;
        };
        if *self.history_action_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let (name, heading, response_label) = match operation.kind {
            HistoryOperationKind::Revert => (
                tr("Revert"),
                tr("Abort Current Revert?"),
                tr("Abort Revert"),
            ),
            HistoryOperationKind::CherryPick => (
                tr("Cherry-pick"),
                tr("Abort Current Cherry-pick?"),
                tr("Abort Cherry-pick"),
            ),
        };
        let body = tr_args(
            "Discard the changes created by the current {name} attempt and restore the branch to its pre-operation state?",
            &[("name", name.clone())],
        );
        let dialog = adw::AlertDialog::builder()
            .heading(&heading)
            .body(&body)
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("abort", &response_label);
        dialog.set_response_appearance("abort", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "abort" {
                return;
            }

            match backend.history_operation().await {
                Ok(Some(current)) if current == operation => {}
                Ok(Some(_)) => {
                    this.show_git_warning_dialog(
                    &tr("History Operation Changed"),
                        tr("A different history operation is now active. Refresh and review Changes before aborting."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Ok(None) => {
                    this.show_git_notice_dialog(
                        &tr("No History Operation in Progress"),
                        tr("There is no Revert or Cherry-pick operation to abort."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check History State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            this.set_history_action_busy(true);
            match backend.abort_history_operation(operation.kind).await {
                Ok(()) => this.show_toast(tr_args("{name} aborted", &[("name", name.to_string())])),
                Err(error) => {
                    let heading = match operation.kind {
                        HistoryOperationKind::Revert => tr("Abort Revert Failed"),
                        HistoryOperationKind::CherryPick => tr("Abort Cherry-pick Failed"),
                    };
                    this.show_git_error_dialog(&heading, error.to_string())
                        .await;
                }
            }
            this.set_history_action_busy(false);
            this.refresh_all();
        });
    }

    fn load_outgoing_commits(
        self: &Rc<Self>,
        branch: String,
        upstream: Option<String>,
        ahead: u32,
        unborn: bool,
        detached: bool,
    ) {
        if unborn || detached {
            self.render_outgoing(&[], upstream.as_deref(), None);
            return;
        }

        if let Some(upstream) = upstream {
            if ahead == 0 {
                self.render_outgoing(&[], Some(&upstream), None);
                return;
            }

            let this = self.clone();
            let backend = self.backend.clone();
            glib::spawn_future_local(async move {
                let result = backend.outgoing_commits(upstream.clone()).await;
                let head = backend.head_commit_id().await.ok().flatten();
                let still_current = this.current_status.borrow().as_ref().is_some_and(|status| {
                    status.branch.as_str() == branch.as_str()
                        && status.ahead > 0
                        && status.upstream.as_deref() == Some(upstream.as_str())
                });
                if !still_current {
                    return;
                }

                match result {
                    Ok(commits) => this.render_outgoing(&commits, Some(&upstream), head.as_deref()),
                    Err(error) => this.render_outgoing_error(Some(&upstream), &error.to_string()),
                }
            });
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let remotes = match backend.remotes().await {
                Ok(remotes) => remotes,
                Err(error) => {
                    this.render_outgoing_error(None, &error.to_string());
                    return;
                }
            };

            if remotes.is_empty() {
                let still_current = this.current_status.borrow().as_ref().is_some_and(|status| {
                    status.branch.as_str() == branch.as_str() && status.upstream.is_none()
                });
                if still_current {
                    this.render_outgoing(&[], None, None);
                }
                return;
            }

            let result = backend.unpublished_commits().await;
            let head = backend.head_commit_id().await.ok().flatten();
            let still_current = this.current_status.borrow().as_ref().is_some_and(|status| {
                status.branch.as_str() == branch.as_str()
                    && status.upstream.is_none()
                    && !status.unborn
                    && !status.detached
            });
            if !still_current {
                return;
            }

            match result {
                Ok(commits) => this.render_outgoing(&commits, None, head.as_deref()),
                Err(error) => this.render_outgoing_error(None, &error.to_string()),
            }
        });
    }

    fn render_outgoing(
        self: &Rc<Self>,
        commits: &[Commit],
        upstream: Option<&str>,
        head_commit: Option<&str>,
    ) {
        clear_list(&self.outgoing_list);
        *self.outgoing_head_commit.borrow_mut() = head_commit.map(str::to_string);

        if commits.is_empty() {
            let had_selection = self.selected_outgoing_commit.borrow_mut().take().is_some();
            self.outgoing_list.unselect_all();
            self.outgoing_group.set_visible(false);
            if had_selection {
                self.clear_inspector();
            }
            self.update_unpublished_commit_actions();
            return;
        }

        let subtitle = if let Some(upstream) = upstream {
            ntr_args(
                "{count} commit ready to push to {upstream}",
                "{count} commits ready to push to {upstream}",
                commits.len() as u64,
                &[
                    ("count", commits.len().to_string()),
                    ("upstream", upstream.to_string()),
                ],
            )
        } else {
            ntr_args(
                "{count} local commit not published to a configured remote",
                "{count} local commits not published to a configured remote",
                commits.len() as u64,
                &[("count", commits.len().to_string())],
            )
        };
        self.outgoing_subtitle.set_label(&subtitle);
        self.outgoing_group.set_visible(true);

        let selected = self.selected_outgoing_commit.borrow().clone();
        if selected
            .as_ref()
            .is_some_and(|id| !commits.iter().any(|commit| &commit.id == id))
        {
            self.selected_outgoing_commit.borrow_mut().take();
            self.clear_inspector();
        }

        for commit in commits {
            let commit = commit.clone();
            let commit_id = commit.id.clone();
            let short = commit.id.chars().take(8).collect::<String>();
            let row = adw::ActionRow::builder()
                .title(&commit.subject)
                .subtitle(format!("{} · {short}", commit.author_name))
                .activatable(true)
                .build();

            let this = self.clone();
            row.connect_activated(move |_| this.toggle_outgoing_commit(commit.clone()));
            self.outgoing_list.append(&row);

            if selected.as_deref() == Some(commit_id.as_str()) {
                self.outgoing_list.select_row(Some(&row));
            }
        }
        self.update_unpublished_commit_actions();
    }

    fn render_outgoing_error(&self, upstream: Option<&str>, message: &str) {
        clear_list(&self.outgoing_list);
        self.outgoing_head_commit.borrow_mut().take();
        let had_selection = self.selected_outgoing_commit.borrow_mut().take().is_some();
        self.outgoing_list.unselect_all();
        self.outgoing_group.set_visible(true);
        if had_selection {
            self.clear_inspector();
        }
        let subtitle = upstream.map_or_else(
            || tr("Could not load local commits not published to a configured remote."),
            |upstream| {
                tr_args(
                    "Could not load commits ready to push to {upstream}.",
                    &[("upstream", upstream.to_string())],
                )
            },
        );
        self.outgoing_subtitle.set_label(&subtitle);
        let row = adw::ActionRow::builder()
            .title(tr("Outgoing commits unavailable"))
            .subtitle(message)
            .build();
        row.set_sensitive(false);
        self.outgoing_list.append(&row);
        self.update_unpublished_commit_actions();
    }

    fn toggle_outgoing_commit(self: &Rc<Self>, commit: Commit) {
        let already_selected =
            self.selected_outgoing_commit.borrow().as_deref() == Some(commit.id.as_str());

        if already_selected {
            self.selected_outgoing_commit.borrow_mut().take();
            self.outgoing_list.unselect_all();
            self.clear_inspector();
            return;
        }

        self.selected_history_commit.borrow_mut().take();
        self.history_list.unselect_all();
        *self.selected_outgoing_commit.borrow_mut() = Some(commit.id.clone());
        self.inspect_commit(commit);
    }

    fn load_history(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();

        glib::spawn_future_local(async move {
            const HISTORY_VISIBLE_LIMIT: usize = 250;
            match backend.history(HISTORY_VISIBLE_LIMIT + 1).await {
                Ok(mut commits) => {
                    let has_older_commits = commits.len() > HISTORY_VISIBLE_LIMIT;
                    commits.truncate(HISTORY_VISIBLE_LIMIT);
                    let graph = build_history_graph(&commits);
                    let subtitle = if commits.is_empty() {
                        tr("No commits yet")
                    } else {
                        ntr_args(
                            "{count} recent commit · select a commit to inspect it",
                            "{count} recent commits · select a commit to inspect it",
                            commits.len() as u64,
                            &[("count", commits.len().to_string())],
                        )
                    };
                    this.history_subtitle.set_label(&subtitle);
                    *this.history.borrow_mut() = graph.clone();
                    this.render_history(&graph, has_older_commits);
                }
                Err(error) => this.history_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn render_history(self: &Rc<Self>, graph: &[GraphRow], has_older_commits: bool) {
        clear_list(&self.history_list);

        if graph.is_empty() {
            self.selected_history_commit.borrow_mut().take();
            let row = adw::ActionRow::builder()
                .title(tr("No commits yet"))
                .subtitle(tr("Your first commit will appear here."))
                .build();
            row.set_sensitive(false);
            self.history_list.append(&row);
            return;
        }

        let selected_commit = self.selected_history_commit.borrow().clone();
        self.history_list
            .append(&history_boundary_row(&tr("Newest")));

        for graph_row in graph {
            let commit_id = graph_row.commit.id.clone();
            let row = history_commit_row(graph_row);

            self.history_list.append(&row);

            if selected_commit.as_deref() == Some(commit_id.as_str()) {
                self.history_list.select_row(Some(&row));
            }
        }

        let end_label = if has_older_commits {
            tr("Older commits not shown")
        } else if graph
            .last()
            .is_some_and(|row| row.commit.parents.is_empty())
        {
            tr("Initial commit")
        } else {
            tr("Beginning of available history")
        };
        self.history_list.append(&history_boundary_row(&end_label));
    }

    fn toggle_commit(self: &Rc<Self>, commit: Commit) {
        let already_selected =
            self.selected_history_commit.borrow().as_deref() == Some(commit.id.as_str());

        if already_selected {
            self.selected_history_commit.borrow_mut().take();
            self.history_list.unselect_all();
            self.clear_inspector();
            return;
        }

        self.selected_outgoing_commit.borrow_mut().take();
        self.outgoing_list.unselect_all();
        *self.selected_history_commit.borrow_mut() = Some(commit.id.clone());
        self.inspect_commit(commit);
    }

    fn commit_is_selected(&self, id: &str) -> bool {
        self.selected_history_commit.borrow().as_deref() == Some(id)
            || self.selected_outgoing_commit.borrow().as_deref() == Some(id)
    }

    fn clear_inspector(&self) {
        self.inspector_title.set_label(&tr("Inspector"));
        self.inspector_subtitle.set_visible(true);
        self.inspector_subtitle
            .set_label(&tr("Select an item to inspect it."));
        self.inspector_subtitle.set_tooltip_text(None);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_commit_metadata.set_visible(false);
        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);
        self.inspector_commit_actions.set_visible(false);
        self.inspector_history_actions.set_visible(false);
        self.inspector_stash_actions.set_visible(false);
        self.inspector_tag_actions.set_visible(false);
        clear_box(&self.inspector_files);
        self.diff_view.set_plain_text("");
        self.inspector_body.set_visible(false);
        self.inspector_empty.set_visible(true);
    }

    fn inspect_commit(self: &Rc<Self>, commit: Commit) {
        self.selected_change.borrow_mut().take();
        self.unstaged_list.unselect_all();
        self.staged_list.unselect_all();
        self.selected_branch.borrow_mut().take();
        self.local_branches.unselect_all();
        self.remote_branches.unselect_all();
        self.selected_stash.borrow_mut().take();
        self.stash_list.unselect_all();
        self.selected_tag.borrow_mut().take();
        self.tag_list.unselect_all();
        self.inspector_empty.set_visible(false);
        self.inspector_body.set_visible(true);
        clear_box(&self.inspector_files);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_title.set_label(&commit.subject);
        self.inspector_subtitle.set_label("");
        self.inspector_subtitle.set_tooltip_text(None);
        self.inspector_subtitle.set_visible(false);

        let short = commit.id.chars().take(8).collect::<String>();
        let committed_at = compact_git_datetime(&commit.author_date);
        self.inspector_commit_metadata
            .append(&inspector_metadata_row(
                &tr("Author"),
                &commit.author_name,
                Some(commit.author_email.as_str()),
            ));
        self.inspector_commit_metadata
            .append(&inspector_metadata_row(
                &tr("Authored"),
                &committed_at,
                Some(commit.author_date.as_str()),
            ));
        self.inspector_commit_metadata
            .append(&inspector_metadata_row(
                &tr("Commit"),
                &short,
                Some(commit.id.as_str()),
            ));

        let (parent_label, parent_value, parent_tooltip) = match commit.parents.as_slice() {
            [] => (tr("Parent"), tr("None · initial commit"), None),
            [parent] => (
                tr("Parent"),
                parent.chars().take(8).collect::<String>(),
                Some(parent.clone()),
            ),
            parents => (
                tr("Parents"),
                parents
                    .iter()
                    .map(|parent| parent.chars().take(8).collect::<String>())
                    .collect::<Vec<_>>()
                    .join(" · "),
                Some(parents.join("\n")),
            ),
        };
        self.inspector_commit_metadata
            .append(&inspector_metadata_row(
                &parent_label,
                &parent_value,
                parent_tooltip.as_deref(),
            ));
        self.inspector_commit_metadata.set_visible(true);
        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);
        self.inspector_stash_actions.set_visible(false);
        self.inspector_tag_actions.set_visible(false);
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let commit_id = commit.id.clone();
            if let Ok(message) = backend.commit_message(commit_id.clone()).await {
                if !this.commit_is_selected(&commit_id) {
                    return;
                }
                let body = commit_body(&message);
                this.inspector_message.set_label(&body);
                this.inspector_message.set_visible(!body.is_empty());
            }

            if !this.commit_is_selected(&commit_id) {
                return;
            }

            match backend.changed_files(commit_id.clone()).await {
                Ok(files) => {
                    if !this.commit_is_selected(&commit_id) {
                        return;
                    }
                    clear_box(&this.inspector_files);

                    if files.is_empty() {
                        this.diff_view.set_placeholder(&tr("No changed files."));
                    } else {
                        let additions: u64 =
                            files.iter().map(|file| u64::from(file.additions)).sum();
                        let deletions: u64 =
                            files.iter().map(|file| u64::from(file.deletions)).sum();
                        let changed_files = ntr_args(
                            "{count} file changed",
                            "{count} files changed",
                            files.len() as u64,
                            &[("count", files.len().to_string())],
                        );

                        let changes_header = gtk::Box::new(Orientation::Vertical, 2);
                        let changes_title = gtk::Label::builder()
                            .label(tr("Changes"))
                            .xalign(0.0)
                            .build();
                        changes_title.add_css_class("heading");
                        changes_header.append(&changes_title);

                        let changes_summary = gtk::Label::builder()
                            .label(format!("{changed_files} · +{additions} −{deletions}"))
                            .xalign(0.0)
                            .build();
                        changes_summary.add_css_class("dim-label");
                        changes_summary.add_css_class("caption");
                        changes_header.append(&changes_summary);
                        this.inspector_files.append(&changes_header);

                        this.diff_view
                            .set_placeholder(&tr("Select a file to inspect its diff."));

                        for file in files {
                            let row = gtk::Button::new();
                            row.set_halign(gtk::Align::Fill);

                            let row_content = gtk::Box::new(Orientation::Horizontal, 10);
                            row_content.set_margin_start(10);
                            row_content.set_margin_end(10);

                            let path_label = gtk::Label::builder()
                                .label(&file.path)
                                .xalign(0.0)
                                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                                .hexpand(true)
                                .build();
                            path_label.set_tooltip_text(Some(file.path.as_str()));
                            row_content.append(&path_label);

                            let stats = gtk::Label::new(Some(&format!(
                                "+{} −{}",
                                file.additions, file.deletions
                            )));
                            stats.add_css_class("caption");
                            stats.add_css_class("dim-label");
                            row_content.append(&stats);
                            row.set_child(Some(&row_content));

                            let this_for_click = this.clone();
                            let backend = backend.clone();
                            let commit_id = commit_id.clone();
                            let path = file.path.clone();
                            let old_path = file.old_path.clone();
                            row.connect_clicked(move |_| {
                                let this = this_for_click.clone();
                                let backend = backend.clone();
                                let commit_id = commit_id.clone();
                                let path = path.clone();
                                let old_path = old_path.clone();
                                glib::spawn_future_local(async move {
                                    match backend
                                        .commit_diff(commit_id.clone(), path.clone(), old_path)
                                        .await
                                    {
                                        Ok(patch) => {
                                            if !this.commit_is_selected(&commit_id) {
                                                return;
                                            }
                                            this.inspector_title.set_label(&path);
                                            this.diff_view.set_patch(&patch);
                                        }
                                        Err(error) => {
                                            this.diff_view.set_plain_text(&error.to_string())
                                        }
                                    }
                                });
                            });
                            this.inspector_files.append(&row);
                        }
                    }
                }
                Err(error) => this.diff_view.set_plain_text(&error.to_string()),
            }
        });
    }

    fn load_branches(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.branches().await {
                Ok(branches) => {
                    let merge_in_progress = match backend.merge_in_progress().await {
                        Ok(value) => value,
                        Err(error) => {
                            this.branches_subtitle.set_label(&error.to_string());
                            false
                        }
                    };
                    let history_operation_in_progress = this.history_operation_active();
                    let local = branches.iter().filter(|branch| !branch.remote).count();
                    let remote = branches.iter().filter(|branch| branch.remote).count();
                    let subtitle = if merge_in_progress {
                        tr_args(
                            "{local} local · {remote} remote · merge in progress",
                            &[("local", local.to_string()), ("remote", remote.to_string())],
                        )
                    } else if history_operation_in_progress {
                        tr_args(
                            "{local} local · {remote} remote · history operation in progress",
                            &[("local", local.to_string()), ("remote", remote.to_string())],
                        )
                    } else {
                        tr_args(
                            "{local} local · {remote} remote",
                            &[("local", local.to_string()), ("remote", remote.to_string())],
                        )
                    };
                    this.branches_subtitle.set_label(&subtitle);
                    this.new_branch_button.set_sensitive(
                        !merge_in_progress
                            && !history_operation_in_progress
                            && !*this.merge_busy.borrow()
                            && !*this.history_action_busy.borrow(),
                    );
                    this.render_branches(
                        &branches,
                        merge_in_progress,
                        history_operation_in_progress,
                    );
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn render_branches(
        self: &Rc<Self>,
        branches: &[Branch],
        merge_in_progress: bool,
        history_operation_in_progress: bool,
    ) {
        clear_list(&self.local_branches);
        clear_list(&self.remote_branches);
        let mutation_locked = merge_in_progress || history_operation_in_progress;

        let selected_branch = self.selected_branch.borrow().clone();
        if selected_branch
            .as_ref()
            .is_some_and(|selected| !branches.iter().any(|branch| &branch.name == selected))
        {
            self.selected_branch.borrow_mut().take();
            self.clear_inspector();
        }

        for branch in branches {
            let subtitle = if branch.current {
                if merge_in_progress {
                    tr("Current branch · merge in progress")
                } else if history_operation_in_progress {
                    tr("Current branch · history operation in progress")
                } else if branch.unborn {
                    tr("Current branch · first commit not created yet")
                } else {
                    tr("Current branch")
                }
            } else if let Some(upstream) = &branch.upstream {
                tr_args("Tracks {upstream}", &[("upstream", upstream.clone())])
            } else if branch.remote {
                tr("Remote branch")
            } else {
                tr("Local branch")
            };

            let row = adw::ActionRow::builder()
                .title(&branch.name)
                .subtitle(&subtitle)
                .activatable(true)
                .build();

            if branch.current {
                row.add_prefix(&gtk::Image::from_icon_name("object-select-symbolic"));
            }

            let this = self.clone();
            let selected = branch.clone();
            row.connect_activated(move |_| this.toggle_branch(selected.clone()));

            if !branch.remote {
                let popover = gtk::Popover::new();
                let actions = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(4)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(6)
                    .margin_end(6)
                    .build();

                if !branch.current {
                    let switch = gtk::Button::with_label(&tr("Switch to Branch"));
                    switch.add_css_class("flat");
                    switch.set_halign(gtk::Align::Fill);
                    switch.set_sensitive(!mutation_locked);
                    let this = self.clone();
                    let target = branch.clone();
                    let action_popover = popover.clone();
                    switch.connect_clicked(move |_| {
                        let this = this.clone();
                        let target = target.clone();
                        run_after_popover_closed(&action_popover, move || {
                            this.request_switch_branch(target);
                        });
                    });
                    actions.append(&switch);

                    let merge = gtk::Button::with_label(&tr("Merge into Current Branch…"));
                    merge.add_css_class("flat");
                    merge.set_halign(gtk::Align::Fill);
                    merge.set_sensitive(!mutation_locked);
                    let this = self.clone();
                    let target = branch.clone();
                    let action_popover = popover.clone();
                    merge.connect_clicked(move |_| {
                        let this = this.clone();
                        let target = target.clone();
                        run_after_popover_closed(&action_popover, move || {
                            this.request_merge_branch(target);
                        });
                    });
                    actions.append(&merge);
                } else if merge_in_progress {
                    let abort = gtk::Button::with_label(&tr("Abort Merge…"));
                    abort.add_css_class("destructive-action");
                    abort.set_halign(gtk::Align::Fill);
                    let this = self.clone();
                    let action_popover = popover.clone();
                    abort.connect_clicked(move |_| {
                        let this = this.clone();
                        run_after_popover_closed(&action_popover, move || {
                            this.confirm_abort_merge();
                        });
                    });
                    actions.append(&abort);
                }

                if !mutation_locked && !branch.unborn {
                    let upstream_label = if branch.upstream.is_some() {
                        "Change Upstream…"
                    } else {
                        "Set Upstream…"
                    };
                    let set_upstream = gtk::Button::with_label(upstream_label);
                    set_upstream.add_css_class("flat");
                    set_upstream.set_halign(gtk::Align::Fill);
                    let this = self.clone();
                    let target = branch.clone();
                    let action_popover = popover.clone();
                    set_upstream.connect_clicked(move |_| {
                        let this = this.clone();
                        let target = target.clone();
                        run_after_popover_closed(&action_popover, move || {
                            this.set_upstream_dialog(target);
                        });
                    });
                    actions.append(&set_upstream);

                    if branch.upstream.is_some() {
                        let unset_upstream = gtk::Button::with_label(&tr("Unset Upstream"));
                        unset_upstream.add_css_class("flat");
                        unset_upstream.set_halign(gtk::Align::Fill);
                        let this = self.clone();
                        let target = branch.clone();
                        unset_upstream
                            .connect_clicked(move |_| this.unset_upstream(target.clone()));
                        actions.append(&unset_upstream);
                    }
                }

                if !mutation_locked {
                    let rename = gtk::Button::with_label(&tr("Rename Branch…"));
                    rename.add_css_class("flat");
                    rename.set_halign(gtk::Align::Fill);
                    let this = self.clone();
                    let target = branch.clone();
                    let action_popover = popover.clone();
                    rename.connect_clicked(move |_| {
                        let this = this.clone();
                        let target = target.clone();
                        run_after_popover_closed(&action_popover, move || {
                            this.rename_branch_dialog(target);
                        });
                    });
                    actions.append(&rename);

                    if !branch.current {
                        let delete = gtk::Button::with_label(&tr("Delete Branch…"));
                        delete.add_css_class("destructive-action");
                        delete.set_halign(gtk::Align::Fill);
                        let this = self.clone();
                        let target = branch.clone();
                        let action_popover = popover.clone();
                        delete.connect_clicked(move |_| {
                            let this = this.clone();
                            let target = target.clone();
                            run_after_popover_closed(&action_popover, move || {
                                this.confirm_delete_branch(target);
                            });
                        });
                        actions.append(&delete);
                    }
                }

                popover.set_child(Some(&actions));

                let menu = gtk::MenuButton::builder()
                    .label(tr("Actions"))
                    .valign(gtk::Align::Center)
                    .tooltip_text(tr("Branch actions"))
                    .build();
                menu.add_css_class("flat");
                menu.set_popover(Some(&popover));
                row.add_suffix(&menu);
            } else if !branch.name.ends_with("/HEAD") {
                let popover = gtk::Popover::new();
                let actions = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(4)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(6)
                    .margin_end(6)
                    .build();

                let merge = gtk::Button::with_label(&tr("Merge into Current Branch…"));
                merge.add_css_class("flat");
                merge.set_halign(gtk::Align::Fill);
                merge.set_sensitive(!mutation_locked);
                let this = self.clone();
                let target = branch.clone();
                let action_popover = popover.clone();
                merge.connect_clicked(move |_| {
                    let this = this.clone();
                    let target = target.clone();
                    run_after_popover_closed(&action_popover, move || {
                        this.request_merge_branch(target);
                    });
                });
                actions.append(&merge);

                popover.set_child(Some(&actions));

                let menu = gtk::MenuButton::builder()
                    .label(tr("Actions"))
                    .valign(gtk::Align::Center)
                    .tooltip_text(tr("Remote branch actions"))
                    .build();
                menu.add_css_class("flat");
                menu.set_popover(Some(&popover));
                row.add_suffix(&menu);
            }

            if branch.remote {
                self.remote_branches.append(&row);
                if selected_branch.as_deref() == Some(branch.name.as_str()) {
                    self.remote_branches.select_row(Some(&row));
                }
            } else {
                self.local_branches.append(&row);
                if selected_branch.as_deref() == Some(branch.name.as_str()) {
                    self.local_branches.select_row(Some(&row));
                }
            }
        }

        if self.local_branches.first_child().is_none() {
            let row = adw::ActionRow::builder()
                .title(tr("No local branches yet"))
                .build();
            row.set_sensitive(false);
            self.local_branches.append(&row);
        }
        if self.remote_branches.first_child().is_none() {
            let row = adw::ActionRow::builder()
                .title(tr("No remote branches configured"))
                .build();
            row.set_sensitive(false);
            self.remote_branches.append(&row);
        }

        if let Some(selected_name) = self.selected_branch.borrow().clone()
            && let Some(branch) = branches.iter().find(|branch| branch.name == selected_name)
        {
            self.inspect_branch(branch);
        }
    }

    fn toggle_branch(self: &Rc<Self>, branch: Branch) {
        let already_selected =
            self.selected_branch.borrow().as_deref() == Some(branch.name.as_str());

        if already_selected {
            self.selected_branch.borrow_mut().take();
            self.local_branches.unselect_all();
            self.remote_branches.unselect_all();
            self.clear_inspector();
            return;
        }

        *self.selected_branch.borrow_mut() = Some(branch.name.clone());
        self.inspect_branch(&branch);

        if branch.remote {
            self.local_branches.unselect_all();
        } else {
            self.remote_branches.unselect_all();
        }
    }

    fn create_branch_dialog(self: &Rc<Self>) {
        if *self.history_action_busy.borrow() || self.history_operation_active() {
            return;
        }

        let entry = gtk::Entry::builder()
            .placeholder_text(tr("feature/my-branch"))
            .activates_default(true)
            .build();

        let dialog = adw::AlertDialog::builder()
            .heading(tr("New Branch"))
            .body(tr("Create a branch from the current branch and switch to it. Uncommitted changes stay in the working tree."))
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("create", &tr("Create & Switch"));
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_response_enabled("create", false);

        let dialog_for_entry = dialog.clone();
        entry.connect_changed(move |entry| {
            dialog_for_entry.set_response_enabled("create", !entry.text().trim().is_empty());
        });

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "create" {
                return;
            }

            let name = entry.text().trim().to_string();
            if name.is_empty() {
                return;
            }

            match backend.create_and_switch_branch(name.clone()).await {
                Ok(()) => {
                    *this.selected_branch.borrow_mut() = Some(name);
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn set_upstream_dialog(self: &Rc<Self>, branch: Branch) {
        if branch.remote || branch.unborn {
            return;
        }

        let entry = gtk::Entry::builder()
            .placeholder_text(tr("origin/main"))
            .activates_default(true)
            .build();
        if let Some(upstream) = &branch.upstream {
            entry.set_text(upstream);
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr_args(
                "Set Upstream for {branch}",
                &[("branch", branch.name.clone())],
            ))
            .body(tr(
                "Enter the remote branch this local branch should track, for example origin/main.",
            ))
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("set", &tr("Set Upstream"));
        dialog.set_response_appearance("set", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("set"));
        dialog.set_response_enabled("set", !entry.text().trim().is_empty());

        let dialog_for_entry = dialog.clone();
        entry.connect_changed(move |entry| {
            dialog_for_entry.set_response_enabled("set", !entry.text().trim().is_empty());
        });

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        let branch_name = branch.name;
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "set" {
                return;
            }

            let upstream = entry.text().trim().to_string();
            if upstream.is_empty() {
                return;
            }

            match backend.set_upstream(branch_name.clone(), upstream).await {
                Ok(()) => {
                    *this.selected_branch.borrow_mut() = Some(branch_name);
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn unset_upstream(self: &Rc<Self>, branch: Branch) {
        if branch.remote || branch.unborn || branch.upstream.is_none() {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        let branch_name = branch.name;
        glib::spawn_future_local(async move {
            match backend.unset_upstream(branch_name.clone()).await {
                Ok(()) => {
                    *this.selected_branch.borrow_mut() = Some(branch_name);
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn request_switch_branch(self: &Rc<Self>, branch: Branch) {
        if branch.remote
            || branch.current
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.status().await {
                Ok(status) if !status.changes.is_empty() => {
                    this.confirm_switch_with_changes(branch);
                }
                Ok(_) => this.perform_switch_branch(branch.name),
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn confirm_switch_with_changes(self: &Rc<Self>, branch: Branch) {
        let dialog = adw::AlertDialog::builder()
            .heading(tr_args("Switch to {branch}?", &[("branch", branch.name.clone())]))
            .body(tr("This repository has uncommitted changes. Git Desk will leave them in the working tree. Git may refuse the switch if the target branch would overwrite them."))
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Warning);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("switch", &tr("Switch Branch"));
        dialog.set_response_appearance("switch", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        glib::spawn_future_local(async move {
            if dialog.choose_future(parent.as_ref()).await.as_str() == "switch" {
                this.perform_switch_branch(branch.name);
            }
        });
    }

    fn perform_switch_branch(self: &Rc<Self>, name: String) {
        if *self.history_action_busy.borrow() || self.history_operation_active() {
            return;
        }
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.switch_branch(name.clone()).await {
                Ok(()) => {
                    *this.selected_branch.borrow_mut() = Some(name);
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn load_merge_state(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let in_progress = match backend.merge_in_progress().await {
                Ok(value) => value,
                Err(error) => {
                    this.merge_group.set_visible(false);
                    this.changes_subtitle.set_label(&error.to_string());
                    return;
                }
            };

            let (unresolved_count, conflicts_known) = if in_progress {
                match backend.unresolved_conflicts().await {
                    Ok(paths) => (paths.len(), true),
                    Err(error) => {
                        this.changes_subtitle.set_label(&error.to_string());
                        (0, false)
                    }
                }
            } else {
                (0, true)
            };

            *this.merge_in_progress.borrow_mut() = in_progress;
            *this.merge_unresolved_count.borrow_mut() = unresolved_count;
            *this.merge_conflicts_known.borrow_mut() = conflicts_known;
            this.update_merge_controls();
            this.update_history_operation_controls();
            this.update_commit_button_state();
            this.update_unpublished_commit_actions();
            this.update_history_commit_actions();
            this.update_stash_action_state();
            this.update_tag_action_state();
        });
    }

    fn load_history_operation_state(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let operation = match backend.history_operation().await {
                Ok(operation) => operation,
                Err(error) => {
                    *this.history_operation_conflicts_known.borrow_mut() = false;
                    this.changes_subtitle.set_label(&error.to_string());
                    this.update_history_operation_controls();
                    this.update_history_commit_actions();
                    this.update_commit_button_state();
                    this.update_stash_action_state();
                    this.update_tag_action_state();
                    return;
                }
            };

            let (unresolved_count, conflicts_known) = if operation.is_some() {
                match backend.unresolved_conflicts().await {
                    Ok(paths) => (paths.len(), true),
                    Err(error) => {
                        this.changes_subtitle.set_label(&error.to_string());
                        (0, false)
                    }
                }
            } else {
                (0, true)
            };

            let operation_changed = this.history_operation.borrow().as_ref() != operation.as_ref();
            *this.history_operation.borrow_mut() = operation;
            *this.history_operation_unresolved_count.borrow_mut() = unresolved_count;
            *this.history_operation_conflicts_known.borrow_mut() = conflicts_known;
            this.update_history_operation_controls();
            this.update_merge_controls();
            this.update_commit_button_state();
            this.update_unpublished_commit_actions();
            this.update_history_commit_actions();
            this.update_stash_action_state();
            this.update_tag_action_state();
            if operation_changed {
                this.load_branches();
                this.load_remotes();
            }
        });
    }

    fn update_history_operation_controls(&self) {
        let operation = self.history_operation.borrow().clone();
        let unresolved = *self.history_operation_unresolved_count.borrow();
        let conflicts_known = *self.history_operation_conflicts_known.borrow();
        let busy = *self.history_action_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || self.sync_busy.borrow().is_some();

        self.update_commit_composer_state();
        self.history_operation_group
            .set_visible(operation.is_some());
        let Some(operation) = operation else {
            self.continue_history_operation_button.set_sensitive(false);
            self.skip_history_operation_button.set_sensitive(false);
            self.skip_history_operation_button.set_visible(false);
            self.abort_history_operation_button.set_sensitive(false);
            return;
        };

        let status = self.current_status.borrow();
        let (status_known, has_tracked_unstaged, has_tracked_changes) = match status.as_ref() {
            Some(status) => (
                true,
                status.changes.iter().any(|change| {
                    change.area == ChangeArea::Unstaged && change.status != "untracked"
                }),
                status
                    .changes
                    .iter()
                    .any(|change| change.status != "untracked"),
            ),
            None => (false, false, false),
        };
        drop(status);
        let empty_cherry_pick = operation.kind == HistoryOperationKind::CherryPick
            && conflicts_known
            && unresolved == 0
            && status_known
            && !has_tracked_changes;
        let short = operation.commit.chars().take(8).collect::<String>();
        let (name, continue_label, abort_label) = match operation.kind {
            HistoryOperationKind::Revert => {
                (tr("Revert"), tr("Continue Revert"), tr("Abort Revert…"))
            }
            HistoryOperationKind::CherryPick => (
                tr("Cherry-pick"),
                tr("Continue Cherry-pick"),
                tr("Abort Cherry-pick…"),
            ),
        };

        self.history_operation_status_row
            .set_title(&tr_args("{name} in Progress", &[("name", name.clone())]));
        self.continue_history_operation_button
            .set_label(&continue_label);
        self.abort_history_operation_button.set_label(&abort_label);

        let (subtitle, ready) = if !conflicts_known {
            (
                tr_args(
                    "Git Desk could not verify the conflict state for {name} {short}. Refresh the repository or abort the operation.",
                    &[("name", name.clone()), ("short", short.clone())],
                ),
                false,
            )
        } else if !status_known {
            (
                tr_args(
                    "Git Desk could not verify the working tree state for {name} {short}. Refresh the repository or abort the operation.",
                    &[("name", name.clone()), ("short", short.clone())],
                ),
                false,
            )
        } else if unresolved > 0 {
            (
                ntr_args(
                    "{name} {short} has {unresolved} unresolved conflict. Resolve the file, then mark it resolved.",
                    "{name} {short} has {unresolved} unresolved conflicts. Resolve each file, then mark it resolved.",
                    unresolved as u64,
                    &[
                        ("name", name.clone()),
                        ("short", short.clone()),
                        ("unresolved", unresolved.to_string()),
                    ],
                ),
                false,
            )
        } else if empty_cherry_pick {
            (
                tr_args(
                    "Cherry-pick {short} has no tracked changes left to commit. Skip the empty Cherry-pick or abort it.",
                    &[("short", short.clone())],
                ),
                false,
            )
        } else if has_tracked_unstaged {
            (
                tr_args(
                    "{name} {short} is resolved, but tracked unstaged changes remain. Stage them before continuing.",
                    &[("name", name.clone()), ("short", short.clone())],
                ),
                false,
            )
        } else {
            (
                tr_args(
                    "All conflicts for {name} {short} are resolved and staged. Continue the operation to create its commit.",
                    &[("name", name.clone()), ("short", short.clone())],
                ),
                true,
            )
        };

        self.history_operation_status_row.set_subtitle(&subtitle);
        self.continue_history_operation_button
            .set_sensitive(ready && !busy);
        self.skip_history_operation_button
            .set_visible(empty_cherry_pick);
        self.skip_history_operation_button
            .set_sensitive(empty_cherry_pick && !busy);
        self.skip_history_operation_button
            .set_tooltip_text(Some(&tr(
                "Finish an empty Cherry-pick without creating a commit.",
            )));
        self.continue_history_operation_button
            .remove_css_class("suggested-action");
        if ready && !busy {
            self.continue_history_operation_button
                .add_css_class("suggested-action");
        }
        let continue_tooltip = if ready {
            tr("Continue the current Git history operation using the staged resolution.")
        } else if !conflicts_known {
            tr("Git Desk must verify the conflict state before continuing.")
        } else if !status_known {
            tr("Git Desk must verify the working tree state before continuing.")
        } else if unresolved > 0 {
            tr("Resolve and mark every conflicted file before continuing.")
        } else if empty_cherry_pick {
            tr("There are no tracked changes to commit. Skip the empty Cherry-pick.")
        } else {
            tr("Stage the remaining tracked changes before continuing.")
        };
        self.continue_history_operation_button
            .set_tooltip_text(Some(&continue_tooltip));
        self.abort_history_operation_button.set_sensitive(!busy);
        self.new_branch_button.set_sensitive(false);
        self.fetch_button.set_sensitive(false);
        self.pull_button.set_sensitive(false);
        self.push_button.set_sensitive(false);
        self.pull_button.remove_css_class("suggested-action");
        self.push_button.remove_css_class("suggested-action");
    }

    fn update_merge_controls(&self) {
        let in_progress = *self.merge_in_progress.borrow();
        let unresolved = *self.merge_unresolved_count.borrow();
        let conflicts_known = *self.merge_conflicts_known.borrow();
        let busy = *self.merge_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some();

        self.update_commit_composer_state();
        self.merge_group.set_visible(in_progress);
        if !in_progress {
            self.complete_merge_button.set_sensitive(false);
            self.abort_merge_button.set_sensitive(false);
            return;
        }

        let has_tracked_unstaged = self.current_status.borrow().as_ref().is_some_and(|status| {
            status
                .changes
                .iter()
                .any(|change| change.area == ChangeArea::Unstaged && change.status != "untracked")
        });

        let (subtitle, ready) = if !conflicts_known {
            (
                tr(
                    "Git Desk could not verify the conflict state. Refresh the repository or abort the merge.",
                ),
                false,
            )
        } else if unresolved > 0 {
            (
                tr_args(
                    "Unresolved conflicts: {unresolved}. Resolve each file, then mark it resolved.",
                    &[("unresolved", unresolved.to_string())],
                ),
                false,
            )
        } else if has_tracked_unstaged {
            (
                tr(
                    "Conflicts are resolved, but tracked unstaged changes remain. Stage them before completing the merge.",
                ),
                false,
            )
        } else {
            (
                tr(
                    "All conflicts are resolved and staged. Complete the merge to create the merge commit.",
                ),
                true,
            )
        };

        self.merge_status_row.set_subtitle(&subtitle);
        self.complete_merge_button.set_sensitive(ready && !busy);
        self.complete_merge_button
            .remove_css_class("suggested-action");
        if ready && !busy {
            self.complete_merge_button.add_css_class("suggested-action");
        }
        let complete_tooltip = if ready {
            tr("Create the merge commit using Git’s prepared merge message.")
        } else if !conflicts_known {
            tr("Git Desk must verify the conflict state before completing the merge.")
        } else if unresolved > 0 {
            tr("Resolve and mark every conflicted file before completing the merge.")
        } else {
            tr("Stage the remaining tracked changes before completing the merge.")
        };
        self.complete_merge_button
            .set_tooltip_text(Some(&complete_tooltip));
        self.abort_merge_button.set_sensitive(!busy);
        self.fetch_button.set_sensitive(false);
        self.pull_button.set_sensitive(false);
        self.push_button.set_sensitive(false);
        self.pull_button.remove_css_class("suggested-action");
        self.push_button.remove_css_class("suggested-action");
    }

    fn complete_merge(self: &Rc<Self>) {
        if *self.merge_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.merge_in_progress().await {
                Ok(true) => {}
                Ok(false) => {
                    this.show_git_notice_dialog(
                        &tr("No Merge in Progress"),
                        tr("There is no active merge to complete."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check Merge State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            let unresolved = match backend.unresolved_conflicts().await {
                Ok(paths) => paths,
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Check Conflicts"), error.to_string())
                        .await;
                    return;
                }
            };
            if !unresolved.is_empty() {
                this.show_git_warning_dialog(
                    &tr("Conflicts Still Need Resolution"),
                    ntr_args(
                        "Resolve and mark all conflicted files before completing the merge. {count} file is still unresolved.",
                        "Resolve and mark all conflicted files before completing the merge. {count} files are still unresolved.",
                        unresolved.len() as u64,
                        &[("count", unresolved.len().to_string())],
                    ),
                )
                .await;
                this.refresh_all();
                return;
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            let has_tracked_unstaged = status
                .changes
                .iter()
                .any(|change| change.area == ChangeArea::Unstaged && change.status != "untracked");
            if has_tracked_unstaged {
                this.show_git_warning_dialog(
                    &tr("Stage Remaining Changes"),
                    tr("Tracked unstaged changes would not be included in the merge commit. Stage them first, then complete the merge."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_merge_busy(true);
            match backend.complete_merge().await {
                Ok(()) => this.show_toast(tr("Merge completed")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Complete Merge Failed"), error.to_string())
                        .await;
                }
            }
            this.set_merge_busy(false);
            this.refresh_all();
        });
    }

    fn set_merge_busy(&self, busy: bool) {
        *self.merge_busy.borrow_mut() = busy;
        self.update_merge_controls();
        self.update_history_operation_controls();
        self.update_commit_button_state();
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();
        self.update_stash_action_state();
        self.update_tag_action_state();
        self.new_branch_button.set_sensitive(!busy);

        if busy {
            self.fetch_button.set_sensitive(false);
            self.pull_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
            self.pull_button.remove_css_class("suggested-action");
            self.push_button.remove_css_class("suggested-action");
        }
    }

    fn request_merge_branch(self: &Rc<Self>, branch: Branch) {
        if branch.current
            || branch.unborn
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.merge_in_progress().await {
                Ok(true) => {
                    this.show_git_warning_dialog(
                        &tr("Merge Already in Progress"),
                        tr("Finish the current merge or abort it before starting another one."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Ok(false) => {}
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check Merge State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };

            if status.detached || status.unborn {
                this.show_git_warning_dialog(
                    &tr("Cannot Merge Here"),
                    tr("Check out a normal local branch with at least one commit before merging another branch into it."),
                )
                .await;
                return;
            }

            if !status.changes.is_empty() {
                this.show_git_warning_dialog(
                    &tr("Commit or Stash Changes First"),
                    tr("Git Desk only starts a branch merge from a clean working tree. Commit your work or save it in Stashes, then try again."),
                )
                .await;
                return;
            }

            let current_branch = status.branch.clone();
            let dialog = adw::AlertDialog::builder()
                .heading(tr_args("Merge {branch} into {current}?", &[("branch", branch.name.clone()), ("current", current_branch.clone())]))
                .body(tr("Git will fast-forward when possible. Otherwise it will create a merge commit. If conflicts occur, Git Desk will keep the merge open so you can resolve the files in Changes or abort the merge from Branches."))
                .build();
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("merge", &tr("Merge Branch"));
            dialog.set_response_appearance("merge", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));

            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "merge" {
                return;
            }

            // Re-read state after the confirmation. This prevents a stale dialog
            // from merging into a different branch or across newly-created work.
            let current = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            if current.detached
                || current.unborn
                || current.branch != current_branch
                || !current.changes.is_empty()
            {
                this.show_git_warning_dialog(
                    &tr("Repository Changed"),
                    tr("The current branch or working tree changed while the merge confirmation was open. Review the repository and try again."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_merge_busy(true);
            match backend.merge_branch(branch.name.clone()).await {
                Ok(()) => {
                    this.show_toast(tr_args(
                        "Merged {branch} into {current}",
                        &[
                            ("branch", branch.name.clone()),
                            ("current", current_branch.clone()),
                        ],
                    ));
                }
                Err(error) => match backend.merge_in_progress().await {
                    Ok(true) => {
                        this.stack.set_visible_child_name("changes");
                        let unresolved_count = backend
                            .unresolved_conflicts()
                            .await
                            .ok()
                            .filter(|paths| !paths.is_empty())
                            .map(|paths| paths.len());
                        let abort_requested =
                            this.show_merge_conflict_dialog(unresolved_count).await;
                        this.set_merge_busy(false);
                        this.refresh_all();
                        if abort_requested {
                            this.confirm_abort_merge();
                        }
                        return;
                    }
                    Ok(false) => {
                        this.show_git_error_dialog(&tr("Merge Failed"), error.to_string())
                            .await;
                    }
                    Err(state_error) => {
                        this.show_git_error_dialog(
                            &tr("Merge Failed"),
                            tr_args(
                                "{error}\n\nCould not verify the merge state: {state_error}",
                                &[
                                    ("error", error.to_string()),
                                    ("state_error", state_error.to_string()),
                                ],
                            ),
                        )
                        .await;
                    }
                },
            }
            this.set_merge_busy(false);
            this.refresh_all();
        });
    }

    async fn show_merge_conflict_dialog(&self, unresolved_count: Option<usize>) -> bool {
        let mut body = tr(
            "Git could not merge the branches automatically. Resolve each conflict in Changes, mark it resolved, then complete the merge. You can also abort the merge to restore the branch to its previous state.",
        );
        if let Some(count) = unresolved_count {
            body.push_str("\n\n");
            body.push_str(&tr_args(
                "Unresolved conflicts: {count}",
                &[("count", count.to_string())],
            ));
        }
        let dialog = adw::AlertDialog::builder()
            .heading(tr("Merge Conflict"))
            .body(&body)
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Warning);
        dialog.add_response("abort", &tr("Abort Merge…"));
        dialog.add_response("review", &tr("Review Conflicts"));
        dialog.set_response_appearance("abort", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("review", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("review"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        dialog.choose_future(parent.as_ref()).await.as_str() == "abort"
    }

    fn confirm_abort_merge(self: &Rc<Self>) {
        if *self.merge_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Abort Current Merge?"))
            .body(tr("Discard the changes created by the current merge attempt and restore the branch to its pre-merge state?"))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("abort", &tr("Abort Merge"));
        dialog.set_response_appearance("abort", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "abort" {
                return;
            }

            match backend.merge_in_progress().await {
                Ok(true) => {}
                Ok(false) => {
                    this.show_git_notice_dialog(
                        &tr("No Merge in Progress"),
                        tr("There is no active merge to abort."),
                    )
                    .await;
                    this.refresh_all();
                    return;
                }
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Check Merge State"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }

            this.set_merge_busy(true);
            match backend.abort_merge().await {
                Ok(()) => this.show_toast(tr("Merge aborted")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Abort Merge Failed"), error.to_string())
                        .await;
                }
            }
            this.set_merge_busy(false);
            this.refresh_all();
        });
    }

    fn rename_branch_dialog(self: &Rc<Self>, branch: Branch) {
        if branch.remote || *self.history_action_busy.borrow() || self.history_operation_active() {
            return;
        }

        let entry = gtk::Entry::builder()
            .text(&branch.name)
            .activates_default(true)
            .build();

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Rename Branch"))
            .body(tr(
                "Renaming a branch changes its local name without changing its commits.",
            ))
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("rename", &tr("Rename"));
        dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("rename"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        let old_name = branch.name;
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "rename" {
                return;
            }

            let new_name = entry.text().trim().to_string();
            if new_name.is_empty() || new_name == old_name {
                return;
            }

            match backend
                .rename_branch(old_name.clone(), new_name.clone())
                .await
            {
                Ok(()) => {
                    let was_selected =
                        this.selected_branch.borrow().as_deref() == Some(old_name.as_str());
                    if was_selected {
                        *this.selected_branch.borrow_mut() = Some(new_name);
                    }
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn confirm_delete_branch(self: &Rc<Self>, branch: Branch) {
        if branch.remote
            || branch.current
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr_args("Delete {branch}?", &[("branch", branch.name.clone())]))
            .body(tr("This removes the local branch reference. Git Desk uses Git’s safe deletion, so Git will refuse if it considers the branch not fully merged."))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete Branch"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        let name = branch.name;
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "delete" {
                return;
            }

            match backend.delete_branch(name.clone()).await {
                Ok(()) => {
                    let was_selected =
                        this.selected_branch.borrow().as_deref() == Some(name.as_str());
                    if was_selected {
                        this.selected_branch.borrow_mut().take();
                        this.clear_inspector();
                    }
                    this.refresh_all();
                }
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn inspect_branch(&self, branch: &Branch) {
        self.selected_change.borrow_mut().take();
        self.unstaged_list.unselect_all();
        self.staged_list.unselect_all();
        self.selected_history_commit.borrow_mut().take();
        self.history_list.unselect_all();
        self.selected_outgoing_commit.borrow_mut().take();
        self.outgoing_list.unselect_all();
        self.selected_stash.borrow_mut().take();
        self.stash_list.unselect_all();
        self.selected_tag.borrow_mut().take();
        self.tag_list.unselect_all();

        self.inspector_empty.set_visible(false);
        self.inspector_body.set_visible(true);
        clear_box(&self.inspector_files);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_commit_metadata.set_visible(false);
        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);
        self.inspector_commit_actions.set_visible(false);
        self.inspector_history_actions.set_visible(false);
        self.inspector_stash_actions.set_visible(false);
        self.inspector_tag_actions.set_visible(false);
        self.diff_view.set_plain_text("");

        self.inspector_title.set_label(&branch.name);
        self.inspector_subtitle.set_visible(true);
        let branch_kind = if branch.remote {
            tr("Remote branch")
        } else {
            tr("Local branch")
        };
        let status = if branch.current {
            if branch.unborn {
                tr("Current branch · first commit not created yet")
            } else {
                tr("Current branch")
            }
        } else {
            branch_kind.clone()
        };
        self.inspector_subtitle.set_label(&status);
        self.inspector_subtitle.set_tooltip_text(None);

        let type_row = adw::ActionRow::builder()
            .title(tr("Type"))
            .subtitle(&branch_kind)
            .activatable(false)
            .build();
        self.inspector_files.append(&type_row);

        let status_row = adw::ActionRow::builder()
            .title(tr("Status"))
            .subtitle(&if branch.current {
                tr("Current branch")
            } else {
                tr("Not checked out")
            })
            .activatable(false)
            .build();
        self.inspector_files.append(&status_row);

        let upstream = branch
            .upstream
            .clone()
            .unwrap_or_else(|| tr("Not configured"));
        let upstream_row = adw::ActionRow::builder()
            .title(tr("Upstream"))
            .subtitle(&upstream)
            .activatable(false)
            .build();
        self.inspector_files.append(&upstream_row);
    }

    fn add_remote_dialog(self: &Rc<Self>) {
        let name_entry = gtk::Entry::builder()
            .placeholder_text(tr("origin"))
            .activates_default(true)
            .build();
        let url_entry = gtk::Entry::builder()
            .placeholder_text(tr("https://github.com/user/repository.git"))
            .activates_default(true)
            .build();
        let fields = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();
        fields.append(&name_entry);
        fields.append(&url_entry);

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Add Remote"))
            .body(tr(
                "Connect this repository to another Git repository. Enter a remote name and URL.",
            ))
            .build();
        dialog.set_extra_child(Some(&fields));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("add", &tr("Add Remote"));
        dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("add"));
        dialog.set_response_enabled("add", false);

        let update_enabled = {
            let dialog = dialog.clone();
            let name_entry = name_entry.clone();
            let url_entry = url_entry.clone();
            move || {
                dialog.set_response_enabled(
                    "add",
                    !name_entry.text().trim().is_empty() && !url_entry.text().trim().is_empty(),
                );
            }
        };
        let update_name = update_enabled.clone();
        name_entry.connect_changed(move |_| update_name());
        url_entry.connect_changed(move |_| update_enabled());

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "add" {
                return;
            }

            let name = name_entry.text().trim().to_string();
            let url = url_entry.text().trim().to_string();
            if name.is_empty() || url.is_empty() {
                return;
            }

            match backend.add_remote(name, url).await {
                Ok(()) => this.refresh_all(),
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn render_remotes(self: &Rc<Self>, remotes: &[(String, String)]) {
        clear_list(&self.remotes_list);

        for (name, url) in remotes {
            let row = adw::ActionRow::builder().title(name).subtitle(url).build();

            let actions = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(4)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .build();
            let remove = gtk::Button::with_label(&tr("Remove Remote…"));
            remove.add_css_class("destructive-action");
            remove.set_halign(gtk::Align::Fill);
            let this = self.clone();
            let remote_name = name.clone();
            let remote_url = url.clone();
            remove.connect_clicked(move |_| {
                this.confirm_remove_remote(remote_name.clone(), remote_url.clone())
            });
            actions.append(&remove);

            let popover = gtk::Popover::new();
            popover.set_child(Some(&actions));
            let menu = gtk::MenuButton::builder()
                .label(tr("Actions"))
                .valign(gtk::Align::Center)
                .tooltip_text(tr("Remote actions"))
                .build();
            menu.add_css_class("flat");
            menu.set_popover(Some(&popover));
            row.add_suffix(&menu);
            self.remotes_list.append(&row);
        }

        if self.remotes_list.first_child().is_none() {
            let row = adw::ActionRow::builder()
                .title(tr("No remotes configured"))
                .subtitle(tr("Add a remote to connect this repository to GitHub, GitLab, Codeberg, or another Git server."))
                .build();
            row.set_sensitive(false);
            self.remotes_list.append(&row);
        }
    }

    fn confirm_remove_remote(self: &Rc<Self>, name: String, url: String) {
        let dialog = adw::AlertDialog::builder()
            .heading(tr_args("Remove {name}?", &[("name", name.clone())]))
            .body(tr_args(
                "This removes the remote configuration for {url}. It does not delete the remote repository or your local commits.",
                &[("url", url.clone())],
            ))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("remove", &tr("Remove Remote"));
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "remove" {
                return;
            }

            match backend.remove_remote(name).await {
                Ok(()) => this.refresh_all(),
                Err(error) => this.branches_subtitle.set_label(&error.to_string()),
            }
        });
    }

    fn load_remotes(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.remotes().await {
                Ok(remotes) => {
                    let has_remote = !remotes.is_empty();
                    this.render_remotes(&remotes);

                    let status = backend.status().await.ok();
                    let can_pull = status.as_ref().is_some_and(|status| {
                        status.upstream.is_some() && !status.unborn && !status.detached
                    });
                    let can_push = has_remote
                        && status
                            .as_ref()
                            .is_some_and(|status| !status.unborn && !status.detached);

                    if this.sync_busy.borrow().is_some()
                        || *this.unpublished_action_busy.borrow()
                        || *this.stash_busy.borrow()
                        || *this.tag_busy.borrow()
                        || *this.merge_busy.borrow()
                        || *this.merge_in_progress.borrow()
                        || *this.history_action_busy.borrow()
                        || this.history_operation_active()
                    {
                        this.fetch_button.set_sensitive(false);
                        this.pull_button.set_sensitive(false);
                        this.push_button.set_sensitive(false);
                        this.pull_button.remove_css_class("suggested-action");
                        this.push_button.remove_css_class("suggested-action");
                        return;
                    }

                    this.fetch_button.set_sensitive(has_remote);
                    this.pull_button.set_sensitive(can_pull);
                    this.push_button.set_sensitive(can_push);
                    this.pull_button.remove_css_class("suggested-action");
                    this.push_button.remove_css_class("suggested-action");

                    match status
                        .as_ref()
                        .and_then(|status| suggested_sync_action(status, has_remote))
                    {
                        Some(SuggestedSyncAction::Pull) if can_pull => {
                            this.pull_button.add_css_class("suggested-action");
                        }
                        Some(SuggestedSyncAction::Push) if can_push => {
                            this.push_button.add_css_class("suggested-action");
                        }
                        _ => {}
                    }
                }
                Err(error) => {
                    this.render_remotes(&[]);
                    this.fetch_button.set_sensitive(false);
                    this.pull_button.set_sensitive(false);
                    this.push_button.set_sensitive(false);
                    this.pull_button.remove_css_class("suggested-action");
                    this.push_button.remove_css_class("suggested-action");
                    this.branches_subtitle.set_label(&error.to_string());
                }
            }
        });
    }

    fn load_stashes(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.stashes().await {
                Ok(stashes) => {
                    let subtitle = if stashes.is_empty() {
                        tr("No saved stashes")
                    } else {
                        ntr_args(
                            "{count} saved stash · select one to inspect it",
                            "{count} saved stashes · select one to inspect it",
                            stashes.len() as u64,
                            &[("count", stashes.len().to_string())],
                        )
                    };
                    this.stashes_subtitle.set_label(&subtitle);
                    this.render_stashes(&stashes);
                }
                Err(error) => {
                    this.stashes_subtitle.set_label(&error.to_string());
                    this.render_stashes(&[]);
                }
            }
        });
    }

    fn render_stashes(self: &Rc<Self>, stashes: &[StashEntry]) {
        clear_list(&self.stash_list);
        self.stash_list.set_visible(!stashes.is_empty());
        self.stash_empty.set_visible(stashes.is_empty());

        let selected_id = self
            .selected_stash
            .borrow()
            .as_ref()
            .map(|stash| stash.id.clone());
        let mut selected_still_exists = false;

        for stash in stashes {
            let stash = stash.clone();
            let short = stash.id.chars().take(8).collect::<String>();
            let row = adw::ActionRow::builder()
                .title(&stash.subject)
                .subtitle(format!("{} · {short}", stash.reference))
                .activatable(true)
                .build();

            let this = self.clone();
            let selected = stash.clone();
            row.connect_activated(move |_| this.toggle_stash(selected.clone()));
            self.stash_list.append(&row);

            if selected_id.as_deref() == Some(stash.id.as_str()) {
                selected_still_exists = true;
                *self.selected_stash.borrow_mut() = Some(stash.clone());
                self.stash_list.select_row(Some(&row));
            }
        }

        if selected_id.is_some() && !selected_still_exists {
            self.selected_stash.borrow_mut().take();
            if self.inspector_stash_actions.is_visible() {
                self.clear_inspector();
            }
        }
        self.update_stash_action_state();
    }

    fn toggle_stash(self: &Rc<Self>, stash: StashEntry) {
        let already_selected = self
            .selected_stash
            .borrow()
            .as_ref()
            .is_some_and(|selected| selected.id == stash.id);

        if already_selected {
            self.selected_stash.borrow_mut().take();
            self.stash_list.unselect_all();
            self.clear_inspector();
            return;
        }

        self.selected_change.borrow_mut().take();
        self.unstaged_list.unselect_all();
        self.staged_list.unselect_all();
        self.selected_history_commit.borrow_mut().take();
        self.history_list.unselect_all();
        self.selected_outgoing_commit.borrow_mut().take();
        self.outgoing_list.unselect_all();
        self.selected_branch.borrow_mut().take();
        self.local_branches.unselect_all();
        self.remote_branches.unselect_all();
        self.selected_tag.borrow_mut().take();
        self.tag_list.unselect_all();
        *self.selected_stash.borrow_mut() = Some(stash.clone());
        self.inspect_stash(stash);
    }

    fn stash_is_selected(&self, id: &str) -> bool {
        self.selected_stash
            .borrow()
            .as_ref()
            .is_some_and(|stash| stash.id == id)
    }

    fn inspect_stash(self: &Rc<Self>, stash: StashEntry) {
        self.inspector_empty.set_visible(false);
        self.inspector_body.set_visible(true);
        self.inspector_commit_actions.set_visible(false);
        self.inspector_history_actions.set_visible(false);
        self.inspector_stash_actions.set_visible(true);
        self.inspector_tag_actions.set_visible(false);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_commit_metadata.set_visible(false);
        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);
        clear_box(&self.inspector_files);
        self.inspector_title.set_label(&stash.subject);
        self.inspector_subtitle.set_visible(true);
        let short = stash.id.chars().take(8).collect::<String>();
        self.inspector_subtitle.set_label(&tr_args(
            "Saved stash · {reference} · {short}",
            &[
                ("reference", stash.reference.clone()),
                ("short", short.clone()),
            ],
        ));
        self.inspector_subtitle.set_tooltip_text(None);
        self.diff_view.set_plain_text(&tr("Loading stash…"));
        self.update_stash_action_state();

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let stash_id = stash.id.clone();
            match backend.stash_files(stash.reference.clone()).await {
                Ok(files) => {
                    if !this.stash_is_selected(&stash_id) {
                        return;
                    }
                    clear_box(&this.inspector_files);
                    for file in files {
                        let row = adw::ActionRow::builder()
                            .title(&file.path)
                            .subtitle(format!("+{} −{}", file.additions, file.deletions))
                            .activatable(false)
                            .build();
                        this.inspector_files.append(&row);
                    }
                }
                Err(error) => {
                    if this.stash_is_selected(&stash_id) {
                        this.diff_view.set_plain_text(&error.to_string());
                    }
                    return;
                }
            }

            match backend.stash_diff(stash.reference.clone()).await {
                Ok(patch) => {
                    if !this.stash_is_selected(&stash_id) {
                        return;
                    }
                    if patch.trim().is_empty() {
                        this.diff_view
                            .set_plain_text(&tr("This stash has no visible diff."));
                    } else {
                        this.diff_view.set_patch(&patch);
                    }
                }
                Err(error) => {
                    if this.stash_is_selected(&stash_id) {
                        this.diff_view.set_plain_text(&error.to_string());
                    }
                }
            }
        });
    }

    fn update_stash_action_state(&self) {
        let selected = self.selected_stash.borrow().is_some();
        let busy = *self.stash_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some();
        let stash_supported = self
            .current_status
            .borrow()
            .as_ref()
            .is_some_and(|status| !status.unborn);
        self.new_stash_button
            .set_sensitive(stash_supported && !busy);
        let stash_tooltip = if stash_supported {
            tr("Temporarily save staged and working-tree changes.")
        } else {
            tr("Create the first commit before using Stash.")
        };
        self.new_stash_button.set_tooltip_text(Some(&stash_tooltip));
        self.inspector_stash_actions.set_visible(selected);
        self.apply_stash_button.set_sensitive(selected && !busy);
        self.pop_stash_button.set_sensitive(selected && !busy);
        self.delete_stash_button.set_sensitive(selected && !busy);
    }

    fn set_stash_busy(&self, busy: bool) {
        *self.stash_busy.borrow_mut() = busy;
        self.update_stash_action_state();
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_tag_action_state();
        self.update_commit_button_state();
        if busy {
            self.fetch_button.set_sensitive(false);
            self.pull_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
            self.pull_button.remove_css_class("suggested-action");
            self.push_button.remove_css_class("suggested-action");
        }
    }

    fn create_stash_dialog(self: &Rc<Self>) {
        if *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let message_entry = gtk::Entry::builder()
            .placeholder_text(tr("Optional stash message"))
            .activates_default(true)
            .build();
        let include_untracked = gtk::CheckButton::with_label(&tr("Include untracked files"));
        include_untracked.set_active(true);

        let fields = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .build();
        fields.append(&message_entry);
        fields.append(&include_untracked);

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Stash Changes"))
            .body(tr("Temporarily save your current staged and working-tree changes, then return to a clean working tree."))
            .build();
        dialog.set_extra_child(Some(&fields));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("stash", &tr("Stash Changes"));
        dialog.set_response_appearance("stash", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("stash"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "stash" {
                return;
            }

            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.show_git_error_dialog(
                        &tr("Could Not Read Repository Status"),
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            };
            if status.unborn {
                this.show_git_notice_dialog(
                    &tr("Stash Requires a First Commit"),
                    tr("Create the repository's first commit before using Stash."),
                )
                .await;
                this.refresh_all();
                return;
            }

            this.set_stash_busy(true);
            let result = backend
                .create_stash(
                    message_entry.text().trim().to_string(),
                    include_untracked.is_active(),
                )
                .await;
            match result {
                Ok(true) => this.show_toast(tr("Stash created")),
                Ok(false) => this.show_toast(tr("No local changes to stash")),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Stash Failed"), error.to_string())
                        .await;
                }
            }
            this.set_stash_busy(false);
            this.refresh_all();
        });
    }

    fn apply_selected_stash(self: &Rc<Self>) {
        let Some(stash) = self.selected_stash.borrow().clone() else {
            return;
        };
        if *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        self.set_stash_busy(true);
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.stash_apply(stash.reference.clone()).await {
                Ok(()) => this.show_toast(tr_args(
                    "Applied {stash}",
                    &[("stash", stash.reference.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Apply Stash Failed"), error.to_string())
                        .await;
                }
            }
            this.set_stash_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_pop_selected_stash(self: &Rc<Self>) {
        let Some(stash) = self.selected_stash.borrow().clone() else {
            return;
        };
        if *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Pop Stash?"))
            .body(tr_args(
                "Apply {stash} to the current working tree and remove it from the stash list if Git applies it successfully?",
                &[("stash", stash.reference.clone())],
            ))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("pop", &tr("Pop Stash"));
        dialog.set_response_appearance("pop", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("pop"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "pop" {
                return;
            }

            this.set_stash_busy(true);
            match backend.stash_pop(stash.reference.clone()).await {
                Ok(()) => this.show_toast(tr_args(
                    "Popped {stash}",
                    &[("stash", stash.reference.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Pop Stash Failed"), error.to_string())
                        .await;
                }
            }
            this.set_stash_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_delete_selected_stash(self: &Rc<Self>) {
        let Some(stash) = self.selected_stash.borrow().clone() else {
            return;
        };
        if *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Delete Stash?"))
            .body(tr_args(
                "Permanently remove {stash} without applying its saved changes? This cannot be undone from Git Desk.",
                &[("stash", stash.reference.clone())],
            ))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete Stash"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "delete" {
                return;
            }

            this.set_stash_busy(true);
            match backend.stash_drop(stash.reference.clone()).await {
                Ok(()) => this.show_toast(tr_args(
                    "Deleted {stash}",
                    &[("stash", stash.reference.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Delete Stash Failed"), error.to_string())
                        .await;
                }
            }
            this.set_stash_busy(false);
            this.refresh_all();
        });
    }

    fn load_tags(self: &Rc<Self>) {
        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            match backend.tags().await {
                Ok(tags) => {
                    let subtitle = match tags.len() {
                        0 => tr("No tags"),
                        count => ntr_args(
                            "{count} tag",
                            "{count} tags",
                            count as u64,
                            &[("count", count.to_string())],
                        ),
                    };
                    this.tags_subtitle.set_label(&subtitle);
                    this.render_tags(&tags);
                }
                Err(error) => {
                    this.tags_subtitle.set_label(&error.to_string());
                    this.render_tags(&[]);
                }
            }
            this.update_tag_action_state();
        });
    }

    fn render_tags(self: &Rc<Self>, tags: &[TagEntry]) {
        clear_list(&self.tag_list);
        self.tag_list.set_visible(!tags.is_empty());
        self.tag_empty.set_visible(tags.is_empty());

        let selected_name = self
            .selected_tag
            .borrow()
            .as_ref()
            .map(|tag| tag.name.clone());
        let mut selected_still_exists = false;

        for tag in tags {
            let tag = tag.clone();
            let short = tag.target.chars().take(8).collect::<String>();
            let kind = if tag.annotated {
                tr("Annotated")
            } else {
                tr("Lightweight")
            };
            let subtitle = if tag.annotated && !tag.subject.trim().is_empty() {
                format!("{kind} · {short} · {}", tag.subject)
            } else {
                format!("{kind} · {short}")
            };
            let row = adw::ActionRow::builder()
                .title(&tag.name)
                .subtitle(&subtitle)
                .activatable(true)
                .build();

            let this = self.clone();
            let selected = tag.clone();
            row.connect_activated(move |_| this.toggle_tag(selected.clone()));
            self.tag_list.append(&row);

            if selected_name.as_deref() == Some(tag.name.as_str()) {
                selected_still_exists = true;
                *self.selected_tag.borrow_mut() = Some(tag.clone());
                self.tag_list.select_row(Some(&row));
            }
        }

        if selected_name.is_some() && !selected_still_exists {
            self.selected_tag.borrow_mut().take();
            if self.inspector_tag_actions.is_visible() {
                self.clear_inspector();
            }
        }
    }

    fn toggle_tag(self: &Rc<Self>, tag: TagEntry) {
        let already_selected = self
            .selected_tag
            .borrow()
            .as_ref()
            .is_some_and(|selected| selected.name == tag.name);

        if already_selected {
            self.selected_tag.borrow_mut().take();
            self.tag_list.unselect_all();
            self.clear_inspector();
            return;
        }

        self.selected_change.borrow_mut().take();
        self.unstaged_list.unselect_all();
        self.staged_list.unselect_all();
        self.selected_history_commit.borrow_mut().take();
        self.history_list.unselect_all();
        self.selected_outgoing_commit.borrow_mut().take();
        self.outgoing_list.unselect_all();
        self.selected_branch.borrow_mut().take();
        self.local_branches.unselect_all();
        self.remote_branches.unselect_all();
        self.selected_stash.borrow_mut().take();
        self.stash_list.unselect_all();
        *self.selected_tag.borrow_mut() = Some(tag.clone());
        self.inspect_tag(tag);
    }

    fn tag_is_selected(&self, name: &str) -> bool {
        self.selected_tag
            .borrow()
            .as_ref()
            .is_some_and(|tag| tag.name == name)
    }

    fn inspect_tag(self: &Rc<Self>, tag: TagEntry) {
        self.inspector_empty.set_visible(false);
        self.inspector_body.set_visible(true);
        self.inspector_commit_actions.set_visible(false);
        self.inspector_history_actions.set_visible(false);
        self.inspector_stash_actions.set_visible(false);
        self.inspector_tag_actions.set_visible(true);
        clear_box(&self.inspector_commit_metadata);
        self.inspector_commit_metadata.set_visible(false);
        clear_box(&self.inspector_files);
        self.diff_view.set_plain_text("");
        self.inspector_title.set_label(&tag.name);
        self.inspector_subtitle.set_visible(true);

        let kind = if tag.annotated {
            tr("Annotated tag")
        } else {
            tr("Lightweight tag")
        };
        let short = tag.target.chars().take(8).collect::<String>();
        self.inspector_subtitle.set_label(&tr_args(
            "{kind} · target {short}",
            &[("kind", kind.clone()), ("short", short.clone())],
        ));
        self.inspector_subtitle.set_tooltip_text(Some(&tag.target));

        self.inspector_message.set_label("");
        self.inspector_message.set_visible(false);

        let type_row = adw::ActionRow::builder()
            .title(tr("Type"))
            .subtitle(&kind)
            .activatable(false)
            .build();
        self.inspector_files.append(&type_row);

        let target_row = adw::ActionRow::builder()
            .title(tr("Target Commit"))
            .subtitle(&tag.target)
            .activatable(false)
            .build();
        self.inspector_files.append(&target_row);
        self.update_tag_action_state();

        if !tag.annotated {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        let tag_name = tag.name.clone();
        glib::spawn_future_local(async move {
            match backend.tag_message(tag_name.clone()).await {
                Ok(message) => {
                    if !this.tag_is_selected(&tag_name) {
                        return;
                    }
                    this.inspector_message.set_label(&message);
                    this.inspector_message
                        .set_visible(!message.trim().is_empty());
                }
                Err(error) => {
                    if this.tag_is_selected(&tag_name) {
                        this.diff_view.set_plain_text(&error.to_string());
                    }
                }
            }
        });
    }

    fn update_tag_action_state(&self) {
        let selected = self.selected_tag.borrow().is_some();
        let can_create = self
            .current_status
            .borrow()
            .as_ref()
            .is_some_and(|status| !status.unborn);
        let busy = *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some();

        self.new_tag_button.set_sensitive(can_create && !busy);
        self.inspector_tag_actions.set_visible(selected);
        self.push_tag_button.set_sensitive(selected && !busy);
        self.delete_tag_button.set_sensitive(selected && !busy);
    }

    fn set_tag_busy(&self, busy: bool) {
        *self.tag_busy.borrow_mut() = busy;
        self.update_tag_action_state();
        self.update_stash_action_state();
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_commit_button_state();
        self.update_merge_controls();

        if busy {
            self.fetch_button.set_sensitive(false);
            self.pull_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
            self.pull_button.remove_css_class("suggested-action");
            self.push_button.remove_css_class("suggested-action");
            self.new_branch_button.set_sensitive(false);
        }
    }

    fn create_tag_dialog(self: &Rc<Self>) {
        if *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        if self
            .current_status
            .borrow()
            .as_ref()
            .is_none_or(|status| status.unborn)
        {
            let this = self.clone();
            glib::spawn_future_local(async move {
                this.show_git_notice_dialog(
                    &tr("Nothing to Tag Yet"),
                    tr("Create the first commit before creating a tag."),
                )
                .await;
            });
            return;
        }

        let name_entry = gtk::Entry::builder()
            .placeholder_text(tr("Tag name, for example v1.0.0"))
            .activates_default(true)
            .build();
        let message_entry = gtk::Entry::builder()
            .placeholder_text(tr("Optional tag message"))
            .activates_default(true)
            .build();

        let hint = gtk::Label::builder()
            .label(tr(
                "Add a message for an annotated tag. Leave it blank for a lightweight tag.",
            ))
            .wrap(true)
            .xalign(0.0)
            .build();
        hint.add_css_class("dim-label");

        let fields = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();
        fields.append(&name_entry);
        fields.append(&message_entry);
        fields.append(&hint);

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Create Tag"))
            .body(tr("Create a tag at the current HEAD commit."))
            .build();
        dialog.set_extra_child(Some(&fields));
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("create", &tr("Create Tag"));
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_response_enabled("create", false);

        let dialog_for_name = dialog.clone();
        name_entry.connect_changed(move |entry| {
            dialog_for_name.set_response_enabled("create", !entry.text().trim().is_empty());
        });

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Notice);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "create" {
                return;
            }

            let name = name_entry.text().trim().to_string();
            let message = message_entry.text().to_string();
            if name.is_empty() {
                return;
            }

            this.set_tag_busy(true);
            match backend.create_tag(name.clone(), message.clone()).await {
                Ok(()) => {
                    let kind = if message.trim().is_empty() {
                        "lightweight"
                    } else {
                        "annotated"
                    };
                    this.show_toast(tr_args(
                        "Created {kind} tag {name}",
                        &[("kind", kind.to_string()), ("name", name.clone())],
                    ));
                }
                Err(error) => {
                    this.show_git_error_dialog(&tr("Create Tag Failed"), error.to_string())
                        .await;
                }
            }
            this.set_tag_busy(false);
            this.refresh_all();
        });
    }

    fn push_selected_tag(self: &Rc<Self>) {
        let Some(tag) = self.selected_tag.borrow().clone() else {
            return;
        };
        if *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let remotes = match backend.remotes().await {
                Ok(remotes) => remotes,
                Err(error) => {
                    this.show_git_error_dialog(&tr("Could Not Load Remotes"), error.to_string())
                        .await;
                    return;
                }
            };
            if remotes.is_empty() {
                this.show_git_notice_dialog(
                    &tr("Add a Remote First"),
                    tr("Configure a remote before pushing this tag."),
                )
                .await;
                return;
            }

            let remote_names: Vec<String> = remotes.iter().map(|(name, _)| name.clone()).collect();
            let default_remote = remote_names
                .iter()
                .find(|name| name.as_str() == "origin")
                .cloned()
                .unwrap_or_else(|| remote_names[0].clone());

            let dialog = adw::AlertDialog::builder()
                .heading(tr_args("Push {tag}?", &[("tag", tag.name.clone())]))
                .body(tr_args(
                    "Push this tag to '{remote}'. Git Desk will not force-update an existing remote tag.",
                    &[("remote", default_remote.clone())],
                ))
                .build();
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("push", &tr("Push Tag"));
            dialog.set_response_appearance("push", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("push"));

            let remote_entry = if remote_names.len() > 1 {
                let entry = gtk::Entry::builder()
                    .placeholder_text(tr("Remote name"))
                    .text(&default_remote)
                    .activates_default(true)
                    .build();
                let hint = gtk::Label::builder()
                    .label(tr_args(
                        "Configured remotes: {remotes}",
                        &[("remotes", remote_names.join(", "))],
                    ))
                    .wrap(true)
                    .xalign(0.0)
                    .build();
                hint.add_css_class("dim-label");
                let fields = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(8)
                    .build();
                fields.append(&entry);
                fields.append(&hint);
                dialog.set_extra_child(Some(&fields));

                let valid_names = remote_names.clone();
                let dialog_for_entry = dialog.clone();
                entry.connect_changed(move |entry| {
                    let value = entry.text();
                    dialog_for_entry.set_response_enabled(
                        "push",
                        valid_names
                            .iter()
                            .any(|name| name.as_str() == value.as_str()),
                    );
                });
                Some(entry)
            } else {
                None
            };

            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "push" {
                return;
            }

            let remote = remote_entry
                .as_ref()
                .map(|entry| entry.text().trim().to_string())
                .unwrap_or(default_remote);
            if !remote_names.iter().any(|name| name == &remote) {
                this.show_git_notice_dialog(
                    &tr("Choose a Configured Remote"),
                    tr("Select one of this repository's configured remotes."),
                )
                .await;
                return;
            }

            if !this.tag_is_selected(&tag.name) {
                return;
            }

            this.set_tag_busy(true);
            match backend.push_tag(remote.clone(), tag.name.clone()).await {
                Ok(()) => this.show_toast(tr_args(
                    "Pushed {tag} to {remote}",
                    &[("tag", tag.name.clone()), ("remote", remote.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Push Tag Failed"), error.to_string())
                        .await;
                }
            }
            this.set_tag_busy(false);
            this.refresh_all();
        });
    }

    fn confirm_delete_selected_tag(self: &Rc<Self>) {
        let Some(tag) = self.selected_tag.borrow().clone() else {
            return;
        };
        if *self.tag_busy.borrow()
            || *self.commit_busy.borrow()
            || *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(tr_args("Delete {tag}?", &[("tag", tag.name.clone())]))
            .body(tr("Delete this local tag? If this tag has already been pushed, the remote tag will remain unchanged."))
            .build();
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete Tag"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Danger);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "delete" {
                return;
            }

            if !this.tag_is_selected(&tag.name) {
                return;
            }

            this.set_tag_busy(true);
            match backend.delete_tag(tag.name.clone()).await {
                Ok(()) => this.show_toast(tr_args(
                    "Deleted local tag {tag}",
                    &[("tag", tag.name.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Delete Tag Failed"), error.to_string())
                        .await;
                }
            }
            this.set_tag_busy(false);
            this.refresh_all();
        });
    }

    fn push(self: &Rc<Self>) {
        if *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let status = match backend.status().await {
                Ok(status) => status,
                Err(error) => {
                    this.changes_subtitle.set_label(&error.to_string());
                    return;
                }
            };

            if status.unborn {
                this.show_git_notice_dialog(
                    &tr("Nothing to Push Yet"),
                    tr("Create the first commit before publishing this branch."),
                )
                .await;
                return;
            }

            if status.detached {
                this.show_git_warning_dialog(
                    &tr("Detached HEAD"),
                    tr("Create or switch to a local branch before pushing."),
                )
                .await;
                return;
            }

            if let Some(upstream) = status.upstream.clone() {
                this.set_sync_busy(Some("push"));
                match backend.push().await {
                    Ok(()) => this.show_toast(tr_args(
                        "Pushed to {upstream}",
                        &[("upstream", upstream.clone())],
                    )),
                    Err(error) => {
                        this.show_git_error_dialog(&tr("Push Failed"), error.to_string())
                            .await;
                    }
                }
                this.set_sync_busy(None);
                this.refresh_all();
                return;
            }

            let remotes = match backend.remotes().await {
                Ok(remotes) => remotes,
                Err(error) => {
                    this.changes_subtitle.set_label(&error.to_string());
                    return;
                }
            };
            if remotes.is_empty() {
                this.changes_subtitle
                    .set_label(&tr("Add a remote before pushing this branch."));
                this.refresh_all();
                return;
            }

            let remote_names: Vec<String> = remotes.iter().map(|(name, _)| name.clone()).collect();
            let default_remote = remote_names
                .iter()
                .find(|name| name.as_str() == "origin")
                .cloned()
                .unwrap_or_else(|| remote_names[0].clone());

            let dialog = adw::AlertDialog::builder()
                .heading(tr("Publish Branch"))
                .body(tr_args(
                    "Push '{branch}' to a remote and set its upstream branch.",
                    &[("branch", status.branch.clone())],
                ))
                .build();
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("push", &tr("Push & Set Upstream"));
            dialog.set_response_appearance("push", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("push"));

            let remote_entry = if remote_names.len() > 1 {
                let entry = gtk::Entry::builder()
                    .placeholder_text(tr("Remote name"))
                    .text(&default_remote)
                    .activates_default(true)
                    .build();
                let hint = gtk::Label::builder()
                    .label(tr_args(
                        "Configured remotes: {remotes}",
                        &[("remotes", remote_names.join(", "))],
                    ))
                    .wrap(true)
                    .xalign(0.0)
                    .build();
                hint.add_css_class("dim-label");
                let fields = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(8)
                    .build();
                fields.append(&entry);
                fields.append(&hint);
                dialog.set_extra_child(Some(&fields));

                let valid_names = remote_names.clone();
                let dialog_for_entry = dialog.clone();
                entry.connect_changed(move |entry| {
                    let value = entry.text();
                    dialog_for_entry.set_response_enabled(
                        "push",
                        valid_names
                            .iter()
                            .any(|name| name.as_str() == value.as_str()),
                    );
                });
                Some(entry)
            } else {
                dialog.set_body(&tr_args(
                    "Push '{branch}' to '{remote}' and set it as the upstream branch?",
                    &[
                        ("branch", status.branch.clone()),
                        ("remote", default_remote.clone()),
                    ],
                ));
                None
            };

            let parent = this
                .root
                .root()
                .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
            apply_alert_eyebrow(&dialog, AlertEyebrow::Confirmation);
            if dialog.choose_future(parent.as_ref()).await.as_str() != "push" {
                return;
            }

            let remote = remote_entry
                .as_ref()
                .map(|entry| entry.text().trim().to_string())
                .unwrap_or(default_remote);
            if !remote_names.iter().any(|name| name == &remote) {
                this.changes_subtitle
                    .set_label(&tr("Choose one of the configured remotes before pushing."));
                return;
            }

            let branch = status.branch.clone();
            this.set_sync_busy(Some("push"));
            match backend
                .push_set_upstream(remote.clone(), branch.clone())
                .await
            {
                Ok(()) => this.show_toast(tr_args(
                    "Published {branch} to {remote}",
                    &[("branch", branch.clone()), ("remote", remote.clone())],
                )),
                Err(error) => {
                    this.show_git_error_dialog(&tr("Push Failed"), error.to_string())
                        .await;
                }
            }
            this.set_sync_busy(None);
            this.refresh_all();
        });
    }

    fn sync(self: &Rc<Self>, action: &'static str) {
        if *self.unpublished_action_busy.borrow()
            || *self.stash_busy.borrow()
            || *self.tag_busy.borrow()
            || *self.merge_busy.borrow()
            || *self.merge_in_progress.borrow()
            || *self.history_action_busy.borrow()
            || self.history_operation_active()
            || self.sync_busy.borrow().is_some()
        {
            return;
        }
        self.set_sync_busy(Some(action));

        let this = self.clone();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move {
            let upstream = this
                .current_status
                .borrow()
                .as_ref()
                .and_then(|status| status.upstream.clone());

            let result = match action {
                "fetch" => backend.fetch().await,
                "pull" => backend.pull().await,
                _ => {
                    this.set_sync_busy(None);
                    return;
                }
            };

            match result {
                Ok(()) => match action {
                    "fetch" => this.show_toast(tr("Fetch completed")),
                    "pull" => {
                        if let Some(upstream) = upstream {
                            this.show_toast(tr_args(
                                "Pulled from {upstream}",
                                &[("upstream", upstream.clone())],
                            ));
                        } else {
                            this.show_toast(tr("Pull completed"));
                        }
                    }
                    _ => {}
                },
                Err(error) => {
                    let message = error.to_string();
                    let diverged_pull = action == "pull"
                        && (message.contains("Not possible to fast-forward")
                            || message.contains("Diverging branches can't be fast-forwarded"));

                    if diverged_pull {
                        this.set_sync_busy(None);
                        this.show_diverged_pull_dialog(upstream.clone()).await;
                        this.refresh_all();
                        return;
                    }

                    let heading = if action == "fetch" {
                        tr("Fetch Failed")
                    } else {
                        tr("Pull Failed")
                    };
                    this.show_git_error_dialog(&heading, message).await;
                }
            }

            this.set_sync_busy(None);
            this.refresh_all();
        });
    }

    fn set_sync_busy(&self, action: Option<&'static str>) {
        *self.sync_busy.borrow_mut() = action;
        self.update_unpublished_commit_actions();
        self.update_history_commit_actions();
        self.update_history_operation_controls();
        self.update_stash_action_state();
        self.update_tag_action_state();

        let fetch_label = if action == Some("fetch") {
            tr("Fetching…")
        } else {
            tr("Fetch")
        };
        let pull_label = if action == Some("pull") {
            tr("Pulling…")
        } else {
            tr("Pull")
        };
        let push_label = if action == Some("push") {
            tr("Pushing…")
        } else {
            tr("Push")
        };
        self.fetch_label.set_label(&fetch_label);
        self.pull_label.set_label(&pull_label);
        self.push_label.set_label(&push_label);

        self.fetch_button.set_sensitive(false);
        self.pull_button.set_sensitive(false);
        self.push_button.set_sensitive(false);
        self.pull_button.remove_css_class("suggested-action");
        self.push_button.remove_css_class("suggested-action");
    }

    fn show_toast(&self, message: impl AsRef<str>) {
        self.toast_overlay
            .add_toast(adw::Toast::new(message.as_ref()));
    }

    async fn show_diverged_pull_dialog(&self, upstream: Option<String>) {
        let body = if let Some(upstream) = upstream {
            tr_args(
                "{upstream} and the current branch both have new commits. Git Desk only fast-forwards when pulling, so it will not merge them automatically.\n\nOpen Branches and choose Merge into Current Branch… for {upstream}.",
                &[("upstream", upstream)],
            )
        } else {
            tr(
                "The current branch and its upstream both have new commits. Git Desk only fast-forwards when pulling, so it will not merge them automatically.\n\nOpen Branches and merge the upstream branch into the current branch.",
            )
        };

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Branches Have Diverged"))
            .body(&body)
            .build();
        apply_alert_eyebrow(&dialog, AlertEyebrow::Warning);
        dialog.add_response("close", &tr("Close"));
        dialog.add_response("branches", &tr("Open Branches"));
        dialog.set_response_appearance("branches", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("branches"));

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        if dialog.choose_future(parent.as_ref()).await.as_str() == "branches" {
            self.nav.select_row(self.nav.row_at_index(2).as_ref());
        }
    }

    async fn show_git_alert_dialog(&self, heading: &str, body: String, eyebrow: AlertEyebrow) {
        let dialog = adw::AlertDialog::builder()
            .heading(tr(heading))
            .body(&body)
            .build();
        dialog.add_response("close", &tr("Close"));
        dialog.set_default_response(Some("close"));
        apply_alert_eyebrow(&dialog, eyebrow);

        let parent = self
            .root
            .root()
            .and_then(|root| root.dynamic_cast::<gtk::Window>().ok());
        let _ = dialog.choose_future(parent.as_ref()).await;
    }

    async fn show_git_notice_dialog(&self, heading: &str, body: String) {
        self.show_git_alert_dialog(heading, body, AlertEyebrow::Notice)
            .await;
    }

    async fn show_git_warning_dialog(&self, heading: &str, body: String) {
        self.show_git_alert_dialog(heading, body, AlertEyebrow::Warning)
            .await;
    }

    async fn show_git_error_dialog(&self, heading: &str, body: String) {
        self.show_git_alert_dialog(heading, body, AlertEyebrow::Error)
            .await;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestedSyncAction {
    Pull,
    Push,
}

fn suggested_sync_action(
    status: &crate::git::models::RepositoryStatus,
    has_remote: bool,
) -> Option<SuggestedSyncAction> {
    if status.unborn || status.detached || !has_remote {
        return None;
    }

    if status.upstream.is_none() {
        return Some(SuggestedSyncAction::Push);
    }

    match (status.ahead, status.behind) {
        (ahead, 0) if ahead > 0 => Some(SuggestedSyncAction::Push),
        (0, behind) if behind > 0 => Some(SuggestedSyncAction::Pull),
        _ => None,
    }
}

fn make_nav() -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .margin_top(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    list.add_css_class("navigation-sidebar");

    for label in [
        tr("Changes"),
        tr("History"),
        tr("Branches"),
        tr("Stashes"),
        tr("Tags"),
    ] {
        append_nav_row(&list, &label);
    }

    append_nav_row(&list, &tr("Git Guide"));
    list.set_header_func(|row, _| {
        if row.index() == 5 {
            let divider = gtk::Separator::new(Orientation::Horizontal);
            divider.set_margin_top(6);
            divider.set_margin_bottom(6);
            divider.set_margin_start(8);
            divider.set_margin_end(8);
            row.set_header(Some(&divider));
        } else {
            row.set_header(None::<&gtk::Widget>);
        }
    });
    list
}

fn append_nav_row(list: &gtk::ListBox, text: &str) {
    let row = gtk::ListBoxRow::new();
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    row.set_child(Some(&label));
    list.append(&row);
}

fn install_history_style() {
    use std::sync::Once;

    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
.history-ref-badge {
    background-color: alpha(@view_fg_color, 0.08);
    border-radius: 999px;
    padding: 2px 7px;
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

fn install_commit_composer_style() {
    use std::sync::Once;

    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
.commit-composer {
    min-height: 0;
    background-color: @view_bg_color;
    border: 1px solid @borders;
    border-radius: 9px;
}

.commit-composer:focus-within {
    border-color: @accent_bg_color;
    box-shadow: inset 0 0 0 1px @accent_bg_color;
}

.commit-composer textview,
.commit-composer textview text {
    background-color: transparent;
    min-height: 0;
    padding: 0;
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

#[allow(clippy::type_complexity)]
fn build_changes_page() -> (
    gtk::Box,
    gtk::Label,
    gtk::Box,
    adw::ActionRow,
    gtk::Button,
    gtk::Button,
    gtk::Box,
    adw::ActionRow,
    gtk::Button,
    gtk::Button,
    gtk::Button,
    gtk::Box,
    gtk::Label,
    gtk::ListBox,
    gtk::Box,
    gtk::ListBox,
    gtk::Box,
    gtk::ListBox,
    gtk::Label,
    gtk::TextBuffer,
    gtk::TextView,
    gtk::Button,
    gtk::Button,
    gtk::Button,
) {
    let page = gtk::Box::new(Orientation::Vertical, 0);
    let header = page_header(&tr("Changes"));
    let subtitle = header.1.clone();
    page.append(&header.0);

    let content_scroller = gtk::ScrolledWindow::builder().vexpand(true).build();
    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(22)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let merge_group = gtk::Box::new(Orientation::Vertical, 8);
    let merge_status_row = adw::ActionRow::builder()
        .title(tr("Merge in Progress"))
        .subtitle(tr("Resolve conflicted files to continue the merge."))
        .build();
    merge_status_row.add_css_class("warning");

    let merge_actions = gtk::Box::new(Orientation::Horizontal, 6);
    merge_actions.set_valign(gtk::Align::Center);

    let complete_merge_button = gtk::Button::with_label(&tr("Complete Merge"));
    complete_merge_button.set_sensitive(false);
    merge_actions.append(&complete_merge_button);

    let abort_merge_button = gtk::Button::with_label(&tr("Abort Merge…"));
    abort_merge_button.add_css_class("destructive-action");
    merge_actions.append(&abort_merge_button);

    merge_status_row.add_suffix(&merge_actions);
    let merge_list = gtk::ListBox::new();
    merge_list.add_css_class("boxed-list");
    merge_list.append(&merge_status_row);
    merge_group.append(&merge_list);
    merge_group.set_visible(false);

    let history_operation_group = gtk::Box::new(Orientation::Vertical, 8);
    let history_operation_status_row = adw::ActionRow::builder()
        .title(tr("History Operation in Progress"))
        .subtitle(tr(
            "Resolve conflicted files to continue the Git operation.",
        ))
        .build();
    history_operation_status_row.add_css_class("warning");

    let history_operation_actions = gtk::Box::new(Orientation::Horizontal, 6);
    history_operation_actions.set_halign(gtk::Align::Start);

    let continue_history_operation_button = gtk::Button::with_label(&tr("Continue"));
    continue_history_operation_button.set_sensitive(false);
    history_operation_actions.append(&continue_history_operation_button);

    let skip_history_operation_button = gtk::Button::with_label(&tr("Skip Cherry-pick"));
    skip_history_operation_button.set_sensitive(false);
    skip_history_operation_button.set_visible(false);
    history_operation_actions.append(&skip_history_operation_button);

    let abort_history_operation_button = gtk::Button::with_label(&tr("Abort…"));
    abort_history_operation_button.add_css_class("destructive-action");
    history_operation_actions.append(&abort_history_operation_button);

    let history_operation_list = gtk::ListBox::new();
    history_operation_list.add_css_class("boxed-list");
    history_operation_list.append(&history_operation_status_row);
    history_operation_group.append(&history_operation_list);
    history_operation_group.append(&history_operation_actions);
    history_operation_group.set_visible(false);

    let outgoing_group = gtk::Box::new(Orientation::Vertical, 8);
    let outgoing_title = gtk::Label::builder()
        .label(tr("Outgoing"))
        .xalign(0.0)
        .build();
    outgoing_title.add_css_class("heading");
    let outgoing_subtitle = gtk::Label::builder()
        .label(tr("Commits ready to push."))
        .xalign(0.0)
        .wrap(true)
        .build();
    outgoing_subtitle.add_css_class("dim-label");
    let outgoing_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    outgoing_list.add_css_class("boxed-list");
    outgoing_group.append(&outgoing_title);
    outgoing_group.append(&outgoing_subtitle);
    outgoing_group.append(&outgoing_list);
    outgoing_group.set_visible(false);

    let unstaged_list = gtk::ListBox::new();
    unstaged_list.add_css_class("boxed-list");
    let stage_all_button = gtk::Button::with_label(&tr("Stage All"));
    stage_all_button.set_valign(gtk::Align::Center);
    stage_all_button.set_sensitive(false);
    let unstaged_group = group_with_action(
        &tr("Changes"),
        &tr("Work not selected for your next commit."),
        &unstaged_list,
        &stage_all_button,
    );

    let staged_list = gtk::ListBox::new();
    staged_list.add_css_class("boxed-list");
    let unstage_all_button = gtk::Button::with_label(&tr("Unstage All"));
    unstage_all_button.set_valign(gtk::Align::Center);
    unstage_all_button.set_sensitive(false);

    install_commit_composer_style();

    // Match the compact resting height of the original native GtkEntry.
    // The TextView is only the editing surface; it must not dictate the
    // composer's resting height.
    let commit_entry_probe = gtk::Entry::builder()
        .placeholder_text(tr("Commit message"))
        .build();
    let (_, commit_compact_height, _, _) = commit_entry_probe.measure(Orientation::Vertical, -1);
    let commit_compact_height = commit_compact_height.max(1);

    let commit_buffer = gtk::TextBuffer::new(None);
    let commit_editor = gtk::TextView::builder()
        .buffer(&commit_buffer)
        .wrap_mode(gtk::WrapMode::WordChar)
        .accepts_tab(false)
        .left_margin(9)
        .right_margin(9)
        .hexpand(true)
        .vexpand(false)
        .build();
    commit_editor.remove_css_class("view");
    commit_editor.set_tooltip_text(Some(&tr("Enter adds a new line · Ctrl+Enter commits")));

    // Let GtkScrolledWindow own the vertical sizing. Keep one compact line
    // visible, propagate the TextView's natural height while it grows, and
    // cap the visible content at six lines before scrolling.
    let metrics = commit_editor.create_pango_context().metrics(None, None);
    let commit_line_height =
        ((metrics.ascent() + metrics.descent() + gtk::pango::SCALE - 1) / gtk::pango::SCALE).max(1);

    // Match the native one-line control rhythm exactly. Center the measured
    // text line inside the measured compact Entry height, accounting for the
    // composer's 1 px border on each side. Keep that same symmetric padding
    // as the composer grows to multiple lines.
    const COMMIT_COMPOSER_BORDER_PX: i32 = 1;
    let commit_inner_height =
        (commit_compact_height - (2 * COMMIT_COMPOSER_BORDER_PX)).max(commit_line_height);
    let commit_vertical_space = (commit_inner_height - commit_line_height).max(0);
    let commit_padding_top = commit_vertical_space / 2;
    let commit_padding_bottom = commit_vertical_space - commit_padding_top;
    commit_editor.set_top_margin(commit_padding_top);
    commit_editor.set_bottom_margin(commit_padding_bottom);

    let commit_max_content_height = commit_compact_height + (5 * commit_line_height);

    let commit_scroller = gtk::ScrolledWindow::builder()
        .child(&commit_editor)
        .hexpand(true)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .valign(gtk::Align::End)
        .build();
    // Give the scroller a real one-line minimum allocation from the first
    // layout pass. min_content_height controls how much content stays visible,
    // but it does not replace the widget's own minimum size request. Keeping
    // this as a minimum still lets propagate_natural_height grow the composer.
    commit_scroller.set_size_request(-1, commit_compact_height);

    // Set the fixed range once. Setting max first also keeps GTK's
    // min <= max invariant valid at every point.
    commit_scroller.set_max_content_height(commit_max_content_height);
    commit_scroller.set_min_content_height(commit_compact_height);
    commit_scroller.add_css_class("commit-composer");
    commit_scroller.set_overflow(gtk::Overflow::Hidden);

    let commit_placeholder = gtk::Label::builder()
        .label(tr("Commit message"))
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(10)
        .margin_top(COMMIT_COMPOSER_BORDER_PX + commit_padding_top)
        .build();
    commit_placeholder.add_css_class("dim-label");
    commit_placeholder.set_can_target(false);

    let commit_overlay = gtk::Overlay::new();
    commit_overlay.set_hexpand(true);
    commit_overlay.set_child(Some(&commit_scroller));
    commit_overlay.add_overlay(&commit_placeholder);

    let placeholder = commit_placeholder.clone();
    let policy_editor = commit_editor.clone();
    let policy_scroller = commit_scroller.clone();
    commit_buffer.connect_changed(move |buffer| {
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        placeholder.set_visible(buffer.text(&start, &end, true).is_empty());

        // Keep the compact composer free of GtkScrolledWindow's vertical
        // scrollbar minimum. Only enable the scrollbar when GTK's own
        // wrapped TextView measurement proves that the content exceeds the
        // six-line viewport. Switching back to Never lets the composer shrink
        // naturally again as content is removed.
        let editor = policy_editor.clone();
        let scroller = policy_scroller.clone();
        gtk::glib::idle_add_local_once(move || {
            let width = editor.width();
            if width <= 0 {
                return;
            }

            let (_, natural_height, _, _) = editor.measure(Orientation::Vertical, width);
            let desired_policy = if natural_height > commit_max_content_height {
                gtk::PolicyType::Automatic
            } else {
                gtk::PolicyType::Never
            };

            if scroller.policy().1 != desired_policy {
                scroller.set_vscrollbar_policy(desired_policy);
                scroller.queue_resize();
            }
        });
    });

    let commit_button = gtk::Button::with_label(&tr("Commit"));
    commit_button.add_css_class("suggested-action");
    commit_button.set_sensitive(false);
    commit_button.set_valign(gtk::Align::End);
    commit_button.set_tooltip_text(Some(&tr("Commit staged changes (Ctrl+Enter)")));

    let commit_bar = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(18)
        .margin_end(18)
        .build();
    commit_bar.append(&commit_overlay);
    commit_bar.append(&commit_button);

    let staged_group = group_with_action(
        &tr("Ready to Commit"),
        &tr("Changes selected for your next saved Git version."),
        &staged_list,
        &unstage_all_button,
    );
    staged_group.set_visible(false);

    let clean_label = gtk::Label::builder()
        .label(tr(
            "Everything is committed. There are no local changes to review.",
        ))
        .xalign(0.0)
        .wrap(true)
        .margin_top(12)
        .build();
    clean_label.add_css_class("dim-label");

    content.append(&merge_group);
    content.append(&history_operation_group);
    content.append(&outgoing_group);
    content.append(&unstaged_group);
    content.append(&staged_group);
    content.append(&clean_label);
    content_scroller.set_child(Some(&content));
    page.append(&content_scroller);

    let separator = gtk::Separator::new(Orientation::Horizontal);
    page.append(&separator);
    page.append(&commit_bar);

    (
        page,
        subtitle,
        merge_group,
        merge_status_row,
        complete_merge_button,
        abort_merge_button,
        history_operation_group,
        history_operation_status_row,
        continue_history_operation_button,
        skip_history_operation_button,
        abort_history_operation_button,
        outgoing_group,
        outgoing_subtitle,
        outgoing_list,
        unstaged_group,
        unstaged_list,
        staged_group,
        staged_list,
        clean_label,
        commit_buffer,
        commit_editor,
        commit_button,
        stage_all_button,
        unstage_all_button,
    )
}

fn build_history_page() -> (gtk::Box, gtk::Label, gtk::ListBox) {
    install_history_style();

    let page = gtk::Box::new(Orientation::Vertical, 0);
    let header = page_header(&tr("History"));
    let subtitle = header.1.clone();
    page.append(&header.0);

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(true)
        .child(&list)
        .build();

    // CBAPL-001 Surface Card: grow with the timeline while it fits, then let
    // only the timeline scroll. The outer frame owns the surface, radius and
    // clipping; the inner ListBox remains flat.
    let card = gtk::Frame::new(None);
    card.add_css_class("card");
    card.set_hexpand(true);
    card.set_overflow(gtk::Overflow::Hidden);
    card.set_child(Some(&scroller));

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .margin_top(6)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&card);
    page.append(&content);
    (page, subtitle, list)
}

fn build_branches_page() -> (
    gtk::Box,
    gtk::Label,
    gtk::ListBox,
    gtk::ListBox,
    gtk::ListBox,
    gtk::Button,
    gtk::Button,
) {
    let page = gtk::Box::new(Orientation::Vertical, 0);
    let header = page_header(&tr("Branches"));
    let subtitle = header.1.clone();
    page.append(&header.0);

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let local = gtk::ListBox::new();
    local.add_css_class("boxed-list");
    let remotes = gtk::ListBox::new();
    remotes.add_css_class("boxed-list");
    remotes.set_selection_mode(gtk::SelectionMode::None);
    let remote = gtk::ListBox::new();
    remote.add_css_class("boxed-list");

    let new_branch = gtk::Button::with_label(&tr("New Branch"));
    new_branch.set_valign(gtk::Align::Center);
    let add_remote = gtk::Button::with_label(&tr("Add Remote"));
    add_remote.set_valign(gtk::Align::Center);

    content.append(&group_with_action(
        &tr("Local Branches"),
        &tr("Branches stored in this repository."),
        &local,
        &new_branch,
    ));
    content.append(&group_with_action(
        &tr("Remotes"),
        &tr("Remote repositories connected to this project."),
        &remotes,
        &add_remote,
    ));
    content.append(&group(
        &tr("Remote Branches"),
        &tr("Branches discovered from configured remotes."),
        &remote,
    ));

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&content)
        .build();
    page.append(&scroller);
    (
        page, subtitle, local, remotes, remote, new_branch, add_remote,
    )
}

fn build_stashes_page() -> (gtk::Box, gtk::Label, gtk::ListBox, gtk::Label, gtk::Button) {
    let page = gtk::Box::new(Orientation::Vertical, 0);
    let header = page_header(&tr("Stashes"));
    let subtitle = header.1.clone();
    page.append(&header.0);

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let stash_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    stash_list.add_css_class("boxed-list");

    let new_stash_button = gtk::Button::with_label(&tr("Stash Changes…"));
    new_stash_button.set_valign(gtk::Align::Center);

    let stash_group = group_with_action(
        &tr("Saved Work"),
        &tr("Temporarily saved working-tree and staged changes."),
        &stash_list,
        &new_stash_button,
    );

    let stash_empty = gtk::Label::builder()
        .label(tr("No stashes yet. Save work here when you need a clean working tree without committing it."))
        .xalign(0.0)
        .wrap(true)
        .margin_top(4)
        .build();
    stash_empty.add_css_class("dim-label");

    content.append(&stash_group);
    content.append(&stash_empty);

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&content)
        .build();
    page.append(&scroller);

    (page, subtitle, stash_list, stash_empty, new_stash_button)
}

fn build_tags_page() -> (gtk::Box, gtk::Label, gtk::ListBox, gtk::Label, gtk::Button) {
    let page = gtk::Box::new(Orientation::Vertical, 0);
    let header = page_header(&tr("Tags"));
    let subtitle = header.1.clone();
    page.append(&header.0);

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let tag_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    tag_list.add_css_class("boxed-list");

    let new_tag_button = gtk::Button::with_label(&tr("New Tag…"));
    new_tag_button.set_valign(gtk::Align::Center);
    new_tag_button.set_sensitive(false);

    let tag_group = group_with_action(
        &tr("Repository Tags"),
        &tr("Named references to important commits, such as releases and milestones."),
        &tag_list,
        &new_tag_button,
    );

    let tag_empty = gtk::Label::builder()
        .label(tr("No tags yet. Create a tag at the current commit when you want to mark a release or milestone."))
        .xalign(0.0)
        .wrap(true)
        .margin_top(4)
        .build();
    tag_empty.add_css_class("dim-label");

    content.append(&tag_group);
    content.append(&tag_empty);

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&content)
        .build();
    page.append(&scroller);

    (page, subtitle, tag_list, tag_empty, new_tag_button)
}

fn page_header(title: &str) -> (gtk::Box, gtk::Label) {
    let box_ = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .margin_top(14)
        .margin_bottom(12)
        .margin_start(18)
        .margin_end(18)
        .build();
    let heading = gtk::Label::builder().label(title).xalign(0.0).build();
    heading.add_css_class("title-3");
    let subtitle = gtk::Label::builder().xalign(0.0).build();
    subtitle.add_css_class("dim-label");
    box_.append(&heading);
    box_.append(&subtitle);
    (box_, subtitle)
}

fn group_with_action(
    title: &str,
    description: &str,
    list: &gtk::ListBox,
    action: &gtk::Button,
) -> gtk::Box {
    let box_ = gtk::Box::new(Orientation::Vertical, 8);

    let title_row = gtk::Box::new(Orientation::Horizontal, 8);
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .hexpand(true)
        .build();
    title.add_css_class("heading");
    title_row.append(&title);
    title_row.append(action);

    let description = gtk::Label::builder()
        .label(description)
        .xalign(0.0)
        .wrap(true)
        .build();
    description.add_css_class("dim-label");

    box_.append(&title_row);
    box_.append(&description);
    box_.append(list);
    box_
}

fn group(title: &str, description: &str, list: &gtk::ListBox) -> gtk::Box {
    let box_ = gtk::Box::new(Orientation::Vertical, 8);
    let title = gtk::Label::builder().label(title).xalign(0.0).build();
    title.add_css_class("heading");
    let description = gtk::Label::builder()
        .label(description)
        .xalign(0.0)
        .wrap(true)
        .build();
    description.add_css_class("dim-label");
    box_.append(&title);
    box_.append(&description);
    box_.append(list);
    box_
}

fn history_commit_row(graph_row: &GraphRow) -> gtk::ListBoxRow {
    let commit = &graph_row.commit;
    let short = commit.id.chars().take(8).collect::<String>();
    let relative_time = relative_commit_time(commit.unix_time);
    let (head_ref, tags, refs) = history_decorations(&commit.decorations);

    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    row.set_selectable(true);

    let content = gtk::Box::new(Orientation::Horizontal, 0);
    content.set_margin_start(18);
    content.set_margin_end(12);

    let graph = graph_widget(graph_row);
    content.append(&graph);

    let details = gtk::Box::new(Orientation::Vertical, 3);
    details.set_hexpand(true);
    details.set_margin_top(9);
    details.set_margin_bottom(9);

    let title_line = gtk::Box::new(Orientation::Horizontal, 6);
    title_line.set_hexpand(true);

    let subject = gtk::Label::builder()
        .label(&commit.subject)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    subject.set_tooltip_text(Some(commit.subject.as_str()));
    title_line.append(&subject);

    if let Some(head_ref) = head_ref {
        title_line.append(&history_decoration_badge(&head_ref, false));
    }

    if !tags.is_empty() {
        title_line.append(&history_decoration_badge(&tags.join(" · "), false));
    }

    if !refs.is_empty() {
        title_line.append(&history_decoration_badge(&refs.join(" · "), true));
    }

    details.append(&title_line);

    let subtitle = gtk::Label::builder()
        .label(format!(
            "{} · {} · {short}",
            commit.author_name, relative_time
        ))
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    subtitle.add_css_class("caption");
    subtitle.add_css_class("dim-label");
    details.append(&subtitle);

    content.append(&details);
    row.set_child(Some(&content));
    row
}

fn graph_widget(row: &GraphRow) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    let lanes = row.lane_count.max(1);
    area.set_content_width((lanes * 16 + 16) as i32);
    area.set_content_height(50);

    let lane = row.lane;
    let parent_lanes = row.parent_lanes.clone();
    let before_count = row.before.len();
    let after_count = row.after.len();
    let starts_lane = row.starts_lane;

    area.set_draw_func(move |_area, cr, _width, height| {
        let spacing = 16.0;
        let top = 0.0;
        let mid = f64::from(height) / 2.0;
        let bottom = f64::from(height);

        cr.set_line_width(2.0);
        cr.set_source_rgba(0.65, 0.65, 0.65, 0.9);

        for index in 0..before_count.max(after_count).max(1) {
            let x = 12.0 + index as f64 * spacing;
            if index < before_count && !(starts_lane && index == lane) {
                cr.move_to(x, top);
                cr.line_to(x, mid);
                let _ = cr.stroke();
            }
            if index < after_count {
                cr.move_to(x, mid);
                cr.line_to(x, bottom);
                let _ = cr.stroke();
            }
        }

        let x = 12.0 + lane as f64 * spacing;
        for parent_lane in &parent_lanes {
            let parent_x = 12.0 + *parent_lane as f64 * spacing;
            cr.move_to(x, mid);
            cr.curve_to(x, mid + 8.0, parent_x, mid + 8.0, parent_x, bottom);
            let _ = cr.stroke();
        }

        cr.arc(x, mid, 4.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    });

    area
}

fn branch_subtitle(
    branch: &str,
    upstream: Option<&str>,
    ahead: u32,
    behind: u32,
    unborn: bool,
    detached: bool,
) -> String {
    if detached {
        return tr("Detached HEAD · no current branch");
    }
    if unborn {
        return tr_args(
            "{branch} · no commits yet",
            &[("branch", branch.to_string())],
        );
    }

    match upstream {
        Some(upstream) if ahead > 0 || behind > 0 => tr_args(
            "{branch} · {upstream} · {ahead} ahead · {behind} behind",
            &[
                ("branch", branch.to_string()),
                ("upstream", upstream.to_string()),
                ("ahead", ahead.to_string()),
                ("behind", behind.to_string()),
            ],
        ),
        Some(upstream) => tr_args(
            "{branch} · up to date with {upstream}",
            &[
                ("branch", branch.to_string()),
                ("upstream", upstream.to_string()),
            ],
        ),
        None => tr_args(
            "{branch} · no upstream branch",
            &[("branch", branch.to_string())],
        ),
    }
}

fn commit_body(message: &str) -> String {
    let mut lines = message.lines();
    let _ = lines.next();
    lines
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn history_decorations(decorations: &[String]) -> (Option<String>, Vec<String>, Vec<String>) {
    let mut head_ref = None;
    let mut tags = Vec::new();
    let mut refs = Vec::new();

    for decoration in decorations {
        let decoration = decoration.trim();
        if decoration.is_empty() {
            continue;
        }

        if decoration == "HEAD" {
            head_ref = Some("HEAD".into());
        } else if let Some(branch) = decoration.strip_prefix("HEAD -> ") {
            head_ref = Some(format!("HEAD → {}", branch.trim()));
        } else if let Some(tag) = decoration.strip_prefix("tag: ") {
            tags.push(format!("Tag {}", tag.trim()));
        } else {
            refs.push(decoration.to_string());
        }
    }

    (head_ref, tags, refs)
}

fn history_decoration_badge(text: &str, dim: bool) -> gtk::Box {
    let badge = gtk::Box::new(Orientation::Horizontal, 0);
    badge.add_css_class("history-ref-badge");
    badge.set_valign(gtk::Align::Center);

    let label = gtk::Label::new(Some(text));
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(28);
    label.set_tooltip_text(Some(text));
    label.add_css_class("caption");
    if dim {
        label.add_css_class("dim-label");
    } else {
        label.add_css_class("accent");
    }
    badge.append(&label);
    badge
}

fn history_boundary_row(text: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(30)
        .margin_end(14)
        .build();
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    row.set_child(Some(&label));
    row
}

fn relative_commit_time(unix_time: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(unix_time);
    let seconds = now.saturating_sub(unix_time).max(0) as u64;

    if seconds < 60 {
        return tr("just now");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return ntr_args(
            "{count} minute ago",
            "{count} minutes ago",
            minutes,
            &[("count", minutes.to_string())],
        );
    }

    let hours = minutes / 60;
    if hours < 24 {
        return ntr_args(
            "{count} hour ago",
            "{count} hours ago",
            hours,
            &[("count", hours.to_string())],
        );
    }

    let days = hours / 24;
    if days < 30 {
        return ntr_args(
            "{count} day ago",
            "{count} days ago",
            days,
            &[("count", days.to_string())],
        );
    }

    let months = days / 30;
    if months < 12 {
        return ntr_args(
            "{count} month ago",
            "{count} months ago",
            months,
            &[("count", months.to_string())],
        );
    }

    let years = days / 365;
    ntr_args(
        "{count} year ago",
        "{count} years ago",
        years.max(1),
        &[("count", years.max(1).to_string())],
    )
}

fn compact_git_datetime(value: &str) -> String {
    let Some((date, rest)) = value.split_once('T') else {
        return value.to_string();
    };
    let mut date_parts = date.split('-');
    let (Some(year), Some(month), Some(day)) =
        (date_parts.next(), date_parts.next(), date_parts.next())
    else {
        return value.to_string();
    };
    if date_parts.next().is_some() {
        return value.to_string();
    }

    let month = match month {
        "01" => tr("Jan"),
        "02" => tr("Feb"),
        "03" => tr("Mar"),
        "04" => tr("Apr"),
        "05" => tr("May"),
        "06" => tr("Jun"),
        "07" => tr("Jul"),
        "08" => tr("Aug"),
        "09" => tr("Sep"),
        "10" => tr("Oct"),
        "11" => tr("Nov"),
        "12" => tr("Dec"),
        _ => return value.to_string(),
    };
    let day = day.trim_start_matches('0');
    let day = if day.is_empty() { "0" } else { day };
    let time = rest.get(..5).unwrap_or(rest);

    format!("{day} {month} {year}, {time}")
}

fn inspector_metadata_row(label: &str, value: &str, tooltip: Option<&str>) -> gtk::Box {
    let row = gtk::Box::new(Orientation::Horizontal, 12);

    let key = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .width_chars(9)
        .build();
    key.add_css_class("caption");
    key.add_css_class("dim-label");
    row.append(&key);

    let value = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .hexpand(true)
        .build();
    if let Some(tooltip) = tooltip {
        value.set_tooltip_text(Some(tooltip));
    }
    row.append(&value);

    row
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn clear_box(box_: &gtk::Box) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
}
