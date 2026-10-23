//! A few results of a slow reading kept in memory by what was read, and read
//! again in the background when they go stale.
//!
//! The folder listings ([`crate::files`]) and the backlink searches
//! ([`crate::backlinks`]) are both answered this way: a window asks for a
//! reading and draws what is kept, a thread of its own does the reading, and
//! the window is told when it is done.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::Hash;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// What is kept, and the readings in flight.
pub(crate) struct KeptIndex<K, V> {
    /// Oldest first.
    kept: Vec<Kept<K, V>>,
    capacity: usize,
    /// The keys being read right now, each with the number of the reading
    /// that holds the mark — see [`Scanning`].
    marks: HashMap<K, u64>,
    next_mark: u64,
    /// Which era what is kept belongs to. See [`KeptIndex::forget`].
    era: u64,
}

struct Kept<K, V> {
    key: K,
    value: V,
    at: Instant,
}

/// One reading's claim on its key, and the era it started in.
#[derive(Debug)]
pub(crate) struct Mark<K> {
    key: K,
    id: u64,
    era: u64,
}

impl<K: Clone + Eq + Hash, V> KeptIndex<K, V> {
    /// An index that keeps at most `capacity` results.
    pub fn new(capacity: usize) -> Self {
        Self {
            kept: Vec::new(),
            capacity,
            marks: HashMap::new(),
            next_mark: 0,
            era: 0,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.kept
            .iter()
            .find(|kept| kept.key == *key)
            .map(|kept| &kept.value)
    }

    pub fn era(&self) -> u64 {
        self.era
    }

    pub fn is_marked(&self, key: &K) -> bool {
        self.marks.contains_key(key)
    }

    /// Mark `key` as being read, unless a reading of it is already under
    /// way, or what is kept of it was read less than `fresh_for` ago and
    /// `still_good` says it still answers.
    pub fn begin(
        &mut self,
        key: K,
        fresh_for: Duration,
        still_good: impl FnOnce(&V) -> bool,
    ) -> Option<Mark<K>> {
        if self.is_marked(&key) {
            return None;
        }
        let fresh = self
            .kept
            .iter()
            .find(|kept| kept.key == key)
            .is_some_and(|kept| kept.at.elapsed() < fresh_for && still_good(&kept.value));
        if fresh {
            return None;
        }
        let id = self.next_mark;
        self.next_mark = id.wrapping_add(1);
        self.marks.insert(key.clone(), id);
        Some(Mark {
            key,
            id,
            era: self.era,
        })
    }

    /// Take `mark` off, unless a newer reading of the same key holds it now.
    pub fn unmark(&mut self, mark: &Mark<K>) {
        if self.marks.get(&mark.key) == Some(&mark.id) {
            self.marks.remove(&mark.key);
        }
    }

    /// Keep a finished reading, dropping the oldest once there are too many.
    ///
    /// The reading replaces any earlier one of the same key, so a re-read
    /// updates in place rather than pushing the other keys out.
    ///
    /// A reading from before the last [`KeptIndex::forget`] is dropped
    /// instead of kept: it saw the disk as it was, and keeping it would
    /// answer the reader's reload with what they asked to be rid of — and
    /// would look freshly read for as long as anything is trusted.
    pub fn remember(&mut self, key: K, value: V, era: u64) {
        if era != self.era {
            return;
        }
        self.kept.retain(|kept| kept.key != key);
        self.kept.push(Kept {
            key,
            value,
            at: Instant::now(),
        });
        while self.kept.len() > self.capacity {
            self.kept.remove(0);
        }
    }

    /// Drop everything kept and start a new era, so a reading already under
    /// way is dropped when it comes back too.
    pub fn forget(&mut self) {
        self.kept.clear();
        self.era = self.era.wrapping_add(1);
    }
}

/// What holds a [`KeptIndex`] under its lock, alongside whatever else has to
/// change with it atomically.
pub(crate) trait HoldsKeptIndex {
    type Key: Clone + Eq + Hash;
    type Value;

