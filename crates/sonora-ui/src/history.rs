use std::time::{Duration, Instant};

use sonora_project::Project;

pub const DEFAULT_LIMIT: usize = 120;
pub const COALESCE_WINDOW: Duration = Duration::from_millis(600);

pub struct History {
    undo: Vec<Project>,
    redo: Vec<Project>,
    limit: usize,
    txn: bool,
    last_key: Option<String>,
    last_at: Option<Instant>,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
            txn: false,
            last_key: None,
            last_at: None,
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
    pub fn in_transaction(&self) -> bool {
        self.txn
    }
    pub fn begin(&mut self, before: Project) {
        self.push(before);
        self.txn = true;
    }
    pub fn begin_after_change(&mut self) {
        self.txn = true;
    }
    pub fn end(&mut self) {
        self.txn = false;
        self.last_key = None;
        self.last_at = None;
    }
    pub fn record(&mut self, before: Project, key: Option<&str>) {
        self.record_at(before, key, Instant::now());
    }
    fn record_at(&mut self, before: Project, key: Option<&str>, now: Instant) {
        if self.txn {
            return;
        }
        if let (Some(k), Some(prev)) = (key, self.last_key.as_deref()) {
            if prev == k
                && self
                    .last_at
                    .is_some_and(|at| now.saturating_duration_since(at) <= COALESCE_WINDOW)
            {
                self.last_at = Some(now);
                return;
            }
        }
        self.push(before);
        self.last_key = key.map(str::to_string);
        self.last_at = Some(now);
    }
    fn push(&mut self, before: Project) {
        self.undo.push(before);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self, current: Project) -> Option<Project> {
        let before = self.undo.pop()?;
        self.redo.push(current);
        self.last_key = None;
        self.last_at = None;
        Some(before)
    }
    pub fn redo(&mut self, current: Project) -> Option<Project> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.last_key = None;
        self.last_at = None;
        Some(next)
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_key = None;
        self.last_at = None;
        self.txn = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::Song;
    fn project(bars: u32) -> Project {
        let mut p = Project::with_song(Song::empty(), Default::default());
        p.song.bars = bars;
        p
    }
    #[test]
    fn undo_redo_round_trip() {
        let mut h = History::new(8);
        assert!(!h.can_undo() && !h.can_redo());
        h.record(project(4), None);
        assert!(h.can_undo());
        let restored = h.undo(project(8)).expect("应能撤销");
        assert_eq!(restored.song.bars, 4);
        assert!(h.can_redo());
        let again = h.redo(restored.clone()).expect("应能重做");
        assert_eq!(again.song.bars, 8);
        assert!(!h.can_redo());
    }
    #[test]
    fn new_edit_clears_redo() {
        let mut h = History::new(8);
        h.record(project(4), None);
        h.undo(project(8)).unwrap();
        assert!(h.can_redo());
        h.record(project(2), None);
        assert!(!h.can_redo(), "撤销后再改动必须清空重做栈");
    }
    #[test]
    fn same_key_coalesces_within_window() {
        let mut h = History::new(8);
        let t0 = Instant::now();
        h.record_at(project(1), Some("master"), t0);
        h.record_at(project(2), Some("master"), t0 + Duration::from_millis(200));
        assert_eq!(h.undo_len(), 1, "同一次拖动只应记一步");
        h.record_at(project(3), Some("master"), t0 + COALESCE_WINDOW * 2);
        assert_eq!(h.undo_len(), 2);
    }
    #[test]
    fn different_keys_do_not_coalesce() {
        let mut h = History::new(8);
        let t0 = Instant::now();
        h.record_at(project(1), Some("master"), t0);
        h.record_at(project(2), Some("bpm"), t0 + Duration::from_millis(50));
        assert_eq!(h.undo_len(), 2);
    }
    #[test]
    fn transaction_records_only_once() {
        let mut h = History::new(8);
        h.begin(project(1));
        h.record(project(2), None);
        h.record(project(3), None);
        h.end();
        assert_eq!(h.undo_len(), 1, "一次拖拽只应记一步");
    }
    #[test]
    fn limit_drops_oldest() {
        let mut h = History::new(2);
        h.record(project(1), None);
        h.record(project(2), None);
        h.record(project(3), None);
        assert_eq!(h.undo_len(), 2);
        let p = h.undo(project(9)).unwrap();
        assert_eq!(p.song.bars, 3);
        let p = h.undo(p).unwrap();
        assert_eq!(p.song.bars, 2);
        assert!(h.undo(p).is_none(), "超出上限的更早快照不应还能撤销");
    }
}
