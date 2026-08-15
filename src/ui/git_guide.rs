use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{Orientation, glib};

use crate::{
    i18n::{ntr_args, tr},
    services::guide_personal::{GuidePersonalData, GuidePersonalStore},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuideCategory {
    Favorites,
    Notes,
    All,
    GettingStarted,
    Changes,
    Commits,
    Branches,
    Sync,
    StashesTags,
    History,
    Recovery,
}

impl GuideCategory {
    const FILTERS: [Self; 11] = [
        Self::Favorites,
        Self::Notes,
        Self::All,
        Self::GettingStarted,
        Self::Changes,
        Self::Commits,
        Self::Branches,
        Self::Sync,
        Self::StashesTags,
        Self::History,
        Self::Recovery,
    ];

    fn label(self) -> String {
        match self {
            Self::Favorites => tr("Favorites"),
            Self::Notes => tr("With Notes"),
            Self::All => tr("All Topics"),
            Self::GettingStarted => tr("Getting Started"),
            Self::Changes => tr("Changes"),
            Self::Commits => tr("Commits"),
            Self::Branches => tr("Branches"),
            Self::Sync => tr("Remotes & Sync"),
            Self::StashesTags => tr("Stashes & Tags"),
            Self::History => tr("History"),
            Self::Recovery => tr("Conflicts & Recovery"),
        }
    }
}

#[derive(Clone)]
struct GuideEntry {
    id: &'static str,
    category: GuideCategory,
    title: String,
    summary: String,
    in_git_desk: String,
    terms: String,
    related: String,
    keywords: &'static str,
}

impl GuideEntry {
    fn matches(&self, query: &str, category: GuideCategory, personal: &GuidePersonalData) -> bool {
        match category {
            GuideCategory::Favorites if !personal.favorites.contains(self.id) => return false,
            GuideCategory::Notes
                if !personal
                    .notes
                    .get(self.id)
                    .is_some_and(|note| !note.trim().is_empty()) =>
            {
                return false;
            }
            GuideCategory::All | GuideCategory::Favorites | GuideCategory::Notes => {}
            _ if self.category != category => return false,
            _ => {}
        }
        if query.is_empty() {
            return true;
        }

        let haystack = format!(
            "{} {} {} {} {} {}",
            self.title, self.summary, self.in_git_desk, self.terms, self.related, self.keywords
        )
        .to_lowercase();

        query
            .split_whitespace()
            .all(|token| haystack.contains(token))
    }
}

#[derive(Clone)]
pub struct GitGuideView {
    pub root: gtk::Box,
    pub sidebar: gtk::Box,
    pub stack: gtk::Stack,
    pub sidebar_toggle: gtk::ToggleButton,
    pub outline_list: gtk::ListBox,
}

impl GitGuideView {
    pub fn new() -> Self {
        install_git_guide_style();

        let entries = Rc::new(guide_entries());
        let visible_entries = Rc::new(RefCell::new(Vec::<usize>::new()));
        let selected_category = Rc::new(RefCell::new(GuideCategory::All));
        let personal_store = Rc::new(GuidePersonalStore::new());
        let personal = Rc::new(RefCell::new(personal_store.load()));
        let current_entry_id = Rc::new(RefCell::new(None::<&'static str>));
        let loading_favorite = Rc::new(Cell::new(false));
        let loading_note = Rc::new(Cell::new(false));

        let root = gtk::Box::new(Orientation::Vertical, 0);

        let guide_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeftRight)
            .transition_duration(160)
            .hexpand(true)
            .vexpand(true)
            .build();

        let overview = gtk::Box::new(Orientation::Vertical, 0);
        let heading_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(16)
            .margin_start(18)
            .margin_end(18)
            .build();
        let heading = gtk::Label::builder()
            .label(tr("Git Guide"))
            .xalign(0.0)
            .build();
        heading.add_css_class("title-2");
        let intro = gtk::Label::builder()
            .label(tr("Browse by workflow or search for a Git term. Short explanations help you get started and double as a quick reference."))
            .xalign(0.0)
            .wrap(true)
            .build();
        intro.add_css_class("dim-label");
        heading_box.append(&heading);
        heading_box.append(&intro);
        overview.append(&heading_box);

        let tools = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(14)
            .margin_start(18)
            .margin_end(18)
            .build();

        let search = gtk::SearchEntry::builder()
            .placeholder_text(tr("Search terms, commands, or workflows…"))
            .hexpand(true)
            .build();
        search.set_tooltip_text(Some(&tr("Search the Git Guide (Ctrl+F)")));
        tools.append(&search);

        let filter_box = gtk::Box::new(Orientation::Horizontal, 0);
        filter_box.add_css_class("linked");
        filter_box.add_css_class("cbapl-segmented-strip");

        let mut filter_buttons = Vec::new();
        for category in GuideCategory::FILTERS.iter().copied() {
            let button = gtk::ToggleButton::with_label(&category.label());
            button.add_css_class("cbapl-segment");
            if let Some(first) = filter_buttons.first() {
                button.set_group(Some(first));
            }
            if category == GuideCategory::All {
                button.set_active(true);
            }
            filter_box.append(&button);
            filter_buttons.push(button);
        }
        let notes_filter_button = filter_buttons[1].clone();
        let all_filter_button = filter_buttons[2].clone();
        notes_filter_button.set_visible(
            personal
                .borrow()
                .notes
                .values()
                .any(|note| !note.trim().is_empty()),
        );

        // CBAPL-003 Adaptive Segmented Strip: follow the natural width while
        // all segments fit, shrink to the available width on overflow, and
        // scroll only the inner strip. Use an external scrollbar so the
        // scrollbar is a separate layout row and can never cover the controls.
        let filter_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .hexpand(false)
            .child(&filter_box)
            .build();

        let filter_adjustment = filter_scroller.hadjustment();
        let filter_scrollbar =
            gtk::Scrollbar::new(Orientation::Horizontal, Some(&filter_adjustment));
        filter_scrollbar.set_hexpand(true);
        filter_scrollbar.set_visible(false);

        // Show the dedicated scrollbar only while the natural strip is wider
        // than its viewport. Adjustment::changed also follows window resizing.
        let scrollbar_for_adjustment = filter_scrollbar.clone();
        filter_adjustment.connect_changed(move |adjustment| {
            let has_overflow = adjustment.upper() > adjustment.page_size() + 0.5;
            scrollbar_for_adjustment.set_visible(has_overflow);
        });

        let filter_layout = gtk::Box::new(Orientation::Vertical, 0);
        filter_layout.append(&filter_scroller);
        filter_layout.append(&filter_scrollbar);

        // The outer card is the single shape owner. Individual segments remain
        // square and are clipped by this rounded boundary at the strip edges.
        let filter_card = gtk::Frame::new(None);
        filter_card.add_css_class("card");
        filter_card.set_halign(gtk::Align::Start);
        filter_card.set_hexpand(false);
        filter_card.set_overflow(gtk::Overflow::Hidden);
        filter_card.set_child(Some(&filter_layout));
        tools.append(&filter_card);

        let result_count = gtk::Label::builder().xalign(0.0).build();
        result_count.add_css_class("dim-label");
        tools.append(&result_count);
        overview.append(&tools);

        let result_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        result_list.add_css_class("boxed-list");
        result_list.add_css_class("git-guide-results-list");
        result_list.set_show_separators(true);

        let results_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .child(&result_list)
            .build();

        // The outer container is the single shape owner. Result rows remain
        // square and the boxed list is clipped to this rounded boundary.
        let results_card = gtk::Box::new(Orientation::Vertical, 0);
        results_card.add_css_class("git-guide-results-container");
        results_card.set_hexpand(true);
        results_card.set_overflow(gtk::Overflow::Hidden);
        results_card.set_margin_top(10);
        results_card.set_margin_bottom(18);
        results_card.set_margin_start(18);
        results_card.set_margin_end(18);
        results_card.append(&results_scroller);

        let empty = gtk::Label::builder()
            .label(tr(
                "No matching topics. Try another term or choose All Topics.",
            ))
            .xalign(0.0)
            .wrap(true)
            .margin_top(18)
            .margin_start(18)
            .margin_end(18)
            .visible(false)
            .build();
        empty.add_css_class("dim-label");

        let results_box = gtk::Box::new(Orientation::Vertical, 0);
        results_box.set_vexpand(true);
        results_box.append(&empty);
        results_box.append(&results_card);
        overview.append(&results_box);

        // Document detail: primary article content with a contextual right outline.
        let detail = gtk::Box::new(Orientation::Vertical, 0);
        let detail_header = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .margin_top(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("flat");
        back.set_tooltip_text(Some(&tr("Back to Git Guide")));
        detail_header.append(&back);
        let detail_header_title = gtk::Label::builder()
            .label(tr("Git Guide"))
            .xalign(0.0)
            .hexpand(true)
            .build();
        detail_header_title.add_css_class("heading");
        detail_header.append(&detail_header_title);

        let favorite_button = gtk::ToggleButton::new();
        favorite_button.set_icon_name("non-starred-symbolic");
        favorite_button.add_css_class("flat");
        favorite_button.set_tooltip_text(Some(&tr("Add to Favorites")));
        detail_header.append(&favorite_button);

        let outline_toggle = gtk::ToggleButton::new();
        outline_toggle.set_icon_name("panel-right-symbolic");
        outline_toggle.add_css_class("flat");
        outline_toggle.set_tooltip_text(Some(&tr("Show or hide document sidebar")));
        outline_toggle.set_visible(false);
        detail_header.append(&outline_toggle);
        detail.append(&detail_header);

        let detail_content = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(20)
            .margin_top(18)
            .margin_bottom(32)
            .margin_start(24)
            .margin_end(24)
            .build();

        let overview_section = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();
        let detail_category = gtk::Label::builder().xalign(0.0).build();
        detail_category.add_css_class("dim-label");
        let detail_title = gtk::Label::builder().xalign(0.0).wrap(true).build();
        detail_title.add_css_class("title-2");
        let detail_summary = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .build();
        overview_section.append(&detail_category);
        overview_section.append(&detail_title);
        overview_section.append(&detail_summary);
        detail_content.append(&overview_section);

        detail_content.append(&gtk::Separator::new(Orientation::Horizontal));
        let in_git_desk_section = guide_section(&tr("In Git Desk"));
        let in_git_desk = section_body();
        in_git_desk_section.append(&in_git_desk);
        detail_content.append(&in_git_desk_section);

        detail_content.append(&gtk::Separator::new(Orientation::Horizontal));
        let terms_section = guide_section(&tr("Git Terminology"));
        let terms = section_body();
        terms_section.append(&terms);
        detail_content.append(&terms_section);

        detail_content.append(&gtk::Separator::new(Orientation::Horizontal));
        let related_section = guide_section(&tr("Related"));
        let related = section_body();
        related_section.append(&related);
        detail_content.append(&related_section);

        let detail_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&detail_content)
            .build();

        let sidebar_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();

        let sidebar_modes = gtk::Box::new(Orientation::Horizontal, 0);
        sidebar_modes.set_homogeneous(true);
        sidebar_modes.add_css_class("linked");
        let outline_mode = gtk::ToggleButton::with_label(&tr("On this page"));
        outline_mode.set_hexpand(true);
        outline_mode.set_active(true);
        let notes_mode = gtk::ToggleButton::with_label(&tr("Notes"));
        notes_mode.set_hexpand(true);
        notes_mode.set_group(Some(&outline_mode));
        sidebar_modes.append(&outline_mode);
        sidebar_modes.append(&notes_mode);
        sidebar_box.append(&sidebar_modes);

        let sidebar_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(120)
            .vexpand(true)
            .build();

        let outline_page = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .build();
        let outline_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        outline_list.add_css_class("navigation-sidebar");

        let overview_outline_row = gtk::ListBoxRow::new();
        overview_outline_row.set_activatable(true);
        let overview_outline_label = gtk::Label::builder()
            .xalign(0.0)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .build();
        overview_outline_row.set_child(Some(&overview_outline_label));
        outline_list.append(&overview_outline_row);

        for text in [tr("In Git Desk"), tr("Git Terminology"), tr("Related")] {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            let label = gtk::Label::builder()
                .label(&text)
                .xalign(0.0)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(10)
                .margin_end(10)
                .build();
            row.set_child(Some(&label));
            outline_list.append(&row);
        }
        outline_page.append(&outline_list);
        sidebar_stack.add_named(&outline_page, Some("outline"));

        let notes_page = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .vexpand(true)
            .build();
        let notes_heading = gtk::Label::builder()
            .label(tr("Personal note"))
            .xalign(0.0)
            .build();
        notes_heading.add_css_class("heading");
        notes_page.append(&notes_heading);
        let notes_help = gtk::Label::builder()
            .label(tr("Keep a private note for this Git Guide topic."))
            .xalign(0.0)
            .wrap(true)
            .build();
        notes_help.add_css_class("dim-label");
        notes_page.append(&notes_help);

        let notes_text = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .vexpand(true)
            .top_margin(10)
            .bottom_margin(10)
            .left_margin(10)
            .right_margin(10)
            .build();
        let notes_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(180)
            .vexpand(true)
            .child(&notes_text)
            .build();
        let notes_frame = gtk::Frame::new(None);
        notes_frame.add_css_class("card");
        notes_frame.set_overflow(gtk::Overflow::Hidden);
        notes_frame.set_vexpand(true);
        notes_frame.set_child(Some(&notes_scroller));
        notes_page.append(&notes_frame);

        let notes_saved = gtk::Label::builder()
            .label(tr("Saved automatically on this device"))
            .xalign(0.0)
            .build();
        notes_saved.add_css_class("dim-label");
        notes_page.append(&notes_saved);
        sidebar_stack.add_named(&notes_page, Some("notes"));
        sidebar_stack.set_visible_child_name("outline");
        sidebar_box.append(&sidebar_stack);

        let stack_for_outline_mode = sidebar_stack.clone();
        outline_mode.connect_toggled(move |button| {
            if button.is_active() {
                stack_for_outline_mode.set_visible_child_name("outline");
            }
        });
        let stack_for_notes_mode = sidebar_stack.clone();
        notes_mode.connect_toggled(move |button| {
            if button.is_active() {
                stack_for_notes_mode.set_visible_child_name("notes");
            }
        });

        // The Guide contributes main document content and contextual sidebar
        // content separately. RepositoryView owns the single app-wide right
        // OverlaySplitView used by both Git Inspector and Guide Context.
        detail.append(&detail_scroller);

        let outline_targets = Rc::new(vec![
            overview_section.clone(),
            in_git_desk_section.clone(),
            terms_section.clone(),
            related_section.clone(),
        ]);

        // Outline click -> section.
        let targets_for_outline = outline_targets.clone();
        let content_for_outline = detail_content.clone();
        let scroller_for_outline = detail_scroller.clone();
        outline_list.connect_row_activated(move |_, row| {
            let Some(target) = targets_for_outline.get(row.index().max(0) as usize) else {
                return;
            };
            if let Some(bounds) = target.compute_bounds(&content_for_outline) {
                let adjustment = scroller_for_outline.vadjustment();
                let max_value =
                    (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
                let target_value = (bounds.y() as f64 - 8.0).clamp(adjustment.lower(), max_value);
                adjustment.set_value(target_value);
            }
        });

        // Scroll position -> active outline row.
        let list_for_scroll = outline_list.clone();
        let targets_for_scroll = outline_targets.clone();
        let content_for_scroll = detail_content.clone();
        detail_scroller
            .vadjustment()
            .connect_value_changed(move |adjustment| {
                let marker = adjustment.value() + 64.0;
                let mut active = 0usize;
                for (index, target) in targets_for_scroll.iter().enumerate() {
                    let Some(bounds) = target.compute_bounds(&content_for_scroll) else {
                        continue;
                    };
                    if bounds.y() as f64 <= marker {
                        active = index;
                    } else {
                        break;
                    }
                }
                if list_for_scroll
                    .selected_row()
                    .as_ref()
                    .map(|row| row.index())
                    != Some(active as i32)
                {
                    list_for_scroll
                        .select_row(list_for_scroll.row_at_index(active as i32).as_ref());
                }
            });
        outline_list.select_row(outline_list.row_at_index(0).as_ref());

        guide_stack.add_named(&overview, Some("overview"));
        guide_stack.add_named(&detail, Some("detail"));
        guide_stack.set_visible_child_name("overview");
        root.append(&guide_stack);

        rebuild_results(
            &result_list,
            &result_count,
            &empty,
            &results_card,
            &visible_entries,
            &entries,
            &personal.borrow(),
            "",
            GuideCategory::All,
        );

        let list_for_search = result_list.clone();
        let count_for_search = result_count.clone();
        let empty_for_search = empty.clone();
        let card_for_search = results_card.clone();
        let visible_for_search = visible_entries.clone();
        let entries_for_search = entries.clone();
        let category_for_search = selected_category.clone();
        let personal_for_search = personal.clone();
        search.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            rebuild_results(
                &list_for_search,
                &count_for_search,
                &empty_for_search,
                &card_for_search,
                &visible_for_search,
                &entries_for_search,
                &personal_for_search.borrow(),
                query.trim(),
                *category_for_search.borrow(),
            );
        });

        for (category, button) in GuideCategory::FILTERS.iter().copied().zip(filter_buttons) {
            let search_for_filter = search.clone();
            let list_for_filter = result_list.clone();
            let count_for_filter = result_count.clone();
            let empty_for_filter = empty.clone();
            let card_for_filter = results_card.clone();
            let visible_for_filter = visible_entries.clone();
            let entries_for_filter = entries.clone();
            let selected_for_filter = selected_category.clone();
            let personal_for_filter = personal.clone();
            button.connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                *selected_for_filter.borrow_mut() = category;
                let query = search_for_filter.text().to_lowercase();
                rebuild_results(
                    &list_for_filter,
                    &count_for_filter,
                    &empty_for_filter,
                    &card_for_filter,
                    &visible_for_filter,
                    &entries_for_filter,
                    &personal_for_filter.borrow(),
                    query.trim(),
                    category,
                );
            });
        }

        let list_for_favorite = result_list.clone();
        let count_for_favorite = result_count.clone();
        let empty_for_favorite = empty.clone();
        let card_for_favorite = results_card.clone();
        let visible_for_favorite = visible_entries.clone();
        let entries_for_favorite = entries.clone();
        let search_for_favorite = search.clone();
        let category_for_favorite = selected_category.clone();
        let current_for_favorite = current_entry_id.clone();
        let personal_for_favorite = personal.clone();
        let store_for_favorite = personal_store.clone();
        let loading_for_favorite = loading_favorite.clone();
        favorite_button.connect_toggled(move |button| {
            if loading_for_favorite.get() {
                return;
            }
            let Some(entry_id) = *current_for_favorite.borrow() else {
                return;
            };

            {
                let mut data = personal_for_favorite.borrow_mut();
                if button.is_active() {
                    data.favorites.insert(entry_id.to_string());
                } else {
                    data.favorites.remove(entry_id);
                }
                store_for_favorite.save(&data);
            }

            button.set_icon_name(if button.is_active() {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            });
            button.set_tooltip_text(Some(&if button.is_active() {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            }));

            let query = search_for_favorite.text().to_lowercase();
            rebuild_results(
                &list_for_favorite,
                &count_for_favorite,
                &empty_for_favorite,
                &card_for_favorite,
                &visible_for_favorite,
                &entries_for_favorite,
                &personal_for_favorite.borrow(),
                query.trim(),
                *category_for_favorite.borrow(),
            );
        });

        let notes_buffer = notes_text.buffer();
        let list_for_note = result_list.clone();
        let count_for_note = result_count.clone();
        let empty_for_note = empty.clone();
        let card_for_note = results_card.clone();
        let visible_for_note = visible_entries.clone();
        let entries_for_note = entries.clone();
        let search_for_note = search.clone();
        let category_for_note = selected_category.clone();
        let current_for_note = current_entry_id.clone();
        let personal_for_note = personal.clone();
        let store_for_note = personal_store.clone();
        let loading_for_note = loading_note.clone();
        let notes_filter_for_note = notes_filter_button.clone();
        let all_filter_for_note = all_filter_button.clone();
        notes_buffer.connect_changed(move |buffer| {
            if loading_for_note.get() {
                return;
            }
            let Some(entry_id) = *current_for_note.borrow() else {
                return;
            };

            let note = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            {
                let mut data = personal_for_note.borrow_mut();
                if note.trim().is_empty() {
                    data.notes.remove(entry_id);
                } else {
                    data.notes.insert(entry_id.to_string(), note);
                }
                store_for_note.save(&data);
            }

            let has_notes = personal_for_note
                .borrow()
                .notes
                .values()
                .any(|note| !note.trim().is_empty());
            notes_filter_for_note.set_visible(has_notes);
            if !has_notes && *category_for_note.borrow() == GuideCategory::Notes {
                all_filter_for_note.set_active(true);
                return;
            }

            let query = search_for_note.text().to_lowercase();
            rebuild_results(
                &list_for_note,
                &count_for_note,
                &empty_for_note,
                &card_for_note,
                &visible_for_note,
                &entries_for_note,
                &personal_for_note.borrow(),
                query.trim(),
                *category_for_note.borrow(),
            );
        });

        let stack_for_row = guide_stack.clone();
        let entries_for_row = entries.clone();
        let visible_for_row = visible_entries.clone();
        let category_for_row = detail_category.clone();
        let title_for_row = detail_title.clone();
        let summary_for_row = detail_summary.clone();
        let in_git_desk_for_row = in_git_desk.clone();
        let terms_for_row = terms.clone();
        let related_for_row = related.clone();
        let outline_title_for_row = overview_outline_label.clone();
        let outline_list_for_row = outline_list.clone();
        let detail_scroller_for_row = detail_scroller.clone();
        let outline_mode_for_row = outline_mode.clone();
        let favorite_for_row = favorite_button.clone();
        let notes_buffer_for_row = notes_text.buffer();
        let current_for_row = current_entry_id.clone();
        let personal_for_row = personal.clone();
        let loading_favorite_for_row = loading_favorite.clone();
        let loading_note_for_row = loading_note.clone();
        result_list.connect_row_activated(move |_, row| {
            let Some(entry_index) = visible_for_row
                .borrow()
                .get(row.index().max(0) as usize)
                .copied()
            else {
                return;
            };
            let Some(entry) = entries_for_row.get(entry_index) else {
                return;
            };

            *current_for_row.borrow_mut() = Some(entry.id);
            let data = personal_for_row.borrow();

            loading_favorite_for_row.set(true);
            let is_favorite = data.favorites.contains(entry.id);
            favorite_for_row.set_active(is_favorite);
            favorite_for_row.set_icon_name(if is_favorite {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            });
            favorite_for_row.set_tooltip_text(Some(&if is_favorite {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            }));
            loading_favorite_for_row.set(false);

            loading_note_for_row.set(true);
            notes_buffer_for_row
                .set_text(data.notes.get(entry.id).map(String::as_str).unwrap_or(""));
            loading_note_for_row.set(false);
            drop(data);

            outline_mode_for_row.set_active(true);
            category_for_row.set_label(&entry.category.label());
            title_for_row.set_label(&entry.title);
            summary_for_row.set_label(&entry.summary);
            in_git_desk_for_row.set_label(&entry.in_git_desk);
            terms_for_row.set_label(&entry.terms);
            related_for_row.set_label(&entry.related);
            outline_title_for_row.set_label(&entry.title);
            stack_for_row.set_visible_child_name("detail");

            let adjustment = detail_scroller_for_row.vadjustment();
            adjustment.set_value(adjustment.lower());
            outline_list_for_row.select_row(outline_list_for_row.row_at_index(0).as_ref());
        });

        let stack_for_back = guide_stack.clone();
        back.connect_clicked(move |_| stack_for_back.set_visible_child_name("overview"));

        let search_for_keys = search.clone();
        let stack_for_keys = guide_stack.clone();
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, state| {
            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) && key == gtk::gdk::Key::f {
                stack_for_keys.set_visible_child_name("overview");
                search_for_keys.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        root.add_controller(keys);

        Self {
            root,
            sidebar: sidebar_box,
            stack: guide_stack,
            sidebar_toggle: outline_toggle,
            outline_list,
        }
    }
}

