use std::collections::HashSet;

use crate::git::models::Commit;

#[derive(Debug, Clone)]
pub struct GraphRow {
    pub commit: Commit,
    pub lane: usize,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub parent_lanes: Vec<usize>,
    pub lane_count: usize,
    pub starts_lane: bool,
}

pub fn build_history_graph(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<String> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let existing_lane = lanes.iter().position(|id| id == &commit.id);
        let starts_lane = existing_lane.is_none();
        let lane = existing_lane.unwrap_or_else(|| {
            lanes.insert(0, commit.id.clone());
            0
        });

        let before = lanes.clone();
        let mut after = lanes.clone();
        after.remove(lane);

        if let Some(first_parent) = commit.parents.first() {
            after.insert(lane, first_parent.clone());

            let mut insert_at = lane + 1;
            for parent in commit.parents.iter().skip(1) {
                if let Some(existing) = after.iter().position(|id| id == parent) {
                    after.remove(existing);
                    if existing < insert_at {
                        insert_at = insert_at.saturating_sub(1);
                    }
                }
                after.insert(insert_at, parent.clone());
                insert_at += 1;
            }
        }

        let mut seen = HashSet::new();
        after.retain(|id| seen.insert(id.clone()));

        let parent_lanes = commit
            .parents
            .iter()
            .filter_map(|parent| after.iter().position(|id| id == parent))
            .collect::<Vec<_>>();

        let lane_count = before.len().max(after.len()).max(lane + 1);

        rows.push(GraphRow {
            commit: commit.clone(),
            lane,
            before,
            after: after.clone(),
            parent_lanes,
            lane_count,
            starts_lane,
        });

        lanes = after;
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            parents: parents.iter().map(|value| (*value).into()).collect(),
            author_name: "Test".into(),
            author_email: "test@example.com".into(),
            unix_time: 0,
            author_date: "1970-01-01T00:00:00+00:00".into(),
            subject: id.into(),
            decorations: vec![],
        }
    }

    #[test]
    fn merge_uses_multiple_lanes() {
        let commits = vec![
            commit("m", &["a", "b"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ];
        let rows = build_history_graph(&commits);
        assert!(rows.iter().any(|row| row.lane_count >= 2));
        assert_eq!(rows[0].parent_lanes.len(), 2);
        assert!(rows[0].starts_lane);
        assert!(!rows[1].starts_lane);
        assert!(!rows[2].starts_lane);
    }
}
