use std::collections::HashSet;
use uuid::Uuid;

use super::{App, Mode, SearchState};

impl App {
    pub fn begin_search(&mut self) {
        self.search = Some(SearchState {
            query: String::new(),
            matches: Vec::new(),
            match_idx: 0,
            original_collapsed: self.collapsed.clone(),
        });
        self.mode = Mode::Search;
    }

    pub fn search_push(&mut self, c: char) {
        if let Some(ref mut s) = self.search {
            s.query.push(c);
        }
        self.recompute_matches();
    }

    pub fn search_pop(&mut self) {
        if let Some(ref mut s) = self.search {
            s.query.pop();
        }
        self.recompute_matches();
    }

    /// Exit search input mode, keep match state active for n/N navigation.
    pub fn commit_search(&mut self) {
        self.mode = Mode::Normal;
    }

    /// Escape: clear search and restore the original fold state.
    pub fn cancel_search(&mut self) {
        if let Some(s) = self.search.take() {
            self.collapsed = s.original_collapsed;
        }
        self.mode = Mode::Normal;
    }

    pub fn search_next(&mut self) {
        if let Some(ref mut s) = self.search {
            if s.matches.is_empty() {
                return;
            }
            s.match_idx = (s.match_idx + 1) % s.matches.len();
        }
        self.jump_to_current_match();
    }

    pub fn search_prev(&mut self) {
        if let Some(ref mut s) = self.search {
            if s.matches.is_empty() {
                return;
            }
            s.match_idx = s.match_idx
                .checked_sub(1)
                .unwrap_or_else(|| s.matches.len() - 1);
        }
        self.jump_to_current_match();
    }

    pub fn recompute_matches(&mut self) {
        let query_lower = match self.search.as_ref() {
            Some(s) if !s.query.is_empty() => s.query.to_lowercase(),
            _ => {
                if let Some(ref mut s) = self.search {
                    s.matches.clear();
                    s.match_idx = 0;
                }
                return;
            }
        };

        // Traverse all layer-visible tasks in DFS order, ignoring collapse state,
        // so collapsed tasks are searchable too.
        let matches: Vec<Uuid> = self.all_visible_dfs()
            .into_iter()
            .filter(|&id| {
                self.task_ref(id)
                    .map(|t| t.title.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .collect();

        if let Some(ref mut s) = self.search {
            if matches.is_empty() {
                s.match_idx = 0;
            } else {
                s.match_idx = s.match_idx.min(matches.len() - 1);
            }
            s.matches = matches;
        }

        self.jump_to_current_match();
    }

    /// DFS-ordered list of all tasks that pass `task_visible`, regardless of
    /// whether their ancestors are collapsed.
    fn all_visible_dfs(&self) -> Vec<Uuid> {
        let visible: HashSet<Uuid> = self.tasks.iter()
            .filter(|t| self.task_visible(t))
            .map(|t| t.id)
            .collect();

        let roots: Vec<Uuid> = self.tasks.iter()
            .filter(|t| {
                visible.contains(&t.id)
                    && t.parent_id.map(|pid| !visible.contains(&pid)).unwrap_or(true)
            })
            .map(|t| t.id)
            .collect();

        let mut result = Vec::new();
        for root_id in roots {
            self.dfs_collect(root_id, &visible, &mut result);
        }
        result
    }

    fn dfs_collect(&self, id: Uuid, visible: &HashSet<Uuid>, out: &mut Vec<Uuid>) {
        out.push(id);
        if let Some(task) = self.task_ref(id) {
            for &cid in &task.children {
                if visible.contains(&cid) {
                    self.dfs_collect(cid, visible, out);
                }
            }
        }
    }

    fn jump_to_current_match(&mut self) {
        let match_id = self.search.as_ref().and_then(|s| s.matches.get(s.match_idx).copied());
        if let Some(id) = match_id {
            // Restore original fold state so only this match's ancestors stay open.
            if let Some(ref s) = self.search {
                self.collapsed = s.original_collapsed.clone();
            }
            self.unfold_ancestors(id);
            self.navigate_to_id(id);
        }
    }

    /// Remove all ancestors of `id` from the collapsed set so the task is visible.
    fn unfold_ancestors(&mut self, id: Uuid) {
        let mut cur = id;
        loop {
            match self.task_ref(cur).and_then(|t| t.parent_id) {
                Some(pid) => {
                    self.collapsed.remove(&pid);
                    cur = pid;
                }
                None => break,
            }
        }
    }
}
