use gtk::prelude::*;
use gtk::{TextBuffer, TextTag, TextTagTable, TextView};

use crate::git::{models::DiffLineKind, parser::parse_unified_diff};
use crate::i18n::tr;

#[derive(Clone)]
pub struct DiffView {
    pub widget: gtk::Frame,
    stack: gtk::Stack,
    placeholder: gtk::Label,
    buffer: TextBuffer,
    gutter_tag: TextTag,
    added_tag: TextTag,
    removed_tag: TextTag,
    hunk_tag: TextTag,
    metadata_tag: TextTag,
}

impl DiffView {
    pub fn new() -> Self {
        install_diff_view_style();

        let table = TextTagTable::new();

        let gutter_tag = TextTag::builder().name("gutter").build();
        let added_tag = TextTag::builder().name("added").weight(600).build();
        let removed_tag = TextTag::builder().name("removed").weight(600).build();
        let hunk_tag = TextTag::builder().name("hunk").weight(600).build();
        let metadata_tag = TextTag::builder().name("metadata").build();

        table.add(&gutter_tag);
        table.add(&added_tag);
        table.add(&removed_tag);
        table.add(&hunk_tag);
        table.add(&metadata_tag);

        let buffer = TextBuffer::new(Some(&table));
        let text_view = TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::None)
            .hexpand(true)
            .left_margin(8)
            .right_margin(8)
            .top_margin(12)
            .bottom_margin(12)
            .build();

        apply_theme_colors(
            &text_view,
            &gutter_tag,
            &added_tag,
            &removed_tag,
            &hunk_tag,
            &metadata_tag,
        );

        let gutter_tag_mapped = gutter_tag.clone();
        let added_tag_mapped = added_tag.clone();
        let removed_tag_mapped = removed_tag.clone();
        let hunk_tag_mapped = hunk_tag.clone();
        let metadata_tag_mapped = metadata_tag.clone();
        text_view.connect_map(move |widget| {
            apply_theme_colors(
                widget,
                &gutter_tag_mapped,
                &added_tag_mapped,
                &removed_tag_mapped,
                &hunk_tag_mapped,
                &metadata_tag_mapped,
            );
        });

        let scroller = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&text_view)
            .build();

        let placeholder = gtk::Label::builder()
            .xalign(0.5)
            .yalign(0.5)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        placeholder.add_css_class("dim-label");

        let stack = gtk::Stack::builder().hexpand(true).vexpand(true).build();
        stack.add_named(&scroller, Some("diff"));
        stack.add_named(&placeholder, Some("placeholder"));
        stack.set_visible_child_name("diff");

        let widget = gtk::Frame::new(None);
        widget.add_css_class("diff-surface");
        widget.set_hexpand(true);
        widget.set_vexpand(true);
        widget.set_overflow(gtk::Overflow::Hidden);
        widget.set_child(Some(&stack));

        Self {
            widget,
            stack,
            placeholder,
            buffer,
            gutter_tag,
            added_tag,
            removed_tag,
            hunk_tag,
            metadata_tag,
        }
    }

    pub fn set_placeholder(&self, text: &str) {
        self.buffer.set_text("");
        self.placeholder.set_label(text);
        self.stack.set_visible_child_name("placeholder");
    }

    pub fn set_plain_text(&self, text: &str) {
        self.buffer.set_text(text);
        self.stack.set_visible_child_name("diff");
    }

    pub fn set_patch(&self, patch: &str) {
        self.buffer.set_text("");
        self.stack.set_visible_child_name("diff");

        let lines = parse_unified_diff(patch);
        if lines.is_empty() {
            self.set_placeholder(&tr("No textual diff available."));
            return;
        }

        let old_width = line_number_width(lines.iter().filter_map(|line| line.old_line));
        let new_width = line_number_width(lines.iter().filter_map(|line| line.new_line));

        for line in lines {
            match line.kind {
                DiffLineKind::Hunk => {
                    self.insert_structural_line(&line.text, old_width, new_width, &self.hunk_tag);
                    continue;
                }
                DiffLineKind::Metadata => {
                    self.insert_structural_line(
                        &line.text,
                        old_width,
                        new_width,
                        &self.metadata_tag,
                    );
                    continue;
                }
                DiffLineKind::Added | DiffLineKind::Removed | DiffLineKind::Context => {}
            }

            let old = format_line_number(line.old_line, old_width);
            let new = format_line_number(line.new_line, new_width);
            let gutter = format!("{old} {new} │ ");

            let (marker, tag) = match line.kind {
                DiffLineKind::Added => ("+", Some(&self.added_tag)),
                DiffLineKind::Removed => ("-", Some(&self.removed_tag)),
                DiffLineKind::Context => (" ", None),
                DiffLineKind::Hunk | DiffLineKind::Metadata => unreachable!(),
            };

            let mut end = self.buffer.end_iter();
            self.buffer
                .insert_with_tags(&mut end, &gutter, &[&self.gutter_tag]);

            let content = format!("{marker} {}\n", line.text);
            if let Some(tag) = tag {
                self.buffer.insert_with_tags(&mut end, &content, &[tag]);
            } else {
                self.buffer.insert(&mut end, &content);
            }
        }
    }

    fn insert_structural_line(
        &self,
        text: &str,
        old_width: usize,
        new_width: usize,
        tag: &TextTag,
    ) {
        let gutter = format!("{} │ ", " ".repeat(old_width + new_width + 1));
        let mut end = self.buffer.end_iter();
        self.buffer
            .insert_with_tags(&mut end, &gutter, &[&self.gutter_tag]);
        self.buffer
            .insert_with_tags(&mut end, &format!("  {text}\n"), &[tag]);
    }
}

fn install_diff_view_style() {
    use std::sync::Once;

    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
.diff-surface {
    background-color: @view_bg_color;
    border: 1px solid @borders;
    border-radius: 12px;
}

.diff-surface textview,
.diff-surface textview text {
    background-color: transparent;
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

fn line_number_width(values: impl Iterator<Item = u32>) -> usize {
    values
        .max()
        .map(|value| value.to_string().len())
        .unwrap_or(1)
}

fn format_line_number(value: Option<u32>, width: usize) -> String {
    value
        .map(|value| format!("{value:>width$}"))
        .unwrap_or_else(|| " ".repeat(width))
}

#[allow(deprecated)]
fn apply_theme_colors(
    widget: &TextView,
    gutter_tag: &TextTag,
    added_tag: &TextTag,
    removed_tag: &TextTag,
    hunk_tag: &TextTag,
    metadata_tag: &TextTag,
) {
    let style = widget.style_context();

    if let Some(color) = style.lookup_color("insensitive_fg_color") {
        gutter_tag.set_foreground_rgba(Some(&color));
        metadata_tag.set_foreground_rgba(Some(&color));
    }
    if let Some(color) = style.lookup_color("success_color") {
        added_tag.set_foreground_rgba(Some(&color));
    }
    if let Some(color) = style.lookup_color("error_color") {
        removed_tag.set_foreground_rgba(Some(&color));
    }
    if let Some(color) = style.lookup_color("accent_color") {
        hunk_tag.set_foreground_rgba(Some(&color));
    }
}
