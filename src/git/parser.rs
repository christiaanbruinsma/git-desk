use crate::git::models::{
    Change, ChangeArea, ChangedFile, Commit, DiffLine, DiffLineKind, RepositoryStatus,
};

fn status_name(code: char) -> &'static str {
    match code {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'U' => "conflicted",
        'T' => "type changed",
        _ => "changed",
    }
}

pub fn parse_status_porcelain_v2(input: &str) -> RepositoryStatus {
    if input.contains('\0') {
        parse_status_records(input.split('\0'), true)
    } else {
        parse_status_records(input.lines(), false)
    }
}

fn parse_status_records<'a, I>(records: I, nul_terminated: bool) -> RepositoryStatus
where
    I: Iterator<Item = &'a str>,
{
    let mut branch = String::from("HEAD");
    let mut upstream = None;
    let mut ahead = 0;
    let mut behind = 0;
    let mut unborn = false;
    let mut detached = false;
    let mut changes = Vec::new();
    let mut records = records.peekable();

    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }

        if let Some(value) = record.strip_prefix("# branch.oid ") {
            unborn = value.trim() == "(initial)";
            continue;
        }
        if let Some(value) = record.strip_prefix("# branch.head ") {
            branch = value.trim().to_string();
            detached = branch == "(detached)";
            continue;
        }
        if let Some(value) = record.strip_prefix("# branch.upstream ") {
            upstream = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = record.strip_prefix("# branch.ab ") {
            for token in value.split_whitespace() {
                if let Some(n) = token.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = token.strip_prefix('-') {
                    behind = n.parse().unwrap_or(0);
                }
            }
            continue;
        }

        if let Some(path) = record.strip_prefix("? ") {
            changes.push(Change {
                path: path.to_string(),
                old_path: None,
                area: ChangeArea::Unstaged,
                status: "untracked".into(),
            });
            continue;
        }

        if record.starts_with("! ") {
            continue;
        }

        if record.starts_with("1 ") {
            let fields: Vec<_> = record.splitn(9, ' ').collect();
            if fields.len() < 9 {
                continue;
            }
            push_xy_changes(&mut changes, fields[1], fields[8].to_string(), None);
            continue;
        }

        if record.starts_with("2 ") {
            let fields: Vec<_> = record.splitn(10, ' ').collect();
            if fields.len() < 10 {
                continue;
            }

            let (path, old_path) = if nul_terminated {
                let old_path = records
                    .next()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                (fields[9].to_string(), old_path)
            } else {
                fields[9]
                    .split_once('\t')
                    .map(|(path, old)| (path.to_string(), Some(old.to_string())))
                    .unwrap_or_else(|| (fields[9].to_string(), None))
            };

            push_xy_changes(&mut changes, fields[1], path, old_path);
            continue;
        }

        if record.starts_with("u ") {
            let fields: Vec<_> = record.splitn(11, ' ').collect();
            let path = fields.last().copied().unwrap_or_default().to_string();
            if !path.is_empty() {
                changes.push(Change {
                    path,
                    old_path: None,
                    area: ChangeArea::Unstaged,
                    status: "conflicted".into(),
                });
            }
        }
    }

    RepositoryStatus {
        branch,
        upstream,
        ahead,
        behind,
        unborn,
        detached,
        changes,
    }
}

pub fn parse_numstat_z(input: &str) -> Vec<ChangedFile> {
    let mut files = Vec::new();
    let mut records = input.split('\0');

    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }

        let mut fields = record.splitn(3, '\t');
        let additions = fields.next().unwrap_or("0").parse().unwrap_or(0);
        let deletions = fields.next().unwrap_or("0").parse().unwrap_or(0);
        let path = fields.next().unwrap_or_default();

        let (path, old_path) = if path.is_empty() {
            // With --numstat -z, rename/copy records use an empty path field
            // followed by the pre-image and post-image paths as NUL records.
            let old_path = records.next().unwrap_or_default();
            let path = records.next().unwrap_or_default();
            let old_path = if old_path.is_empty() {
                None
            } else {
                Some(old_path.to_string())
            };
            (path, old_path)
        } else {
            (path, None)
        };

        if !path.is_empty() {
            files.push(ChangedFile {
                path: path.to_string(),
                old_path,
                additions,
                deletions,
            });
        }
    }

    files
}

