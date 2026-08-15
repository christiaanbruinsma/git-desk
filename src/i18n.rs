use gettextrs::{
    LocaleCategory, bind_textdomain_codeset, bindtextdomain, gettext, ngettext, setlocale,
    textdomain,
};

pub const GETTEXT_PACKAGE: &str = "git-desk";

pub fn locale_dir() -> &'static str {
    option_env!("LOCALEDIR").unwrap_or("/usr/share/locale")
}

pub fn init() {
    let _ = setlocale(LocaleCategory::LcAll, "");
    let _ = bindtextdomain(GETTEXT_PACKAGE, locale_dir());
    let _ = bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8");
    let _ = textdomain(GETTEXT_PACKAGE);
}

pub fn tr(message: &str) -> String {
    gettext(message)
}

pub fn tr_args(message: &str, args: &[(&str, String)]) -> String {
    let mut translated = gettext(message);
    for (key, value) in args {
        translated = translated.replace(&format!("{{{key}}}"), value);
    }
    translated
}

pub fn ntr_args(singular: &str, plural: &str, n: u64, args: &[(&str, String)]) -> String {
    let plural_count = u32::try_from(n).unwrap_or(u32::MAX);
    let mut translated = ngettext(singular, plural, plural_count);
    for (key, value) in args {
        translated = translated.replace(&format!("{{{key}}}"), value);
    }
    translated
}
