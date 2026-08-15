use adw::prelude::*;
use gtk::{Align, Justification, Orientation};

use crate::i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlertEyebrow {
    Notice,
    Confirmation,
    Warning,
    Error,
    Danger,
}

impl AlertEyebrow {
    fn label(self) -> String {
        match self {
            Self::Notice => tr("Notice"),
            Self::Confirmation => tr("Confirmation"),
            Self::Warning => tr("Warning"),
            Self::Error => tr("Error"),
            Self::Danger => tr("Danger"),
        }
    }

    fn css_class(self) -> Option<&'static str> {
        match self {
            Self::Notice => None,
            Self::Confirmation => Some("success"),
            Self::Warning => Some("warning"),
            Self::Error | Self::Danger => Some("error"),
        }
    }
}

pub(crate) fn apply_alert_eyebrow(dialog: &adw::AlertDialog, eyebrow: AlertEyebrow) {
    // Git Desk deliberately uses a semantic eyebrow-first alert hierarchy:
    // EYEBROW -> TITLE -> BODY -> OPTIONAL CONTENT -> ACTIONS.
    // Keep native AdwAlertDialog responses/behavior while moving the message
    // hierarchy into the extra child. Existing form/content children are
    // preserved below the body.
    let heading = dialog.heading().unwrap_or_default();
    let body = dialog.body();
    let existing_extra = dialog.extra_child();

    // Use an explicit empty heading instead of NULL. On the current
    // libadwaita runtime, NULL leaves the built-in heading visually present,
    // which duplicates our custom eyebrow-first title.
    dialog.set_heading(Some(""));
    dialog.set_body("");
    dialog.set_extra_child(None::<&gtk::Widget>);

    let content = gtk::Box::new(Orientation::Vertical, 6);
    content.set_halign(Align::Fill);

    let eyebrow_label = eyebrow.label().to_uppercase();
    let eyebrow_widget = gtk::Label::builder()
        .halign(Align::Center)
        .xalign(0.5)
        .build();
    eyebrow_widget.add_css_class("caption-heading");

    if eyebrow == AlertEyebrow::Notice {
        let escaped = gtk::glib::markup_escape_text(&eyebrow_label);
        eyebrow_widget.set_markup(&format!("<span foreground=\"#3584e4\">{escaped}</span>"));
    } else {
        eyebrow_widget.set_label(&eyebrow_label);
        if let Some(css_class) = eyebrow.css_class() {
            eyebrow_widget.add_css_class(css_class);
        }
    }
    content.append(&eyebrow_widget);

    if !heading.is_empty() {
        let title = gtk::Label::builder()
            .label(heading)
            .halign(Align::Center)
            .xalign(0.5)
            .justify(Justification::Center)
            .wrap(true)
            .build();
        title.add_css_class("title-2");
        content.append(&title);
    }

    if !body.is_empty() {
        let body_label = gtk::Label::builder()
            .label(body)
            .halign(Align::Center)
            .xalign(0.5)
            .justify(Justification::Center)
            .wrap(true)
            .margin_top(6)
            .build();
        content.append(&body_label);
    }

    if let Some(existing_extra) = existing_extra {
        existing_extra.set_margin_top(existing_extra.margin_top().max(6));
        content.append(&existing_extra);
    }

    dialog.set_extra_child(Some(&content));
}