fn push_xy_changes(changes: &mut Vec<Change>, xy: &str, path: String, old_path: Option<String>) {
    let x = xy.chars().next().unwrap_or('.');
    let y = xy.chars().nth(1).unwrap_or('.');

    if x != '.' {
        changes.push(Change {
            path: path.clone(),
            old_path: old_path.clone(),
            area: ChangeArea::Staged,
            status: status_name(x).into(),
        });
    }

    if y != '.' {
        changes.push(Change {
            path,
            old_path,
            area: ChangeArea::Unstaged,
            status: status_name(y).into(),
        });
    }
}

pub fn parse_history(input: &str) -> Vec<Commit> {
    input
        .split('\x1e')
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let fields: Vec<_> = record.split('\x1f').collect();
            if fields.len() < 8 {
                return None;
            }

            Some(Commit {
                id: fields[0].trim().to_string(),
                parents: fields[1]
                    .split_whitespace()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect(),
                author_name: fields[2].to_string(),
                author_email: fields[3].to_string(),
                unix_time: fields[4].parse().unwrap_or(0),
                author_date: fields[5].to_string(),
                subject: fields[6].to_string(),
                decorations: fields[7]
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
        })
        .collect()
}

pub fn parse_unified_diff(input: &str) -> Vec<DiffLine> {
    let mut old_line = 0_u32;
    let mut new_line = 0_u32;
    let mut in_hunk = false;
    let mut output = Vec::new();

    for line in input.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
            continue;
        }
        if line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }

        if line.starts_with("@@") {
            if let Some((old, new)) = parse_hunk_header(line) {
                old_line = old;
                new_line = new;
            }
            in_hunk = true;
            output.push(DiffLine {
                old_line: None,
                new_line: None,
                kind: DiffLineKind::Hunk,
                text: line.to_string(),
            });
            continue;
        }

        if !in_hunk {
            output.push(DiffLine {
                old_line: None,
                new_line: None,
                kind: DiffLineKind::Metadata,
                text: line.to_string(),
            });
            continue;
        }

        if let Some(text) = line.strip_prefix('+') {
            output.push(DiffLine {
                old_line: None,
                new_line: Some(new_line),
                kind: DiffLineKind::Added,
                text: text.to_string(),
            });
            new_line += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            output.push(DiffLine {
                old_line: Some(old_line),
                new_line: None,
                kind: DiffLineKind::Removed,
                text: text.to_string(),
            });
            old_line += 1;
        } else if let Some(text) = line.strip_prefix(' ') {
            output.push(DiffLine {
                old_line: Some(old_line),
                new_line: Some(new_line),
                kind: DiffLineKind::Context,
                text: text.to_string(),
            });
            old_line += 1;
            new_line += 1;
        } else {
            output.push(DiffLine {
                old_line: None,
                new_line: None,
                kind: DiffLineKind::Metadata,
                text: line.to_string(),
            });
        }
    }

    output
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    // @@ -old,count +new,count @@
    let mut parts = line.split_whitespace();
    let _ = parts.next()?;
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let old_start = old.split(',').next()?.parse().ok()?;
    let new_start = new.split(',').next()?.parse().ok()?;
    Some((old_start, new_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_untracked_files() {
        let input = "# branch.oid (initial)\n# branch.head main\n? assets/css/style.css\n? assets/js/script.js\n? index.html\n";
        let parsed = parse_status_porcelain_v2(input);
        assert_eq!(parsed.branch, "main");
        assert!(parsed.unborn);
        assert!(!parsed.detached);
        assert_eq!(parsed.changes.len(), 3);
    }

    #[test]
    fn parses_detached_head_state() {
        let input =
            "# branch.oid 7b3329e2e28618348f339ab66e8dbc570e792346\n# branch.head (detached)\n";
        let parsed = parse_status_porcelain_v2(input);
        assert_eq!(parsed.branch, "(detached)");
        assert!(parsed.detached);
        assert!(!parsed.unborn);
    }

    #[test]
    fn parses_ahead_and_behind_counts() {
        let input = "# branch.oid aaaaaaa\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -2\n";
        let parsed = parse_status_porcelain_v2(input);
        assert_eq!(parsed.upstream.as_deref(), Some("origin/main"));
        assert_eq!(parsed.ahead, 3);
        assert_eq!(parsed.behind, 2);
    }

    #[test]
    fn preserves_paths_with_spaces() {
        let input = "# branch.head main\n1 .M N... 100644 100644 100644 aaaaaaa bbbbbbb folder/file name.txt\n";
        let parsed = parse_status_porcelain_v2(input);
        assert_eq!(parsed.changes.len(), 1);
        assert_eq!(parsed.changes[0].path, "folder/file name.txt");
    }

    #[test]
    fn preserves_nul_terminated_unicode_paths() {
        let input = concat!(
            "# branch.oid aaaaaaa",
            "\0",
            "# branch.head main",
            "\0",
            "? café-🦊.txt",
            "\0",
        );
        let parsed = parse_status_porcelain_v2(input);

        assert_eq!(parsed.changes.len(), 1);
        assert_eq!(parsed.changes[0].path, "café-🦊.txt");
    }

    #[test]
    fn parses_nul_terminated_unicode_rename_paths() {
        let input = concat!(
            "# branch.oid aaaaaaa",
            "\0",
            "# branch.head main",
            "\0",
            "2 R. N... 100644 100644 100644 aaaaaaa bbbbbbb R100 日本語 renamed.txt",
            "\0",
            "über old.txt",
            "\0",
        );
        let parsed = parse_status_porcelain_v2(input);

        assert_eq!(parsed.changes.len(), 1);
        assert_eq!(parsed.changes[0].path, "日本語 renamed.txt");
        assert_eq!(parsed.changes[0].old_path.as_deref(), Some("über old.txt"));
        assert_eq!(parsed.changes[0].status, "renamed");
    }

    #[test]
    fn parses_numstat_z_unicode_and_rename_paths() {
        let input = concat!(
            "1\t0\tcafé-🦊.txt",
            "\0",
            "0\t0\t",
            "\0",
            "über old.txt",
            "\0",
            "日本語 renamed.txt",
            "\0",
        );
        let parsed = parse_numstat_z(input);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, "café-🦊.txt");
        assert_eq!(parsed[0].old_path, None);
        assert_eq!(parsed[0].additions, 1);
        assert_eq!(parsed[0].deletions, 0);
        assert_eq!(parsed[1].path, "日本語 renamed.txt");
        assert_eq!(parsed[1].old_path.as_deref(), Some("über old.txt"));
    }

    #[test]
    fn parses_diff_line_numbers() {
        let input = "@@ -4,2 +4,2 @@\n old\n-old title\n+new title\n";
        let lines = parse_unified_diff(input);
        assert_eq!(lines[1].old_line, Some(4));
        assert_eq!(lines[1].new_line, Some(4));
        assert_eq!(lines[2].old_line, Some(5));
        assert_eq!(lines[3].new_line, Some(5));
    }

    #[test]
    fn parses_diff_metadata_without_line_numbers() {
        let input = "diff --git a/new.txt b/new.txt\nnew file mode 100644\nindex 0000000..aaaaaaa\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+first\n+second\n";
        let lines = parse_unified_diff(input);

        assert_eq!(lines[0].kind, DiffLineKind::Metadata);
        assert_eq!(lines[0].text, "new file mode 100644");
        assert_eq!(lines[0].old_line, None);
        assert_eq!(lines[0].new_line, None);
        assert_eq!(lines[1].kind, DiffLineKind::Hunk);
        assert_eq!(lines[2].new_line, Some(1));
        assert_eq!(lines[3].new_line, Some(2));
    }

    #[test]
    fn metadata_inside_a_patch_does_not_advance_line_numbers() {
        let input = "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n";
        let lines = parse_unified_diff(input);

        assert_eq!(lines[3].kind, DiffLineKind::Metadata);
        assert_eq!(lines[3].old_line, None);
        assert_eq!(lines[3].new_line, None);
    }
}
