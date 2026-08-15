use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
};

use glib::variant::ToVariant;
use gtk::{gio, glib};

const DOCUMENTS_BUS: &str = "org.freedesktop.portal.Documents";
const DOCUMENTS_PATH: &str = "/org/freedesktop/portal/documents";
const DOCUMENTS_INTERFACE: &str = "org.freedesktop.portal.Documents";

macro_rules! path_diag {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        eprintln!($($arg)*);
        #[cfg(not(debug_assertions))]
        let _ = format_args!($($arg)*);
    }};
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortalDocumentPath {
    doc_id: String,
    relative: PathBuf,
}

/// Resolve a Document Portal/FUSE path back to its stable host path when the
/// sandbox can access that host path directly. If resolution is unavailable
/// or the host path is not accessible, the original portal path is preserved.
pub async fn resolve_host_path(path: PathBuf) -> PathBuf {
    let Some(portal) = parse_portal_document_path(&path) else {
        return path;
    };

    let original = path.clone();
    let doc_id = portal.doc_id.clone();
    let relative = portal.relative.clone();

    path_diag!(
        "[Git Desk path] parsed portal={} doc_id={} relative={}",
        original.display(),
        doc_id,
        relative.display()
    );

    let resolved = gio::spawn_blocking(move || resolve_host_path_sync(&doc_id, &relative)).await;

    match resolved {
        Ok(Some(host_path)) if host_path.exists() => {
            path_diag!(
                "[Git Desk path] portal={} host={}",
                original.display(),
                host_path.display()
            );
            host_path
        }
        Ok(Some(host_path)) => {
            path_diag!(
                "[Git Desk path] host path is not directly accessible; keeping portal path portal={} host={}",
                original.display(),
                host_path.display()
            );
            original
        }
        Ok(None) => {
            path_diag!(
                "[Git Desk path] could not resolve portal path; keeping {}",
                original.display()
            );
            original
        }
        Err(error) => {
            let panic_message = if let Some(message) = error.downcast_ref::<&str>() {
                *message
            } else if let Some(message) = error.downcast_ref::<String>() {
                message.as_str()
            } else {
                "non-string panic payload"
            };
            path_diag!(
                "[Git Desk path] resolver worker failed path={} panic={panic_message}",
                original.display()
            );
            original
        }
    }
}

fn resolve_host_path_sync(doc_id: &str, relative: &Path) -> Option<PathBuf> {
    path_diag!(
        "[Git Desk path] GetHostPaths begin doc_id={} relative={}",
        doc_id,
        relative.display()
    );

    let connection = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(connection) => {
            path_diag!("[Git Desk path] session bus connected");
            connection
        }
        Err(error) => {
            path_diag!("[Git Desk path] session bus connection failed: {error}");
            return None;
        }
    };

    let parameters = (vec![doc_id.to_string()],).to_variant();
    path_diag!("[Git Desk path] GetHostPaths parameters={parameters:?}");

    let reply = match connection.call_sync(
        Some(DOCUMENTS_BUS),
        DOCUMENTS_PATH,
        DOCUMENTS_INTERFACE,
        "GetHostPaths",
        Some(&parameters),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
    ) {
        Ok(reply) => {
            path_diag!(
                "[Git Desk path] GetHostPaths reply children={} raw={reply:?}",
                reply.n_children()
            );
            reply
        }
        Err(error) => {
            path_diag!("[Git Desk path] GetHostPaths D-Bus call failed: {error}");
            if error
                .to_string()
                .contains("org.freedesktop.DBus.Error.UnknownMethod")
            {
                path_diag!(
                    "[Git Desk path] GetHostPaths unavailable; trying legacy host Info fallback"
                );
                return resolve_host_path_via_info(doc_id, relative);
            }
            return None;
        }
    };

    if reply.n_children() != 1 {
        path_diag!(
            "[Git Desk path] unexpected GetHostPaths reply child count: {}",
            reply.n_children()
        );
        return None;
    }

    let paths = reply.child_value(0);
    path_diag!(
        "[Git Desk path] GetHostPaths map entries={} raw={paths:?}",
        paths.n_children()
    );

    for index in 0..paths.n_children() {
        let entry = paths.child_value(index);
        path_diag!(
            "[Git Desk path] GetHostPaths entry index={} children={} raw={entry:?}",
            index,
            entry.n_children()
        );
        if entry.n_children() != 2 {
            path_diag!(
                "[Git Desk path] skipping entry index={} because child count is {}",
                index,
                entry.n_children()
            );
            continue;
        }

        let key = entry.child_value(0);
        path_diag!("[Git Desk path] GetHostPaths entry key={key:?}");
        if key.str() != Some(doc_id) {
            path_diag!(
                "[Git Desk path] skipping entry index={} because key does not match doc_id={}",
                index,
                doc_id
            );
            continue;
        }

        let bytes = entry.child_value(1);
        path_diag!("[Git Desk path] GetHostPaths host bytes variant={bytes:?}");
        let mut host_bytes = match bytes.fixed_array::<u8>() {
            Ok(bytes) => bytes.to_vec(),
            Err(error) => {
                path_diag!("[Git Desk path] host-path byte-array parse failed: {error}");
                return None;
            }
        };
        while host_bytes.last() == Some(&0) {
            host_bytes.pop();
        }
        if host_bytes.is_empty() {
            path_diag!("[Git Desk path] host-path byte-array was empty");
            return None;
        }

        let host_root = PathBuf::from(OsString::from_vec(host_bytes));
        let candidate = if relative.as_os_str().is_empty() {
            host_root
        } else {
            host_root.join(relative)
        };
        path_diag!(
            "[Git Desk path] GetHostPaths candidate host path={}",
            candidate.display()
        );
        return Some(candidate);
    }

    path_diag!(
        "[Git Desk path] GetHostPaths returned no matching entry for doc_id={}",
        doc_id
    );
    None
}

