//! Answers of earlier runs, so that a document re-rendered after an edit
//! gets its unchanged blocks back at once instead of running the command
//! over all of them again.

use std::collections::{HashMap, VecDeque};

/// A map from a job's cache key to the command's answer, bounded by the
/// total size of the answers, dropping the oldest first. Only answers that
/// could be shown go in: a failure may be passing, and retrying it is what
/// the reader wants.
pub(crate) struct ResultCache {
    max_bytes: usize,
    bytes: usize,
    entries: HashMap<[u8; 32], String>,
    order: VecDeque<[u8; 32]>,
}

impl ResultCache {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(crate) fn get(&self, key: &[u8; 32]) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Keep `answer`, unless it alone is larger than the whole cache.
    pub(crate) fn insert(&mut self, key: [u8; 32], answer: String) {
        if answer.len() > self.max_bytes {
            return;
        }
        self.bytes += answer.len();
        match self.entries.insert(key, answer) {
            Some(previous) => self.bytes -= previous.len(),
            None => self.order.push_back(key),
        }
        while self.bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(dropped) = self.entries.remove(&oldest) {
                self.bytes -= dropped.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> [u8; 32] {
        [n; 32]
    }

    #[test]
    fn a_stored_answer_comes_back() {
        let mut cache = ResultCache::new(100);
        cache.insert(key(1), "one".to_string());
        assert_eq!(cache.get(&key(1)), Some("one"));
        assert_eq!(cache.get(&key(2)), None);
    }

    #[test]
    fn the_oldest_answers_go_first_once_the_size_is_reached() {
        let mut cache = ResultCache::new(10);
        cache.insert(key(1), "1234".to_string());
        cache.insert(key(2), "5678".to_string());
        cache.insert(key(1), "abcd".to_string());
        cache.insert(key(3), "wxyz".to_string());

        assert_eq!(cache.get(&key(1)), None);
        assert_eq!(cache.get(&key(2)), Some("5678"));
        assert_eq!(cache.get(&key(3)), Some("wxyz"));
    }

    #[test]
    fn an_answer_larger_than_the_cache_is_not_kept() {
        let mut cache = ResultCache::new(3);
        cache.insert(key(1), "1234".to_string());
        assert_eq!(cache.get(&key(1)), None);
    }
}