    fn kept_index(&mut self) -> &mut KeptIndex<Self::Key, Self::Value>;
}

impl<K: Clone + Eq + Hash, V> HoldsKeptIndex for KeptIndex<K, V> {
    type Key = K;
    type Value = V;

    fn kept_index(&mut self) -> &mut KeptIndex<K, V> {
        self
    }
}

/// A key marked as being read, unmarked however the reading ends.
///
/// A guard rather than a line at the end of the reading: one that panics
/// would otherwise leave its key marked for the life of the process, and a
/// key that is forever being read is never read again.
pub(crate) struct Scanning<'a, I: HoldsKeptIndex> {
    index: &'a RwLock<I>,
    mark: Mark<I::Key>,
}

impl<'a, I: HoldsKeptIndex> Scanning<'a, I> {
    pub fn new(index: &'a RwLock<I>, mark: Mark<I::Key>) -> Self {
        Self { index, mark }
    }

    pub fn key(&self) -> &I::Key {
        &self.mark.key
    }

    /// The era the reading started in.
    pub fn era(&self) -> u64 {
        self.mark.era
    }

    /// Take the mark off early, under a lock the caller already holds.
    pub fn unmark(&self, index: &mut I) {
        index.kept_index().unmark(&self.mark);
    }
}

impl<I: HoldsKeptIndex> Drop for Scanning<'_, I> {
    fn drop(&mut self) {
        self.unmark(&mut self.index.write());
    }
}