fn install_git_guide_style() {
    use std::sync::Once;

    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
.cbapl-segmented-strip .cbapl-segment {
    border-radius: 0;
}

.git-guide-results-container {
    border-radius: 12px;
}

.git-guide-results-list,
.git-guide-results-list > row {
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

fn guide_section(title: &str) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let label = gtk::Label::builder().label(title).xalign(0.0).build();
    label.add_css_class("title-4");
    section.append(&label);
    section
}

fn section_body() -> gtk::Label {
    gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build()
}

#[allow(clippy::too_many_arguments)]
fn rebuild_results(
    list: &gtk::ListBox,
    result_count: &gtk::Label,
    empty: &gtk::Label,
    card: &gtk::Box,
    visible_entries: &Rc<RefCell<Vec<usize>>>,
    entries: &[GuideEntry],
    personal: &GuidePersonalData,
    query: &str,
    category: GuideCategory,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    visible_entries.borrow_mut().clear();

    for (index, entry) in entries.iter().enumerate() {
        if !entry.matches(query, category, personal) {
            continue;
        }

        visible_entries.borrow_mut().push(index);
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);

        let content = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(3)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(12)
            .margin_end(12)
            .build();
        let title_row = gtk::Box::new(Orientation::Horizontal, 8);
        let title = gtk::Label::builder()
            .label(&entry.title)
            .xalign(0.0)
            .hexpand(true)
            .build();
        title.add_css_class("heading");
        title_row.append(&title);

        let indicators = gtk::Box::new(Orientation::Horizontal, 6);
        indicators.set_halign(gtk::Align::End);
        if personal.favorites.contains(entry.id) {
            let favorite = gtk::Image::from_icon_name("starred-symbolic");
            favorite.add_css_class("dim-label");
            favorite.set_tooltip_text(Some(&tr("Favorite")));
            indicators.append(&favorite);
        }
        if personal
            .notes
            .get(entry.id)
            .is_some_and(|note| !note.trim().is_empty())
        {
            let note = gtk::Image::from_icon_name("document-edit-symbolic");
            note.add_css_class("dim-label");
            note.set_tooltip_text(Some(&tr("Has personal note")));
            indicators.append(&note);
        }
        title_row.append(&indicators);

        let summary = gtk::Label::builder()
            .label(&entry.summary)
            .xalign(0.0)
            .wrap(true)
            .build();
        let meta = gtk::Label::builder()
            .label(format!("{} · {}", entry.category.label(), entry.terms))
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        meta.add_css_class("dim-label");
        content.append(&title_row);
        content.append(&summary);
        content.append(&meta);
        row.set_child(Some(&content));
        list.append(&row);
    }

    let count = visible_entries.borrow().len();
    result_count.set_label(&ntr_args(
        "{count} topic",
        "{count} topics",
        count as u64,
        &[("count", count.to_string())],
    ));
    if count == 0 {
        empty.set_label(&match category {
            GuideCategory::Favorites => {
                tr("No favorite topics yet. Open a topic and use the star button to add it here.")
            }
            GuideCategory::Notes => tr("No topics with notes match this search."),
            _ => tr("No matching topics. Try another term or choose All Topics."),
        });
    }
    empty.set_visible(count == 0);
    card.set_visible(count > 0);
}

