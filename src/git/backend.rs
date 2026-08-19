use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use gtk::gio;
use log::{debug, error, info};
use thiserror::Error;

use crate::git::{
    models::{Branch, ChangedFile, Commit, RepositoryStatus, StashEntry},
    parser::{parse_history, parse_numstat_z, parse_status_porcelain_v2},
};
use crate::validate::{validate_branch_name, validate_commit_message, validate_git_url, validate_repository_path};


#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git could not be started: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("{0}")]
    Command(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Clone)]
pub struct GitBackend {
    path: PathBuf,
    // Caching voor performance
    status_cache: std::sync::Arc<std::sync::Mutex<Option<(RepositoryStatus, std::time::Instant)>>>,
    branches_cache: std::sync::Arc<std::sync::Mutex<Option<(Vec<Branch>, std::time::Instant)>>>,
    tags_cache: std::sync::Arc<std::sync::Mutex<Option<(Vec<TagEntry>, std::time::Instant)>>>,
    cache_ttl: std::time::Duration,
}

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone)]
pub struct RepositoryState {
    pub status: RepositoryStatus,
    pub branches: Vec<Branch>,
    pub tags: Vec<TagEntry>,
    pub stashes: Vec<StashEntry>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEntry {
    pub name: String,
    pub target: String,
    pub annotated: bool,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOperationKind {
    Revert,
    CherryPick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryOperation {
    pub kind: HistoryOperationKind,
    pub commit: String,
}

impl GitBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            status_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            branches_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            tags_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            cache_ttl: std::time::Duration::from_secs(2), // Cache voor 2 seconden
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn discover(path: PathBuf) -> Result<Option<PathBuf>> {
        let backend = Self::new(path);
        let result = backend
            .run_allow_failure(vec!["rev-parse".into(), "--show-toplevel".into()])
            .await?;

        if !result.success {
            return Ok(None);
        }

        let root = result.stdout.trim();
        if root.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(root)))
        }
    }

    pub async fn init(path: PathBuf) -> Result<Self> {
        let backend = Self::new(path);
        backend
            .run(vec!["init".into(), "--initial-branch=main".into()])
            .await?;
        Ok(backend)
    }

    pub async fn clone_repository(url: String, parent: PathBuf) -> Result<Self> {
        validate_git_url(&url).map_err(|e| GitError::Command(e.to_string()))?;
        validate_repository_path(&parent).map_err(|e| GitError::Command(e.to_string()))?;
        validate_repository_path(&parent).map_err(|e| GitError::Command(e.to_string()))?;
        
        info!("Cloning repository from URL: {}", url);
        debug!("Destination parent: {}", parent.display());
        
        let name = clone_directory_name(&url).ok_or_else(|| {
            GitError::Command(
                "Git Desk could not determine a repository name from that URL.".into(),
            )
        })?;
        let destination = parent.join(name);

        if destination.exists() {
            return Err(GitError::Command(format!(
                "A folder named '{}' already exists in the selected location.",
                destination
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("repository")
            )));
        }

        let destination_for_command = destination.clone();
        let output = gio::spawn_blocking(move || {
            run_git_global_command(vec![
                "clone".into(),
                "--".into(),
                url.into(),
                destination_for_command.as_os_str().to_os_string(),
            ])
        })
        .await
        .map_err(|_| GitError::Command("Git worker thread failed unexpectedly.".into()))??;

        if !output.success {
            let message = output.stderr.trim();
            return Err(GitError::Command(if message.is_empty() {
                "Git clone failed.".into()
            } else {
                message.into()
            }));
        }

        Ok(Self::new(destination))
    }

    pub async fn status(&self) -> Result<RepositoryStatus> {
        // Check cache
        {
            let cache = self.status_cache.lock().unwrap();
            if let Some((status, timestamp)) = &*cache {
                if timestamp.elapsed() < self.cache_ttl {
                    debug!("Returning cached status");
                    return Ok(status.clone());
                }
            }
        }

        // Execute command
        let output = self
            .run(vec![
                "--no-optional-locks".into(),
                "status".into(),
                "--porcelain=v2".into(),
                "-z".into(),
                "--branch".into(),
                "--untracked-files=all".into(),
            ])
            .await?;

        let status = parse_status_porcelain_v2(&output.stdout);

        // Update cache
        {
            let mut cache = self.status_cache.lock().unwrap();
            *cache = Some((status.clone(), std::time::Instant::now()));
        }

        Ok(status)
    }

    pub async fn stage(&self, path: String, old_path: Option<String>) -> Result<()> {
        let mut args = vec!["add".into(), "-A".into(), "--".into()];
        if let Some(old_path) = old_path {
            args.push(old_path.into());
        }
        args.push(path.into());
        self.run(args).await?;
        Ok(())
    }

    pub async fn stage_all(&self) -> Result<()> {
        self.run(vec!["add".into(), "-A".into()]).await?;
        Ok(())
    }

    pub async fn unstage(&self, path: String, old_path: Option<String>) -> Result<()> {
        let mut paths = Vec::<OsString>::new();
        if let Some(old_path) = old_path {
            paths.push(old_path.into());
        }
        paths.push(path.into());

        let mut restore_args = vec!["restore".into(), "--staged".into(), "--".into()];
        restore_args.extend(paths.iter().cloned());
        let restore = self.run_allow_failure(restore_args).await?;

        if restore.success {
            return Ok(());
        }

        // Before the first commit HEAD does not exist. Staged new files are
        // safely returned to untracked state by removing them from the index.
        let mut fallback = vec![
            "rm".into(),
            "--cached".into(),
            "-r".into(),
            "--ignore-unmatch".into(),
            "--".into(),
        ];
        fallback.extend(paths);
        self.run(fallback).await?;
        Ok(())
    }

    pub async fn unstage_all(&self) -> Result<()> {
        let restore = self
            .run_allow_failure(vec![
                "restore".into(),
                "--staged".into(),
                "--".into(),
                ".".into(),
            ])
            .await?;

        if restore.success {
            return Ok(());
        }

        // In an unborn repository there is no HEAD to restore the index from.
        self.run(vec![
            "rm".into(),
            "--cached".into(),
            "-r".into(),
            "--ignore-unmatch".into(),
            "--".into(),
            ".".into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn discard_worktree(&self, path: String, untracked: bool) -> Result<()> {
        if untracked {
            // `git clean` only removes files Git still considers untracked at
            // execution time, which is safer than deleting the path directly.
            self.run(vec!["clean".into(), "-f".into(), "--".into(), path.into()])
                .await?;
        } else {
            // Restore from the index so any staged version remains intact.
            self.run(vec![
                "restore".into(),
                "--worktree".into(),
                "--".into(),
                path.into(),
            ])
            .await?;
        }
        Ok(())
    }

    pub async fn commit(&self, message: String) -> Result<()> {
        validate_commit_message(&message).map_err(|e| GitError::Command(e.to_string()))?;
        info!("Committing with message: {}", message);
        self.run(vec!["commit".into(), "-m".into(), message.into()])
            .await
            .map_err(|e| {
                error!("Commit failed: {}", e);
                e
            })?;
        info!("Commit successful");
        Ok(())
    }

    pub async fn head_commit_id(&self) -> Result<Option<String>> {
        let output = self
            .run_allow_failure(vec!["rev-parse".into(), "--verify".into(), "HEAD".into()])
            .await?;

        if !output.success {
            return Ok(None);
        }

        let id = output.stdout.trim();
        Ok((!id.is_empty()).then(|| id.to_string()))
    }

    async fn ensure_head(&self, expected_head: &str) -> Result<()> {
        if self.head_commit_id().await?.as_deref() == Some(expected_head) {
            return Ok(());
        }

        Err(GitError::Command(
            "The current commit changed before this action could run. Refresh and try again."
                .into(),
        ))
    }

    pub async fn amend_commit_message(&self, expected_head: String, message: String) -> Result<()> {
        self.ensure_head(&expected_head).await?;
        self.run(vec![
            "commit".into(),
            "--amend".into(),
            "--only".into(),
            "-m".into(),
            message.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn amend_staged_changes(&self, expected_head: String) -> Result<()> {
        self.ensure_head(&expected_head).await?;
        self.run(vec!["commit".into(), "--amend".into(), "--no-edit".into()])
            .await?;
        Ok(())
    }

    pub async fn undo_head_commit(&self, expected_head: String) -> Result<()> {
        self.ensure_head(&expected_head).await?;

        let output = self
            .run(vec![
                "rev-list".into(),
                "--parents".into(),
                "-n".into(),
                "1".into(),
                expected_head.clone().into(),
            ])
            .await?;
        let mut ids = output.stdout.split_whitespace();
        let current = ids.next().unwrap_or_default();
        if current != expected_head {
            return Err(GitError::Command(
                "Git Desk could not verify the commit before undoing it.".into(),
            ));
        }

        if let Some(parent) = ids.next() {
            self.run(vec![
                "update-ref".into(),
                "HEAD".into(),
                parent.into(),
                expected_head.into(),
            ])
            .await?;
        } else {
            self.run(vec![
                "update-ref".into(),
                "-d".into(),
                "HEAD".into(),
                expected_head.into(),
            ])
            .await?;
        }

        Ok(())
    }

    pub async fn working_diff(
        &self,
        path: String,
        staged: bool,
        untracked: bool,
    ) -> Result<String> {
        if untracked && !staged {
            // `git diff` omits untracked files. --no-index gives us a normal
            // unified patch against /dev/null; exit status 1 simply means
            // differences were found and is therefore expected here.
            let output = self
                .run_allow_failure(vec![
                    "-c".into(),
                    "core.quotePath=false".into(),
                    "diff".into(),
                    "--no-index".into(),
                    "--no-ext-diff".into(),
                    "--".into(),
                    "/dev/null".into(),
                    path.into(),
                ])
                .await?;
            if !output.stdout.is_empty() {
                return Ok(output.stdout);
            }
            if output.success {
                return Ok(String::new());
            }
            return Err(GitError::Command(output.stderr.trim().to_string()));
        }

        let mut args = vec![
            "-c".into(),
            "core.quotePath=false".into(),
            "diff".into(),
            "--no-ext-diff".into(),
            "--".into(),
            path.into(),
        ];
        if staged {
            // Git's per-command config must remain `-c key=value`; place
            // `--cached` after the `diff` subcommand instead of between them.
            args.insert(3, "--cached".into());
        }
        Ok(self.run(args).await?.stdout)
    }

    pub async fn commit_diff(
        &self,
        commit: String,
        path: String,
        old_path: Option<String>,
    ) -> Result<String> {
        let mut args = vec![
            "-c".into(),
            "core.quotePath=false".into(),
            "show".into(),
            "--format=".into(),
            "--no-ext-diff".into(),
            "--find-renames".into(),
            commit.into(),
            "--".into(),
        ];
        if let Some(old_path) = old_path {
            args.push(old_path.into());
        }
        args.push(path.into());

        Ok(self.run(args).await?.stdout)
    }

    pub async fn history(&self, limit: usize) -> Result<Vec<Commit>> {
        let format = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%aI%x1f%s%x1f%D%x1e";
        let output = self
            .run_allow_failure(vec![
                "log".into(),
                "--all".into(),
                "--topo-order".into(),
                format!("--max-count={limit}").into(),
                format!("--format={format}").into(),
            ])
            .await?;

        if !output.success {
            return Ok(Vec::new());
        }

        Ok(parse_history(&output.stdout))
    }

    /// Paginated history for lazy loading.
    /// This allows loading commits in batches as the user scrolls.
    pub async fn history_paginated(&self, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        debug!("Loading history (skip={}, limit={})", skip, limit);
        
        let format = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%aI%x1f%s%x1f%D%x1e";
        let output = self
            .run_allow_failure(vec![
                "log".into(),
                "--all".into(),
                "--topo-order".into(),
                format!("--skip={skip}").into(),
                format!("--max-count={limit}").into(),
                format!("--format={format}").into(),
            ])
            .await?;

        if !output.success {
            return Ok(Vec::new());
        }

        Ok(parse_history(&output.stdout))
    }

    pub async fn outgoing_commits(&self, upstream: String) -> Result<Vec<Commit>> {
        let format = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%aI%x1f%s%x1f%D%x1e";
        let range = format!("{upstream}..HEAD");
        let output = self
            .run(vec![
                "log".into(),
                "--topo-order".into(),
                range.into(),
                format!("--format={format}").into(),
            ])
            .await?;

        Ok(parse_history(&output.stdout))
    }

    pub async fn unpublished_commits(&self) -> Result<Vec<Commit>> {
        let format = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%aI%x1f%s%x1f%D%x1e";
        let output = self
            .run(vec![
                "log".into(),
                "--topo-order".into(),
                "HEAD".into(),
                "--not".into(),
                "--remotes".into(),
                format!("--format={format}").into(),
            ])
            .await?;

        Ok(parse_history(&output.stdout))
    }

    
    /// Batch function to get multiple repository states in a single call.
    /// This is more efficient than calling each function separately.
    pub async fn get_repository_state(&self) -> Result<RepositoryState> {
        info!("Fetching repository state (sequential)");
        
        let status = self.status().await?;
        let branches = self.branches().await?;
        let tags = self.tags().await?;
        let stashes = self.stashes().await?;

        Ok(RepositoryState {
            status,
            branches,
            tags,
            stashes,
        })
    }
pub async fn stashes(&self) -> Result<Vec<StashEntry>> {
        let output = self
            .run_allow_failure(vec![
                "stash".into(),
                "list".into(),
                "--format=%gd%x1f%H%x1f%s%x1e".into(),
            ])
            .await?;

        if !output.success {
            return Ok(Vec::new());
        }

        let mut stashes = Vec::new();
        for record in output.stdout.split('\u{1e}') {
            let record = record.trim_matches(|character| character == '\n' || character == '\r');
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split('\u{1f}').collect();
            if fields.len() < 3 {
                continue;
            }
            stashes.push(StashEntry {
                reference: fields[0].to_string(),
                id: fields[1].to_string(),
                subject: fields[2].to_string(),
            });
        }
        Ok(stashes)
    }

    pub async fn create_stash(&self, message: String, include_untracked: bool) -> Result<bool> {
        let before = self.stash_head().await?;
        let mut args = vec!["stash".into(), "push".into()];
        if include_untracked {
            args.push("--include-untracked".into());
        }
        if !message.trim().is_empty() {
            args.push("-m".into());
            args.push(message.into());
        }
        self.run(args).await?;
        let after = self.stash_head().await?;
        Ok(after.is_some() && after != before)
    }

    async fn stash_head(&self) -> Result<Option<String>> {
        let output = self
            .run_allow_failure(vec![
                "rev-parse".into(),
                "--verify".into(),
                "refs/stash".into(),
            ])
            .await?;
        if !output.success {
            return Ok(None);
        }
        let id = output.stdout.trim();
        Ok((!id.is_empty()).then(|| id.to_string()))
    }

    pub async fn stash_apply(&self, reference: String) -> Result<()> {
        self.run(vec!["stash".into(), "apply".into(), reference.into()])
            .await?;
        Ok(())
    }

    pub async fn stash_pop(&self, reference: String) -> Result<()> {
        self.run(vec!["stash".into(), "pop".into(), reference.into()])
            .await?;
        Ok(())
    }

    pub async fn stash_drop(&self, reference: String) -> Result<()> {
        self.run(vec!["stash".into(), "drop".into(), reference.into()])
            .await?;
        Ok(())
    }

    pub async fn stash_files(&self, reference: String) -> Result<Vec<ChangedFile>> {
        let output = self
            .run(vec![
                "stash".into(),
                "show".into(),
                "--include-untracked".into(),
                "--numstat".into(),
                "-z".into(),
                reference.into(),
            ])
            .await?;

        Ok(parse_numstat_z(&output.stdout))
    }

    pub async fn stash_diff(&self, reference: String) -> Result<String> {
        Ok(self
            .run(vec![
                "-c".into(),
                "core.quotePath=false".into(),
                "stash".into(),
                "show".into(),
                "--include-untracked".into(),
                "-p".into(),
                "--no-ext-diff".into(),
                reference.into(),
            ])
            .await?
            .stdout)
    }

    pub async fn commit_message(&self, commit: String) -> Result<String> {
        let output = self
            .run(vec![
                "show".into(),
                "-s".into(),
                "--format=%B".into(),
                commit.into(),
            ])
            .await?;

        Ok(output.stdout.trim_end_matches('\n').to_string())
    }

    pub async fn changed_files(&self, commit: String) -> Result<Vec<ChangedFile>> {
        let output = self
            .run(vec![
                "show".into(),
                "--format=".into(),
                "--numstat".into(),
                "-z".into(),
                "--find-renames".into(),
                commit.into(),
            ])
            .await?;

        Ok(parse_numstat_z(&output.stdout))
    }

    pub async fn branches(&self) -> Result<Vec<Branch>> {
        // Check cache
        {
            let cache = self.branches_cache.lock().unwrap();
            if let Some((branches, timestamp)) = &*cache {
                if timestamp.elapsed() < self.cache_ttl {
                    debug!("Returning cached branches");
                    return Ok(branches.clone());
                }
            }
        }

        let output = self
            .run_allow_failure(vec![
                "for-each-ref".into(),
                "--sort=refname".into(),
                "--format=%(refname)%09%(refname:short)%09%(HEAD)%09%(upstream:short)".into(),
                "refs/heads".into(),
                "refs/remotes".into(),
            ])
            .await?;

        let mut branches = Vec::new();
        if output.success {
            for line in output.stdout.lines().filter(|line| !line.is_empty()) {
                let fields: Vec<_> = line.split('\t').collect();
                if fields.len() < 3 {
                    continue;
                }
                let refname = fields[0];
                branches.push(Branch {
                    name: fields[1].to_string(),
                    current: fields[2] == "*",
                    upstream: fields
                        .get(3)
                        .filter(|v| !v.is_empty())
                        .map(|v| (*v).to_string()),
                    remote: refname.starts_with("refs/remotes/"),
                    unborn: false,
                });
            }
        }

        let current = self
            .run_allow_failure(vec!["branch".into(), "--show-current".into()])
            .await?;
        let current_name = current.stdout.trim();

        if !current_name.is_empty()
            && !branches
                .iter()
                .any(|branch| !branch.remote && branch.name == current_name)
        {
            branches.insert(
                0,
                Branch {
                    name: current_name.to_string(),
                    current: true,
                    upstream: None,
                    remote: false,
                    unborn: true,
                },
            );
        }

        // Update cache
        {
            let mut cache = self.branches_cache.lock().unwrap();
            *cache = Some((branches.clone(), std::time::Instant::now()));
        }

        Ok(branches)
    }

    pub async fn create_and_switch_branch(&self, name: String) -> Result<()> {
        validate_branch_name(&name).map_err(|e| GitError::Command(e.to_string()))?;
        info!("Creating and switching to branch: {}", name);
        self.run(vec!["switch".into(), "-c".into(), name.into()])
            .await?;
        Ok(())
    }

    pub async fn switch_branch(&self, name: String) -> Result<()> {
        validate_branch_name(&name).map_err(|e| GitError::Command(e.to_string()))?;
        info!("Switching to branch: {}", name);
        self.run(vec!["switch".into(), name.into()]).await?;
        Ok(())
    }

    pub async fn rename_branch(&self, old_name: String, new_name: String) -> Result<()> {
        validate_branch_name(&old_name).map_err(|e| GitError::Command(e.to_string()))?;
        validate_branch_name(&new_name).map_err(|e| GitError::Command(e.to_string()))?;
        info!("Renaming branch from {} to {}", old_name, new_name);
        self.run(vec![
            "branch".into(),
            "-m".into(),
            old_name.into(),
            new_name.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn delete_branch(&self, name: String) -> Result<()> {
        validate_branch_name(&name).map_err(|e| GitError::Command(e.to_string()))?;
        info!("Deleting branch: {}", name);
        self.run(vec!["branch".into(), "-d".into(), name.into()])
            .await?;
        Ok(())
    }

    pub async fn merge_branch(&self, name: String) -> Result<()> {
        self.run(vec!["merge".into(), "--no-edit".into(), name.into()])
            .await?;
        Ok(())
    }

    pub async fn merge_in_progress(&self) -> Result<bool> {
        let output = self
            .run_allow_failure(vec![
                "rev-parse".into(),
                "-q".into(),
                "--verify".into(),
                "MERGE_HEAD".into(),
            ])
            .await?;
        Ok(output.success)
    }

    pub async fn unresolved_conflicts(&self) -> Result<Vec<String>> {
        let output = self
            .run(vec![
                "diff".into(),
                "--name-only".into(),
                "--diff-filter=U".into(),
                "-z".into(),
            ])
            .await?;
        Ok(output
            .stdout
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect())
    }

    pub async fn complete_merge(&self) -> Result<()> {
        self.run(vec!["commit".into(), "--no-edit".into()]).await?;
        Ok(())
    }

    pub async fn abort_merge(&self) -> Result<()> {
        self.run(vec!["merge".into(), "--abort".into()]).await?;
        Ok(())
    }

    pub async fn history_operation(&self) -> Result<Option<HistoryOperation>> {
        for (reference, kind) in [
            ("CHERRY_PICK_HEAD", HistoryOperationKind::CherryPick),
            ("REVERT_HEAD", HistoryOperationKind::Revert),
        ] {
            let output = self
                .run_allow_failure(vec![
                    "rev-parse".into(),
                    "-q".into(),
                    "--verify".into(),
                    reference.into(),
                ])
                .await?;
            if output.success {
                let commit = output.stdout.trim();
                if !commit.is_empty() {
                    return Ok(Some(HistoryOperation {
                        kind,
                        commit: commit.to_string(),
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn ensure_single_parent_commit(&self, commit: &str) -> Result<()> {
        let output = self
            .run(vec![
                "rev-list".into(),
                "--parents".into(),
                "-n".into(),
                "1".into(),
                commit.into(),
            ])
            .await?;
        let ids: Vec<_> = output.stdout.split_whitespace().collect();
        if ids.first().copied() != Some(commit) {
            return Err(GitError::Command(
                "Git Desk could not verify the selected commit.".into(),
            ));
        }
        if ids.len() > 2 {
            return Err(GitError::Command(
                "Merge commits need an explicit mainline parent before they can be reverted or cherry-picked. Git Desk does not guess that parent.".into(),
            ));
        }
        Ok(())
    }

    async fn ensure_history_operation(&self, expected: HistoryOperationKind) -> Result<()> {
        match self.history_operation().await? {
            Some(operation) if operation.kind == expected => Ok(()),
            Some(_) => Err(GitError::Command(
                "A different Git history operation is currently in progress. Refresh and review Changes before continuing.".into(),
            )),
            None => Err(GitError::Command(
                "There is no matching Git history operation in progress.".into(),
            )),
        }
    }

    pub async fn commit_is_ancestor_of_head(&self, commit: String) -> Result<bool> {
        let output = self
            .run_allow_failure(vec![
                "merge-base".into(),
                "--is-ancestor".into(),
                commit.into(),
                "HEAD".into(),
            ])
            .await?;
        if output.success {
            return Ok(true);
        }
        if output.stderr.trim().is_empty() {
            return Ok(false);
        }
        Err(GitError::Command(output.stderr.trim().to_string()))
    }

    pub async fn revert_commit(&self, commit: String) -> Result<()> {
        self.ensure_single_parent_commit(&commit).await?;
        if self.history_operation().await?.is_some() {
            return Err(GitError::Command(
                "Finish or abort the current history operation before starting another one.".into(),
            ));
        }
        self.run(vec!["revert".into(), "--no-edit".into(), commit.into()])
            .await?;
        Ok(())
    }

    pub async fn cherry_pick_commit(&self, commit: String) -> Result<()> {
        self.ensure_single_parent_commit(&commit).await?;
        if self.history_operation().await?.is_some() {
            return Err(GitError::Command(
                "Finish or abort the current history operation before starting another one.".into(),
            ));
        }
        self.run(vec!["cherry-pick".into(), commit.into()]).await?;
        Ok(())
    }

    pub async fn continue_history_operation(&self, kind: HistoryOperationKind) -> Result<()> {
        self.ensure_history_operation(kind).await?;
        let command = match kind {
            HistoryOperationKind::Revert => "revert",
            HistoryOperationKind::CherryPick => "cherry-pick",
        };
        self.run(vec![command.into(), "--continue".into()]).await?;
        Ok(())
    }

    pub async fn abort_history_operation(&self, kind: HistoryOperationKind) -> Result<()> {
        self.ensure_history_operation(kind).await?;
        let command = match kind {
            HistoryOperationKind::Revert => "revert",
            HistoryOperationKind::CherryPick => "cherry-pick",
        };
        self.run(vec![command.into(), "--abort".into()]).await?;
        Ok(())
    }

    pub async fn skip_cherry_pick(&self) -> Result<()> {
        self.ensure_history_operation(HistoryOperationKind::CherryPick)
            .await?;
        self.run(vec!["cherry-pick".into(), "--skip".into()])
            .await?;
        Ok(())
    }

    pub async fn tags(&self) -> Result<Vec<TagEntry>> {
        // Check cache
        {
            let cache = self.tags_cache.lock().unwrap();
            if let Some((tags, timestamp)) = &*cache {
                if timestamp.elapsed() < self.cache_ttl {
                    debug!("Returning cached tags");
                    return Ok(tags.clone());
                }
            }
        }

        let output = self
            .run(vec![
                "for-each-ref".into(),
                "--sort=-creatordate".into(),
                "--format=%(refname:short)%00%(objecttype)%00%(objectname)%00%(*objectname)%00%(subject)".into(),
                "refs/tags".into(),
            ])
            .await?;

        let mut tags = Vec::new();
        for line in output.stdout.lines().filter(|line| !line.is_empty()) {
            let mut fields = line.split('\0');
            let Some(name) = fields.next() else { continue };
            let Some(object_type) = fields.next() else {
                continue;
            };
            let Some(object_id) = fields.next() else {
                continue;
            };
            let peeled_id = fields.next().unwrap_or_default();
            let subject = fields.next().unwrap_or_default();
            let annotated = object_type == "tag";
            let target = if annotated && !peeled_id.is_empty() {
                peeled_id
            } else {
                object_id
            };

            tags.push(TagEntry {
                name: name.to_string(),
                target: target.to_string(),
                annotated,
                subject: if annotated {
                    subject.to_string()
                } else {
                    String::new()
                },
            });
        }

        // Update cache
        {
            let mut cache = self.tags_cache.lock().unwrap();
            *cache = Some((tags.clone(), std::time::Instant::now()));
        }

        Ok(tags)
    }

    pub async fn tag_message(&self, name: String) -> Result<String> {
        let output = self
            .run(vec![
                "for-each-ref".into(),
                "--format=%(contents)".into(),
                format!("refs/tags/{name}").into(),
            ])
            .await?;
        Ok(output.stdout.trim_end().to_string())
    }

    pub async fn create_tag(&self, name: String, message: String) -> Result<()> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(GitError::Command("Enter a tag name.".into()));
        }

        let reference = format!("refs/tags/{name}");
        let check = self
            .run_allow_failure(vec!["check-ref-format".into(), reference.into()])
            .await?;
        if !check.success {
            return Err(GitError::Command(format!(
                "'{name}' is not a valid Git tag name."
            )));
        }

        if message.trim().is_empty() {
            self.run(vec!["tag".into(), "--".into(), name.into()])
                .await?;
        } else {
            self.run(vec![
                "tag".into(),
                "-a".into(),
                "-m".into(),
                message.into(),
                "--".into(),
                name.into(),
            ])
            .await?;
        }
        Ok(())
    }

    pub async fn delete_tag(&self, name: String) -> Result<()> {
        self.run(vec![
            "tag".into(),
            "--delete".into(),
            "--".into(),
            name.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn push_tag(&self, remote: String, name: String) -> Result<()> {
        let reference = format!("refs/tags/{name}");
        let refspec = format!("{reference}:{reference}");
        self.run(vec![
            "push".into(),
            "--".into(),
            remote.into(),
            refspec.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn remotes(&self) -> Result<Vec<(String, String)>> {
        let output = self.run(vec!["remote".into()]).await?;
        let names: Vec<String> = output
            .stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        let mut remotes = Vec::new();

        for name in names {
            let url = self
                .run(vec!["remote".into(), "get-url".into(), name.clone().into()])
                .await?
                .stdout
                .trim()
                .to_string();
            remotes.push((name, url));
        }

        Ok(remotes)
    }

    pub async fn add_remote(&self, name: String, url: String) -> Result<()> {
        self.run(vec!["remote".into(), "add".into(), name.into(), url.into()])
            .await?;
        Ok(())
    }

    pub async fn remove_remote(&self, name: String) -> Result<()> {
        self.run(vec!["remote".into(), "remove".into(), name.into()])
            .await?;
        Ok(())
    }

    pub async fn set_upstream(&self, branch: String, upstream: String) -> Result<()> {
        self.run(vec![
            "branch".into(),
            format!("--set-upstream-to={upstream}").into(),
            branch.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn unset_upstream(&self, branch: String) -> Result<()> {
        self.run(vec![
            "branch".into(),
            "--unset-upstream".into(),
            branch.into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn fetch(&self) -> Result<()> {
        self.run(vec!["fetch".into(), "--all".into(), "--prune".into()])
            .await?;
        Ok(())
    }

    pub async fn pull(&self) -> Result<()> {
        self.run(vec!["pull".into(), "--ff-only".into()]).await?;
        Ok(())
    }

    pub async fn push(&self) -> Result<()> {
        self.run(vec!["push".into()]).await?;
        Ok(())
    }

    pub async fn push_set_upstream(&self, remote: String, branch: String) -> Result<()> {
        self.run(vec![
            "push".into(),
            "--set-upstream".into(),
            remote.into(),
            branch.into(),
        ])
        .await?;
        Ok(())
    }

    async fn run(&self, args: Vec<OsString>) -> Result<CommandOutput> {
        let output = self.run_allow_failure(args).await?;
        if output.success {
            Ok(output)
        } else {
            let message = output.stderr.trim();
            Err(GitError::Command(if message.is_empty() {
                "Git command failed.".into()
            } else {
                message.into()
            }))
        }
    }

    async fn run_allow_failure(&self, args: Vec<OsString>) -> Result<CommandOutput> {
        let repo = self.path.clone();
        let output = gio::spawn_blocking(move || run_git_command(&repo, args))
            .await
            .map_err(|_| GitError::Command("Git worker thread failed unexpectedly.".into()))??;
        Ok(output)
    }
}

fn run_git_command(repo: &Path, args: Vec<OsString>) -> std::io::Result<CommandOutput> {
    let mut command = git_command();

    // Do not rely on the app/process cwd. The repository is always explicit.
    command.current_dir("/");
    command.arg("-C").arg(repo);
    command.args(args);

    command_output(command)
}

fn run_git_global_command(args: Vec<OsString>) -> std::io::Result<CommandOutput> {
    let mut command = git_command();

    // Clone does not have a repository yet. Keep cwd neutral and pass the
    // destination as an absolute path instead of relying on process cwd.
    command.current_dir("/");
    command.args(args);

    command_output(command)
}

fn git_command() -> Command {
    if std::env::var_os("FLATPAK_ID").is_some() {
        let mut command = Command::new("flatpak-spawn");
        command.arg("--host").arg("git");
        command
    } else {
        Command::new("git")
    }
}

fn command_output(mut command: Command) -> std::io::Result<CommandOutput> {
    let output = command.output()?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn clone_directory_name(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let tail = trimmed.rsplit(['/', ':']).next()?;
    let name = tail.strip_suffix(".git").unwrap_or(tail);

    if name.is_empty() || name == "." || name == ".." {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_validate_git_url_integration() {
        // Test valid URLs
        assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
        
        // Test invalid URLs (command injection attempts)
        assert!(validate_git_url("").is_err());
        assert!(validate_git_url("; rm -rf /").is_err());
        assert!(validate_git_url("https://example.com; ls").is_err());
    }

    #[tokio::test]
    async fn test_init_repository() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let result = GitBackend::init(path.clone()).await;
        assert!(result.is_ok());
        
        let backend = result.unwrap();
        assert!(path.join(".git").exists());
    }

    #[tokio::test]
    async fn test_discover_existing_repo() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        // First initialize a repo
        let _ = GitBackend::init(path.clone()).await;
        
        // Then try to discover it
        let result = GitBackend::discover(path).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_discover_nonexistent_repo() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let result = GitBackend::discover(path).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_status_parsing() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        // Initialize a repo
        let backend = GitBackend::init(path.clone()).await.unwrap();
        
        // Get status (should work on empty repo)
        let result = backend.status().await;
        assert!(result.is_ok());
        
        let status = result.unwrap();
        assert_eq!(status.branch, "main");
        assert!(status.changes.is_empty());
    }

    #[tokio::test]
    async fn test_commit_validation() {
        // Test valid commit message
        assert!(validate_commit_message("Fix bug").is_ok());
        assert!(validate_commit_message("Add new feature").is_ok());
        
        // Test invalid commit messages
        assert!(validate_commit_message("").is_err());
        assert!(validate_commit_message("Fix bug\nAnother line").is_err());
    }

    #[tokio::test]
    async fn test_branch_name_validation() {
        // Test valid branch names
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("feature/new-feature").is_ok());
        
        // Test invalid branch names
        assert!(validate_branch_name("").is_err());
        assert!(validate_branch_name("; rm -rf /").is_err());
        assert!(validate_branch_name(".hidden").is_err());
        assert!(validate_branch_name("-invalid").is_err());
    }

    #[test]
    fn test_clone_directory_name() {
        // Test valid URLs
        assert_eq!(
            clone_directory_name("https://github.com/user/repo.git"),
            Some("repo".to_string())
        );
        assert_eq!(
            clone_directory_name("git@github.com:user/repo.git"),
            Some("repo".to_string())
        );
        assert_eq!(
            clone_directory_name("https://github.com/user/my-repo"),
            Some("my-repo".to_string())
        );
        
        // Test URLs with .git suffix
        assert_eq!(
            clone_directory_name("https://github.com/user/my-repo.git"),
            Some("my-repo".to_string())
        );
        
        // Test invalid cases
        assert_eq!(clone_directory_name(""), None);
        assert_eq!(clone_directory_name("../repo"), None);
        assert_eq!(clone_directory_name("."), None);
        assert_eq!(clone_directory_name(".."), None);
    }
}