/// Run `read` on a thread of its own while it holds `mark`, keep what it
/// comes back with, and announce it on `changed`.
///
/// A plain thread rather than a task: a reading is blocking work from first
/// call to last, and the runtime it would otherwise sit on is the one drawing
/// the window. A reading that comes back with nothing has given up; it says
/// nothing, since whatever replaced it will announce itself.
pub(crate) fn spawn<I>(
    index: &'static RwLock<I>,
    mark: Mark<I::Key>,
    changed: &'static broadcast::Sender<()>,
    read: impl FnOnce(&Scanning<'static, I>) -> Option<I::Value> + Send + 'static,
) -> JoinHandle<()>
where
    I: HoldsKeptIndex + Send + Sync + 'static,
    I::Key: Send + Sync,
    I::Value: Send,
{
    std::thread::spawn(move || {
        let scanning = Scanning::new(index, mark);
        let Some(value) = read(&scanning) else {
            return;
        };
        index
            .write()
            .kept_index()
            .remember(scanning.key().clone(), value, scanning.era());
        // The mark comes off before the news goes out, so a window woken by
        // it can ask for another reading straight away.
        drop(scanning);
        let _ = changed.send(());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    const LONG: Duration = Duration::from_secs(60);

    fn kept_keys(index: &KeptIndex<u32, &'static str>) -> Vec<u32> {
        index.kept.iter().map(|kept| kept.key).collect()
    }

    #[test]
    fn only_the_last_few_readings_are_kept() {
        let mut index = KeptIndex::new(2);
        for key in 0..4 {
            let era = index.era();
            index.remember(key, "read", era);
        }

        assert_eq!(kept_keys(&index), vec![2, 3]);
    }

    #[test]
    fn reading_a_key_again_replaces_what_was_kept() {
        let mut index = KeptIndex::new(2);
        index.remember(1, "first", 0);
        index.remember(2, "other", 0);
        index.remember(1, "second", 0);

        assert_eq!(index.get(&1), Some(&"second"));
        assert_eq!(kept_keys(&index), vec![2, 1]);
    }

    #[test]
    fn forgetting_drops_what_was_kept() {
        let mut index = KeptIndex::new(2);
        index.remember(1, "read", 0);

        index.forget();

        assert_eq!(index.get(&1), None);
    }

    #[test]
    fn a_reading_from_before_a_forget_is_not_kept() {
        // The reading was already under way when the reader asked for
        // everything to be read again, so it saw the disk as it was.
        let mut index = KeptIndex::new(2);
        let mark = index.begin(1, LONG, |_| true).unwrap();
        index.forget();

        index.remember(1, "stale", mark.era);

        assert_eq!(index.get(&1), None);
    }

    #[test]
    fn a_key_being_read_is_not_read_twice() {
        let mut index = KeptIndex::<u32, &str>::new(2);
        assert!(index.begin(1, LONG, |_| true).is_some());

        assert!(index.is_marked(&1));
        assert!(index.begin(1, LONG, |_| true).is_none());
    }

    #[test]
    fn a_fresh_reading_is_not_read_again() {
        let mut index = KeptIndex::new(2);
        index.remember(1, "read", 0);

        assert!(index.begin(1, LONG, |_| true).is_none());
    }

    #[test]
    fn a_stale_reading_is_read_again() {
        let mut index = KeptIndex::new(2);
        index.remember(1, "read", 0);

        assert!(index.begin(1, Duration::ZERO, |_| true).is_some());
    }

    #[test]
    fn a_reading_that_no_longer_answers_is_read_again() {
        let mut index = KeptIndex::new(2);
        index.remember(1, "read", 0);

        assert!(index.begin(1, LONG, |value| *value != "read").is_some());
    }

    #[test]
    fn a_reading_that_gave_up_leaves_a_newer_mark_alone() {
        let mut index = KeptIndex::<u32, &str>::new(2);
        let old = index.begin(1, LONG, |_| true).unwrap();
        index.unmark(&old);
        let new = index.begin(1, LONG, |_| true).unwrap();

        index.unmark(&old);

        assert!(index.is_marked(&1));
        index.unmark(&new);
        assert!(!index.is_marked(&1));
    }

    #[test]
    fn a_reading_that_falls_over_does_not_leave_its_key_marked() {
        // The panic below is the point of the test, and its message on
        // stderr is not a failure.
        let index = RwLock::new(KeptIndex::<u32, &str>::new(2));
        let mark = index.write().begin(1, LONG, |_| true).unwrap();

        let fell_over = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _scanning = Scanning::new(&index, mark);
            panic!("the reading fell over");
        }));

        assert!(fell_over.is_err());
        assert!(!index.read().is_marked(&1));
    }

    #[test]
    fn a_reading_is_kept_and_announced_once_its_mark_is_off() {
        static INDEX: LazyLock<RwLock<KeptIndex<u32, &str>>> =
            LazyLock::new(|| RwLock::new(KeptIndex::new(2)));
        static CHANGED: LazyLock<broadcast::Sender<()>> =
            LazyLock::new(|| broadcast::channel(16).0);
        let mut changed = CHANGED.subscribe();
        let mark = INDEX.write().begin(1, LONG, |_| true).unwrap();

        spawn(&INDEX, mark, &CHANGED, |_| Some("read"))
            .join()
            .unwrap();

        assert!(changed.try_recv().is_ok());
        assert_eq!(INDEX.read().get(&1), Some(&"read"));
        assert!(!INDEX.read().is_marked(&1));
    }

    #[test]
    fn a_reading_that_gives_up_keeps_and_announces_nothing() {
        static INDEX: LazyLock<RwLock<KeptIndex<u32, &str>>> =
            LazyLock::new(|| RwLock::new(KeptIndex::new(2)));
        static CHANGED: LazyLock<broadcast::Sender<()>> =
            LazyLock::new(|| broadcast::channel(16).0);
        let mut changed = CHANGED.subscribe();
        let mark = INDEX.write().begin(1, LONG, |_| true).unwrap();

        spawn(&INDEX, mark, &CHANGED, |_| None).join().unwrap();

        assert!(changed.try_recv().is_err());
        assert_eq!(INDEX.read().get(&1), None);
        assert!(!INDEX.read().is_marked(&1));
    }
}