fn guide_entries() -> Vec<GuideEntry> {
    vec![
        GuideEntry {
            id: "repository",
            category: GuideCategory::GettingStarted,
            title: tr("Repository"),
            summary: tr("A project folder whose history is tracked by Git."),
            in_git_desk: tr(
                "Open a repository to see its changes, history, branches, stashes, and tags in one workspace.",
            ),
            terms: tr("repository, repo, .git"),
            related: tr("Clone Repository, Working Tree, Commit"),
            keywords: "project folder git repo initialize init",
        },
        GuideEntry {
            id: "clone-repository",
            category: GuideCategory::GettingStarted,
            title: tr("Clone Repository"),
            summary: tr("Create a local copy of an existing remote Git repository."),
            in_git_desk: tr(
                "Use Clone Repository, enter the repository URL, then choose where the new local folder should be created.",
            ),
            terms: tr("clone, remote URL, git clone"),
            related: tr("Repository, Remote, Origin"),
            keywords: "download copy github gitlab codeberg url",
        },
        GuideEntry {
            id: "open-repository",
            category: GuideCategory::GettingStarted,
            title: tr("Open Repository"),
            summary: tr("Open an existing local Git project without changing its history."),
            in_git_desk: tr(
                "Choose Open Project and select the project folder that contains the repository.",
            ),
            terms: tr("working copy, local repository"),
            related: tr("Repository, Working Tree"),
            keywords: "folder existing project local open",
        },
        GuideEntry {
            id: "working-tree",
            category: GuideCategory::GettingStarted,
            title: tr("Working Tree"),
            summary: tr("The files you are currently editing in the checked-out repository."),
            in_git_desk: tr(
                "Changes compares your working tree with the saved Git state and separates unstaged and staged work.",
            ),
            terms: tr("working tree, worktree, working directory"),
            related: tr("Modified File, Untracked File, Stage"),
            keywords: "files edits current checkout directory status",
        },
        GuideEntry {
            id: "modified-file",
            category: GuideCategory::Changes,
            title: tr("Modified File"),
            summary: tr("A tracked file whose contents differ from the last saved Git state."),
            in_git_desk: tr(
                "Modified files appear in Changes. Inspect the diff, then stage the changes you want in your next commit.",
            ),
            terms: tr("modified, tracked file, diff"),
            related: tr("Stage, Discard Changes, Commit"),
            keywords: "edited changed tracked diff status",
        },
        GuideEntry {
            id: "untracked-file",
            category: GuideCategory::Changes,
            title: tr("Untracked File"),
            summary: tr("A file in the project that Git is not tracking yet."),
            in_git_desk: tr(
                "Stage an untracked file if it belongs in the repository, or delete it if it should not be kept.",
            ),
            terms: tr("untracked, new file"),
            related: tr("Stage, Commit"),
            keywords: "new file unknown add git add",
        },
        GuideEntry {
            id: "stage",
            category: GuideCategory::Changes,
            title: tr("Stage"),
            summary: tr("Choose changes that will be included in your next commit."),
            in_git_desk: tr(
                "Stage a file from Changes. Git Desk moves it to the ready-to-commit section.",
            ),
            terms: tr("stage, staging area, index, git add"),
            related: tr("Unstage, Commit, Stage All"),
            keywords: "prepare select include next commit index add",
        },
        GuideEntry {
            id: "unstage",
            category: GuideCategory::Changes,
            title: tr("Unstage"),
            summary: tr("Remove a change from the next commit without discarding the file edit."),
            in_git_desk: tr(
                "Unstage a ready-to-commit file to move it back to the unstaged changes list.",
            ),
            terms: tr("unstage, index, restore --staged"),
            related: tr("Stage, Commit, Discard Changes"),
            keywords: "remove from commit keep edit staged",
        },
        GuideEntry {
            id: "stage-all-unstage-all",
            category: GuideCategory::Changes,
            title: tr("Stage All / Unstage All"),
            summary: tr("Move all current changes into or out of the next commit selection."),
            in_git_desk: tr(
                "Use Stage All when everything belongs in one commit. Use Unstage All when you want to rebuild the selection.",
            ),
            terms: tr("stage all, unstage all, index"),
            related: tr("Stage, Unstage, Commit"),
            keywords: "all files bulk prepare reset selection",
        },
        GuideEntry {
            id: "discard-changes",
            category: GuideCategory::Changes,
            title: tr("Discard Changes"),
            summary: tr("Permanently throw away local edits that have not been committed."),
            in_git_desk: tr(
                "Use Discard Changes only when you are sure the local edits are no longer needed. Git Desk asks for confirmation first.",
            ),
            terms: tr("discard, restore, delete untracked file"),
            related: tr("Modified File, Untracked File, Stash"),
            keywords: "undo edits revert file delete restore destructive",
        },
        GuideEntry {
            id: "commit",
            category: GuideCategory::Commits,
            title: tr("Commit"),
            summary: tr(
                "Save the currently staged changes as a new point in the repository history.",
            ),
            in_git_desk: tr(
                "Stage the intended changes, write a clear commit message, then press Commit.",
            ),
            terms: tr("commit, snapshot, commit ID, SHA"),
            related: tr("Stage, Commit Message, History"),
            keywords: "save version snapshot sha hash",
        },
        GuideEntry {
            id: "commit-message",
            category: GuideCategory::Commits,
            title: tr("Commit Message"),
            summary: tr("A short explanation of why a set of changes was saved."),
            in_git_desk: tr(
                "Write the message beside the Commit button. Keep the first line short and describe the purpose of the change.",
            ),
            terms: tr("subject, commit message"),
            related: tr("Commit, Amend"),
            keywords: "description subject title save",
        },
        GuideEntry {
            id: "amend",
            category: GuideCategory::Commits,
            title: tr("Amend"),
            summary: tr("Replace your latest unpublished commit with an updated version."),
            in_git_desk: tr(
                "Select the latest unpublished commit in History and use Amend to add currently staged changes to it.",
            ),
            terms: tr("amend, rewrite latest commit"),
            related: tr("Commit, Stage, Undo Commit"),
            keywords: "edit last commit add staged rewrite unpublished",
        },
        GuideEntry {
            id: "undo-commit",
            category: GuideCategory::Commits,
            title: tr("Undo Commit"),
            summary: tr(
                "Remove the latest unpublished commit while returning its contents to staging.",
            ),
            in_git_desk: tr(
                "Use Undo Commit from the Inspector when the latest unpublished commit should be rebuilt rather than kept.",
            ),
            terms: tr("undo commit, reset, staged changes"),
            related: tr("Amend, Commit, Stage"),
            keywords: "remove last commit keep changes soft reset unpublished",
        },
        GuideEntry {
            id: "branch",
            category: GuideCategory::Branches,
            title: tr("Branch"),
            summary: tr("A movable line of development that points to a sequence of commits."),
            in_git_desk: tr(
                "Branches shows local branches, configured remotes, and remote-tracking branches together.",
            ),
            terms: tr("branch, ref"),
            related: tr("Current Branch, Switch Branch, Merge"),
            keywords: "line development ref feature main master",
        },
        GuideEntry {
            id: "current-branch",
            category: GuideCategory::Branches,
            title: tr("Current Branch"),
            summary: tr("The branch that receives new commits from your current working tree."),
            in_git_desk: tr(
                "Git Desk marks the current branch and shows its upstream, ahead, and behind state when available.",
            ),
            terms: tr("current branch, checked out branch, HEAD"),
            related: tr("HEAD, Upstream Branch, Ahead / Behind"),
            keywords: "active checkout checked out head",
        },
        GuideEntry {
            id: "switch-branch",
            category: GuideCategory::Branches,
            title: tr("Switch Branch"),
            summary: tr("Change which branch is checked out in the working tree."),
            in_git_desk: tr(
                "Select another local branch in Branches and switch to it. Commit or stash conflicting local work first when necessary.",
            ),
            terms: tr("switch, checkout"),
            related: tr("Branch, Stash, Current Branch"),
            keywords: "checkout move branch change current",
        },
        GuideEntry {
            id: "new-branch",
            category: GuideCategory::Branches,
            title: tr("New Branch"),
            summary: tr(
                "Create a new branch so work can develop separately from the current line.",
            ),
            in_git_desk: tr(
                "Use New Branch in Branches. The new branch starts from the currently selected Git position.",
            ),
            terms: tr("create branch, branch name"),
            related: tr("Branch, Switch Branch, Merge"),
            keywords: "feature branch create start line",
        },
        GuideEntry {
            id: "local-vs-remote-branch",
            category: GuideCategory::Branches,
            title: tr("Local vs Remote Branch"),
            summary: tr(
                "A local branch is yours to edit; a remote-tracking branch records a branch last seen on a remote.",
            ),
            in_git_desk: tr(
                "Branches separates Local Branches from Remote Branches so you can see which state is local and which came from a remote.",
            ),
            terms: tr("local branch, remote-tracking branch, refs/remotes"),
            related: tr("Remote, Fetch, Upstream Branch"),
            keywords: "origin branch tracking remote local",
        },
        GuideEntry {
            id: "merge",
            category: GuideCategory::Branches,
            title: tr("Merge"),
            summary: tr("Combine another branch into the current branch."),
            in_git_desk: tr(
                "Merge a branch into the current branch from Branches. Git Desk fast-forwards when possible and keeps conflicts open for recovery.",
            ),
            terms: tr("merge, merge commit, fast-forward"),
            related: tr("Fast-forward, Conflict, Abort Operation"),
            keywords: "combine branches integrate merge commit ff",
        },
        GuideEntry {
            id: "remote",
            category: GuideCategory::Sync,
            title: tr("Remote"),
            summary: tr(
                "A named connection to another copy of the repository, usually on a server.",
            ),
            in_git_desk: tr(
                "Branches lists configured remotes and lets you add a remote repository URL.",
            ),
            terms: tr("remote, remote URL"),
            related: tr("Origin, Fetch, Push"),
            keywords: "server github gitlab codeberg url repository",
        },
        GuideEntry {
            id: "origin",
            category: GuideCategory::Sync,
            title: tr("Origin"),
            summary: tr(
                "The conventional name Git gives to the remote a repository was cloned from.",
            ),
            in_git_desk: tr(
                "Origin is shown like any other remote. The name is conventional, not mandatory.",
            ),
            terms: tr("origin, remote name"),
            related: tr("Remote, Clone Repository, Upstream Branch"),
            keywords: "default remote clone server",
        },
        GuideEntry {
            id: "upstream-branch",
            category: GuideCategory::Sync,
            title: tr("Upstream Branch"),
            summary: tr(
                "The remote-tracking branch your local branch follows for pull and push status.",
            ),
            in_git_desk: tr(
                "Git Desk shows the upstream beside local branches and uses it to calculate ahead and behind counts.",
            ),
            terms: tr("upstream, tracking branch"),
            related: tr("Ahead / Behind, Pull, Push"),
            keywords: "track tracking origin main branch remote",
        },
        GuideEntry {
            id: "fetch",
            category: GuideCategory::Sync,
            title: tr("Fetch"),
            summary: tr(
                "Download updated remote branch and tag information without changing your working files.",
            ),
            in_git_desk: tr(
                "Use Fetch when you want the latest remote state before deciding whether to pull, merge, or inspect differences.",
            ),
            terms: tr("fetch, git fetch, remote-tracking branch"),
            related: tr("Pull, Remote, Ahead / Behind"),
            keywords: "download updates refresh remote no merge",
        },
        GuideEntry {
            id: "pull",
            category: GuideCategory::Sync,
            title: tr("Pull"),
            summary: tr("Bring upstream commits into the current local branch."),
            in_git_desk: tr(
                "Git Desk uses fast-forward-only Pull. If local and remote history diverged, fetch first and merge the appropriate branch explicitly.",
            ),
            terms: tr("pull, git pull --ff-only, fast-forward"),
            related: tr("Fetch, Fast-forward, Diverged"),
            keywords: "download integrate upstream ff-only update branch",
        },
        GuideEntry {
            id: "push",
            category: GuideCategory::Sync,
            title: tr("Push"),
            summary: tr("Send local commits to the branch's configured remote."),
            in_git_desk: tr(
                "Use Push when your branch has commits that the upstream does not have yet.",
            ),
            terms: tr("push, git push"),
            related: tr("Upstream Branch, Ahead / Behind, Remote"),
            keywords: "upload publish commits remote server",
        },
        GuideEntry {
            id: "ahead-behind",
            category: GuideCategory::Sync,
            title: tr("Ahead / Behind"),
            summary: tr("Counts showing how local and upstream branch histories differ."),
            in_git_desk: tr(
                "Ahead means local commits are waiting to be pushed. Behind means the upstream contains commits you do not have locally.",
            ),
            terms: tr("ahead, behind, upstream"),
            related: tr("Fetch, Pull, Push"),
            keywords: "sync counts remote local status",
        },
        GuideEntry {
            id: "diverged",
            category: GuideCategory::Sync,
            title: tr("Diverged"),
            summary: tr(
                "Both local and upstream branches contain commits the other side does not have.",
            ),
            in_git_desk: tr(
                "Fetch to refresh the remote state, then review the branches and merge deliberately. Git Desk does not hide this behind a non-fast-forward Pull.",
            ),
            terms: tr("diverged, non-fast-forward"),
            related: tr("Fetch, Merge, Pull"),
            keywords: "both ahead behind histories split non fast forward",
        },
        GuideEntry {
            id: "fast-forward",
            category: GuideCategory::Sync,
            title: tr("Fast-forward"),
            summary: tr(
                "Move a branch pointer forward when no competing history needs to be combined.",
            ),
            in_git_desk: tr(
                "Pull requires a fast-forward. Merge also fast-forwards automatically when the current branch has no competing commits.",
            ),
            terms: tr("fast-forward, ff, ff-only"),
            related: tr("Pull, Merge, Diverged"),
            keywords: "linear history branch pointer ff-only",
        },
        GuideEntry {
            id: "stash",
            category: GuideCategory::StashesTags,
            title: tr("Stash"),
            summary: tr(
                "Temporarily save unfinished working-tree changes without making a commit.",
            ),
            in_git_desk: tr(
                "Use Stash Changes when you need a clean working tree before switching branches or starting another Git operation.",
            ),
            terms: tr("stash, stash entry"),
            related: tr("Apply Stash, Pop Stash, Working Tree"),
            keywords: "temporary save work in progress wip clean tree",
        },
        GuideEntry {
            id: "apply-stash",
            category: GuideCategory::StashesTags,
            title: tr("Apply Stash"),
            summary: tr("Restore a stash to the working tree while keeping the stash entry saved."),
            in_git_desk: tr(
                "Select a stash and use Apply when you may want to reuse or keep that saved snapshot afterward.",
            ),
            terms: tr("stash apply"),
            related: tr("Stash, Pop Stash"),
            keywords: "restore saved work keep stash",
        },
        GuideEntry {
            id: "pop-stash",
            category: GuideCategory::StashesTags,
            title: tr("Pop Stash"),
            summary: tr("Restore a stash and remove the stash entry when the operation succeeds."),
            in_git_desk: tr(
                "Use Pop when you are ready to resume the stashed work and no longer need the saved stash entry.",
            ),
            terms: tr("stash pop"),
            related: tr("Stash, Apply Stash"),
            keywords: "restore saved work remove stash",
        },
        GuideEntry {
            id: "tag",
            category: GuideCategory::StashesTags,
            title: tr("Tag"),
            summary: tr(
                "A stable name attached to a specific commit, often used for releases and milestones.",
            ),
            in_git_desk: tr(
                "Tags lets you create a tag at the current commit, inspect tags, push a tag, or delete a local tag.",
            ),
            terms: tr("tag, ref, release tag"),
            related: tr("Commit, History, Push"),
            keywords: "version release milestone label reference",
        },
        GuideEntry {
            id: "history",
            category: GuideCategory::History,
            title: tr("History"),
            summary: tr("The ordered record of commits that make up the repository."),
            in_git_desk: tr(
                "History shows the most recent commits with graph lanes, references, messages, and commit details in the Inspector.",
            ),
            terms: tr("history, log, commit graph"),
            related: tr("Commit, HEAD, Revert"),
            keywords: "log graph timeline commits past",
        },
        GuideEntry {
            id: "head",
            category: GuideCategory::History,
            title: tr("HEAD"),
            summary: tr("Git's reference to the commit or branch currently checked out."),
            in_git_desk: tr(
                "Normally HEAD follows the current branch. History decorations help show where HEAD points.",
            ),
            terms: tr("HEAD, ref, current commit"),
            related: tr("Current Branch, Detached HEAD, History"),
            keywords: "current pointer checkout commit branch",
        },
        GuideEntry {
            id: "detached-head",
            category: GuideCategory::History,
            title: tr("Detached HEAD"),
            summary: tr("HEAD points directly to a commit instead of following a local branch."),
            in_git_desk: tr(
                "Git Desk identifies Detached HEAD clearly. Create or switch to a branch before making work you intend to keep on a named branch.",
            ),
            terms: tr("detached HEAD, checkout commit"),
            related: tr("HEAD, New Branch, Switch Branch"),
            keywords: "detached no branch commit checkout recover",
        },
        GuideEntry {
            id: "revert",
            category: GuideCategory::History,
            title: tr("Revert"),
            summary: tr(
                "Create a new commit that reverses the effect of an older commit without rewriting existing history.",
            ),
            in_git_desk: tr(
                "Select a commit in History and choose Revert Commit. If conflicts occur, resolve them in Changes and continue or abort the Revert.",
            ),
            terms: tr("revert, inverse commit"),
            related: tr("Conflict, Continue Operation, Abort Operation"),
            keywords: "undo old commit safe history reverse",
        },
        GuideEntry {
            id: "cherry-pick",
            category: GuideCategory::History,
            title: tr("Cherry-pick"),
            summary: tr(
                "Apply the changes introduced by one existing commit on top of the current branch as a new commit.",
            ),
            in_git_desk: tr(
                "Select a commit in History and choose Cherry-pick. Resolve conflicts in Changes, then continue, skip an empty Cherry-pick, or abort.",
            ),
            terms: tr("cherry-pick, apply commit"),
            related: tr("Conflict, Skip Cherry-pick, Abort Operation"),
            keywords: "copy commit another branch apply selected change",
        },
        GuideEntry {
            id: "conflict",
            category: GuideCategory::Recovery,
            title: tr("Conflict"),
            summary: tr(
                "Git needs your decision because competing changes cannot be combined automatically.",
            ),
            in_git_desk: tr(
                "Conflicted files appear in Changes during Merge, Revert, or Cherry-pick recovery. Open each file, resolve the competing content, then mark it resolved.",
            ),
            terms: tr("conflict, unmerged path, ours, theirs"),
            related: tr("Conflict Markers, Mark Resolved, Abort Operation"),
            keywords: "merge conflict resolve unmerged both changed",
        },
        GuideEntry {
            id: "conflict-markers",
            category: GuideCategory::Recovery,
            title: tr("Conflict Markers"),
            summary: tr(
                "Text markers Git writes into a conflicted file to show competing versions.",
            ),
            in_git_desk: tr(
                "The Inspector shows the conflicted file contents. Edit the file so the final content is correct and remove all conflict markers before marking it resolved.",
            ),
            terms: tr("<<<<<<<, =======, >>>>>>>"),
            related: tr("Conflict, Mark Resolved"),
            keywords: "markers ours theirs merge edit file resolve",
        },
        GuideEntry {
            id: "mark-resolved",
            category: GuideCategory::Recovery,
            title: tr("Mark Resolved"),
            summary: tr("Tell Git that you have finished resolving a conflicted file."),
            in_git_desk: tr(
                "After editing a conflicted file to its final form, choose Mark Resolved. Git Desk stages that resolution and rechecks the operation state.",
            ),
            terms: tr("mark resolved, stage resolution, git add"),
            related: tr("Conflict, Continue Operation"),
            keywords: "resolved stage conflict finished git add",
        },
        GuideEntry {
            id: "continue-operation",
            category: GuideCategory::Recovery,
            title: tr("Continue Operation"),
            summary: tr(
                "Finish an interrupted Git operation after all required conflict resolutions are staged.",
            ),
            in_git_desk: tr(
                "Continue becomes available when Git Desk verifies that the operation has no unresolved conflicts and the required resolution is ready.",
            ),
            terms: tr("continue, merge continue, revert continue, cherry-pick continue"),
            related: tr("Mark Resolved, Abort Operation, Skip Cherry-pick"),
            keywords: "finish resume operation recovery conflict staged",
        },
        GuideEntry {
            id: "abort-operation",
            category: GuideCategory::Recovery,
            title: tr("Abort Operation"),
            summary: tr(
                "Cancel the current Merge, Revert, or Cherry-pick and restore its pre-operation state.",
            ),
            in_git_desk: tr(
                "Use the operation's Abort action when you do not want to finish the in-progress history change. Git Desk asks for confirmation first.",
            ),
            terms: tr("abort, merge --abort, revert --abort, cherry-pick --abort"),
            related: tr("Conflict, Continue Operation"),
            keywords: "cancel stop recovery restore previous state",
        },
        GuideEntry {
            id: "skip-cherry-pick",
            category: GuideCategory::Recovery,
            title: tr("Skip Cherry-pick"),
            summary: tr(
                "Finish an empty Cherry-pick when there are no tracked changes left to commit.",
            ),
            in_git_desk: tr(
                "Git Desk offers Skip Cherry-pick only when the active Cherry-pick is empty and no unresolved conflicts remain.",
            ),
            terms: tr("cherry-pick --skip, empty cherry-pick"),
            related: tr("Cherry-pick, Continue Operation, Abort Operation"),
            keywords: "skip empty no changes conflict recovery",
        },
    ]
}
