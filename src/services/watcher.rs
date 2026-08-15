use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use gtk::gio::prelude::*;
use gtk::{gio, glib};
use walkdir::WalkDir;

pub struct RepositoryWatcher {
    monitors: Rc<RefCell<Vec<gio::FileMonitor>>>,
    debounce: Rc<RefCell<Option<glib::SourceId>>>,
    root: PathBuf,
    callback: Rc<dyn Fn()>,
}

impl RepositoryWatcher {
    pub fn new(root: PathBuf, callback: Rc<dyn Fn()>) -> Self {
        let watcher = Self {
            monitors: Rc::new(RefCell::new(Vec::new())),
            debounce: Rc::new(RefCell::new(None)),
            root,
            callback,
        };
        watcher.rebuild();
        watcher
    }

    pub fn rebuild(&self) {
        self.monitors.borrow_mut().clear();

        for entry in WalkDir::new(&self.root)
            .max_depth(12)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_dir())
        {
            if is_inside_git(entry.path(), &self.root) {
                continue;
            }
            self.add_directory(entry.path());
        }

        let git_dir = self.root.join(".git");
        if git_dir.is_dir() {
            self.add_directory(&git_dir);
        }
    }

    fn add_directory(&self, path: &Path) {
        let file = gio::File::for_path(path);
        let monitor = match file.monitor_directory(
            gio::FileMonitorFlags::WATCH_MOVES,
            None::<&gio::Cancellable>,
        ) {
            Ok(monitor) => monitor,
            Err(error) => {
                eprintln!(
                    "[Git Desk watcher] monitor failed path={} error={}",
                    path.display(),
                    error
                );
                return;
            }
        };

        let debounce = self.debounce.clone();
        let callback = self.callback.clone();

        monitor.connect_changed(move |_, _file, _other_file, _event_type| {
            if let Some(source) = debounce.borrow_mut().take() {
                source.remove();
            }

            let debounce_for_timeout = debounce.clone();
            let callback = callback.clone();
            let source = glib::timeout_add_local_once(Duration::from_millis(250), move || {
                debounce_for_timeout.borrow_mut().take();
                callback();
            });
            *debounce.borrow_mut() = Some(source);
        });

        self.monitors.borrow_mut().push(monitor);
    }
}

fn is_inside_git(path: &Path, root: &Path) -> bool {
    let git = root.join(".git");
    path != git && path.starts_with(&git)
}