fn resolve_host_path_via_info(doc_id: &str, relative: &Path) -> Option<PathBuf> {
    let output = run_documents_info(doc_id)?;
    if !output.status.success() {
        path_diag!(
            "[Git Desk path] legacy Info command failed status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }

    let stdout = match std::str::from_utf8(&output.stdout) {
        Ok(stdout) => stdout.trim(),
        Err(error) => {
            path_diag!("[Git Desk path] legacy Info output was not UTF-8: {error}");
            return None;
        }
    };
    path_diag!("[Git Desk path] legacy Info raw reply={stdout}");

    let reply = match glib::Variant::parse(None, stdout) {
        Ok(reply) => reply,
        Err(error) => {
            path_diag!("[Git Desk path] legacy Info GVariant parse failed: {error}");
            return None;
        }
    };

    if reply.n_children() < 1 {
        path_diag!("[Git Desk path] legacy Info reply had no path child");
        return None;
    }

    let bytes = reply.child_value(0);
    let mut host_bytes = match bytes.fixed_array::<u8>() {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => {
            path_diag!("[Git Desk path] legacy Info host-path parse failed: {error}");
            return None;
        }
    };
    while host_bytes.last() == Some(&0) {
        host_bytes.pop();
    }
    if host_bytes.is_empty() {
        path_diag!("[Git Desk path] legacy Info returned an empty host path");
        return None;
    }

    let host_root = PathBuf::from(OsString::from_vec(host_bytes));
    let candidate = if relative.as_os_str().is_empty() {
        host_root
    } else {
        host_root.join(relative)
    };
    path_diag!(
        "[Git Desk path] legacy Info candidate host path={}",
        candidate.display()
    );
    Some(candidate)
}

fn run_documents_info(doc_id: &str) -> Option<std::process::Output> {
    let flatpak = std::env::var_os("FLATPAK_ID").is_some();
    let mut command = if flatpak {
        let mut command = Command::new("flatpak-spawn");
        command.arg("--host").arg("gdbus");
        command
    } else {
        Command::new("gdbus")
    };

    command
        .current_dir("/")
        .arg("call")
        .arg("--session")
        .arg("--dest")
        .arg(DOCUMENTS_BUS)
        .arg("--object-path")
        .arg(DOCUMENTS_PATH)
        .arg("--method")
        .arg(format!("{DOCUMENTS_INTERFACE}.Info"))
        .arg(doc_id);

    match command.output() {
        Ok(output) => Some(output),
        Err(error) => {
            path_diag!("[Git Desk path] legacy Info command could not start: {error}");
            None
        }
    }
}

fn parse_portal_document_path(path: &Path) -> Option<PortalDocumentPath> {
    let parts: Vec<OsString> = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect();

    let doc_index = if parts.len() >= 5
        && part_is(&parts, 0, "run")
        && part_is(&parts, 1, "user")
        && part_is(&parts, 3, "doc")
    {
        3
    } else if parts.len() >= 4
        && part_is(&parts, 0, "run")
        && part_is(&parts, 1, "flatpak")
        && part_is(&parts, 2, "doc")
    {
        2
    } else {
        return None;
    };

    let doc_id = parts.get(doc_index + 1)?.to_string_lossy().into_owned();
    if doc_id.is_empty() || doc_id == "by-app" {
        return None;
    }

    // The first component after the document ID is the exported file or
    // directory itself. GetHostPaths already returns that host path, so only
    // append descendants below the exported root.
    let mut relative = PathBuf::new();
    for component in parts.iter().skip(doc_index + 3) {
        relative.push(component);
    }

    Some(PortalDocumentPath { doc_id, relative })
}

fn part_is(parts: &[OsString], index: usize, expected: &str) -> bool {
    parts
        .get(index)
        .and_then(|part| part.to_str())
        .is_some_and(|part| part == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_document_portal_directory_descendant() {
        let parsed = parse_portal_document_path(Path::new(
            "/run/user/1000/doc/4fff5bc1/Github Projects/sandbox",
        ))
        .expect("portal path");

        assert_eq!(parsed.doc_id, "4fff5bc1");
        assert_eq!(parsed.relative, PathBuf::from("sandbox"));
    }

    #[test]
    fn parses_flatpak_document_portal_directory_descendant() {
        let parsed =
            parse_portal_document_path(Path::new("/run/flatpak/doc/abc123/Projects/example/src"))
                .expect("portal path");

        assert_eq!(parsed.doc_id, "abc123");
        assert_eq!(parsed.relative, PathBuf::from("example/src"));
    }

    #[test]
    fn exported_root_has_no_extra_relative_path() {
        let parsed =
            parse_portal_document_path(Path::new("/run/user/1000/doc/4fff5bc1/Github Projects"))
                .expect("portal path");

        assert!(parsed.relative.as_os_str().is_empty());
    }

    #[test]
    fn ordinary_host_path_is_not_treated_as_portal_path() {
        assert!(
            parse_portal_document_path(Path::new("/home/chris/Projects/Github Projects/sandbox",))
                .is_none()
        );
    }
}
