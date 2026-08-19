//! Input validation utilities for Git Desk.
//!
//! This module provides functions to validate user input and prevent
//! security issues like command injection and path traversal attacks.

use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during input validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// The input is empty.
    #[error("Input cannot be empty")]
    EmptyInput,

    /// The Git URL has an invalid format.
    #[error("Invalid Git URL format: {0}")]
    InvalidGitUrl(String),

    /// The path contains invalid characters or attempts traversal.
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// The input contains characters that could lead to command injection.
    #[error("Invalid characters in input: potential command injection detected")]
    CommandInjection,
}

/// Validates a Git URL to prevent command injection.
///
/// # Arguments
/// * `url` - The Git URL to validate.
///
/// # Returns
/// * `Ok(())` if the URL is valid.
/// * `Err(ValidationError)` if the URL is invalid or potentially dangerous.
///
/// # Examples
/// ```
/// use git_desk::validate::validate_git_url;
///
/// assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
/// assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
/// assert!(validate_git_url("; rm -rf /").is_err());
/// ```
pub fn validate_git_url(url: &str) -> Result<(), ValidationError> {
    if url.is_empty() {
        return Err(ValidationError::EmptyInput);
    }

    // Block dangerous characters that could lead to command injection
    let dangerous_chars = [';', '&', '|', '$', '`', '>', '<', '"', '\'', '\\', '!', '{', '}'];
    if url.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err(ValidationError::CommandInjection);
    }

    // Check for valid Git URL formats
    let valid_prefixes = [
        "http://",
        "https://",
        "git@",
        "ssh://",
        "ftp://",
        "ftps://",
        "file://",
    ];

    if !valid_prefixes.iter().any(|prefix| url.starts_with(prefix)) {
        return Err(ValidationError::InvalidGitUrl(url.to_string()));
    }

    Ok(())
}

/// Validates a repository path to prevent path traversal attacks.
///
/// # Arguments
/// * `path` - The path to validate.
///
/// # Returns
/// * `Ok(())` if the path is valid.
/// * `Err(ValidationError)` if the path is invalid or attempts traversal.
///
/// # Examples
/// ```
/// use git_desk::validate::validate_repository_path;
/// use std::path::Path;
///
/// assert!(validate_repository_path(Path::new("/home/user/repo")).is_ok());
/// assert!(validate_repository_path(Path::new("../repo")).is_err());
/// ```
pub fn validate_repository_path(path: &Path) -> Result<(), ValidationError> {
    for component in path.components() {
        if let Component::ParentDir = component {
            return Err(ValidationError::InvalidPath(
                "Path traversal attempt detected".to_string(),
            ));
        }
    }
    Ok(())
}

/// Normalizes a path and checks for path traversal attempts.
///
/// # Arguments
/// * `path` - The path to normalize.
///
/// # Returns
/// * `Ok(PathBuf)` - The normalized path if valid.
/// * `Err(ValidationError)` if the path attempts traversal.
///
/// # Examples
/// ```
/// use git_desk::validate::safe_normalize_path;
/// use std::path::Path;
///
/// let path = safe_normalize_path(Path::new("/home/user/../repo")).unwrap();
/// assert_eq!(path, Path::new("/home/repo"));
/// ```
pub fn safe_normalize_path(path: &Path) -> Result<PathBuf, ValidationError> {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                normalized.push(component);
            }
            Component::ParentDir => {
                if normalized.components().count() > 0 {
                    normalized.pop();
                } else {
                    return Err(ValidationError::InvalidPath(
                        "Path traversal attempt detected".to_string(),
                    ));
                }
            }
            Component::Normal(_) | Component::CurDir => {
                normalized.push(component);
            }
        }
    }

    Ok(normalized)
}

/// Validates a branch name to prevent command injection.
///
/// # Arguments
/// * `name` - The branch name to validate.
///
/// # Returns
/// * `Ok(())` if the branch name is valid.
/// * `Err(ValidationError)` if the branch name is invalid.
pub fn validate_branch_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::EmptyInput);
    }

    // Block dangerous characters
    let dangerous_chars = [';', '&', '|', '$', '`', '>', '<', '"', '\'', '\\', '!', '{', '}'];
    if name.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err(ValidationError::CommandInjection);
    }

    // Block branch names that start with special characters
    let invalid_starts = ['.', '-', '/', '\\'];
    if let Some(first_char) = name.chars().next() {
        if invalid_starts.contains(&first_char) {
            return Err(ValidationError::InvalidPath(
                "Branch name cannot start with special characters".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validates a commit message to prevent command injection.
///
/// # Arguments
/// * `message` - The commit message to validate.
///
/// # Returns
/// * `Ok(())` if the commit message is valid.
/// * `Err(ValidationError)` if the commit message is invalid.
pub fn validate_commit_message(message: &str) -> Result<(), ValidationError> {
    if message.is_empty() {
        return Err(ValidationError::EmptyInput);
    }

    // Block dangerous characters that could break Git commands
    let dangerous_chars = ['\n', '\r'];
    if message.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err(ValidationError::CommandInjection);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_git_url_valid() {
        assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("http://gitlab.com/user/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
        assert!(validate_git_url("ssh://git@github.com/user/repo.git").is_ok());
        assert!(validate_git_url("ftp://example.com/repo.git").is_ok());
    }

    #[test]
    fn test_validate_git_url_invalid() {
        assert!(validate_git_url("").is_err());
        assert!(validate_git_url("; rm -rf /").is_err());
        assert!(validate_git_url("https://example.com; ls").is_err());
        assert!(validate_git_url("invalid").is_err());
        assert!(validate_git_url("| cat /etc/passwd").is_err());
    }

    #[test]
    fn test_validate_repository_path_valid() {
        assert!(validate_repository_path(Path::new("/home/user/repo")).is_ok());
        assert!(validate_repository_path(Path::new("repo")).is_ok());
        assert!(validate_repository_path(Path::new("/tmp/repo")).is_ok());
    }

    #[test]
    fn test_validate_repository_path_invalid() {
        assert!(validate_repository_path(Path::new("../repo")).is_err());
        assert!(validate_repository_path(Path::new("../../repo")).is_err());
        assert!(validate_repository_path(Path::new("/home/user/../repo")).is_err());
    }

    #[test]
    fn test_safe_normalize_path() {
        assert_eq!(
            safe_normalize_path(Path::new("/home/user/repo")).unwrap(),
            PathBuf::from("/home/user/repo")
        );
        assert_eq!(
            safe_normalize_path(Path::new("/home/user/../repo")).unwrap(),
            PathBuf::from("/home/repo")
        );
        assert!(safe_normalize_path(Path::new("../../repo")).is_err());
    }

    #[test]
    fn test_validate_branch_name_valid() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("feature/new-feature").is_ok());
        assert!(validate_branch_name("fix-bug-123").is_ok());
    }

    #[test]
    fn test_validate_branch_name_invalid() {
        assert!(validate_branch_name("").is_err());
        assert!(validate_branch_name("; rm -rf /").is_err());
        assert!(validate_branch_name(".hidden").is_err());
        assert!(validate_branch_name("-invalid").is_err());
    }

    #[test]
    fn test_validate_commit_message_valid() {
        assert!(validate_commit_message("Fix bug").is_ok());
        assert!(validate_commit_message("Add new feature").is_ok());
    }

    #[test]
    fn test_validate_commit_message_invalid() {
        assert!(validate_commit_message("").is_err());
        assert!(validate_commit_message("Fix bug\nAnother line").is_err());
    }
}
