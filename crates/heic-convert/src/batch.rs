//! A conversion batch that can grow while it runs (more files arrive from other Explorer
//! invocations) and is processed by a small worker pool.

use std::collections::{HashSet, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use crate::convert::{convert_file, is_heic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Converted(PathBuf),
    /// Not a HEIC file, or a folder without HEIC files.
    NotHeic,
    NoHeicInFolder,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    /// HEIC files queued so far (grows while new files arrive).
    pub total: usize,
    /// HEIC files finished (converted, failed or cancelled).
    pub finished: usize,
    pub converted: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: bool,
    /// No more input will arrive and all work is done.
    pub done: bool,
}

#[derive(Default)]
struct State {
    queue: VecDeque<PathBuf>,
    seen: HashSet<PathBuf>,
    results: Vec<(PathBuf, Outcome)>,
    total: usize,
    active: usize,
    closed: bool,
    cancelled: bool,
}

#[derive(Default)]
pub struct Batch {
    state: Mutex<State>,
    changed: Condvar,
}

/// Number of parallel conversions: at most 4 (a 48 MP photo needs ~350 MB while decoding)
/// and at most cores - 1.
pub fn default_workers() -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    cores.saturating_sub(1).clamp(1, 4)
}

impl Batch {
    pub fn new() -> Arc<Self> {
        Arc::new(Batch::default())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Adds files and/or folders (folders: HEIC files directly inside, not recursive).
    pub fn add<P: AsRef<Path>>(&self, inputs: &[P]) {
        let mut s = self.lock();
        if s.closed {
            return;
        }
        for input in inputs {
            let input = input.as_ref();
            if input.is_dir() {
                let mut files: Vec<PathBuf> = std::fs::read_dir(input)
                    .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_heic(p)).collect())
                    .unwrap_or_default();
                files.sort();
                if files.is_empty() {
                    s.results.push((input.to_path_buf(), Outcome::NoHeicInFolder));
                }
                for f in files {
                    enqueue(&mut s, f);
                }
            } else if is_heic(input) {
                enqueue(&mut s, input.to_path_buf());
            } else {
                s.results.push((input.to_path_buf(), Outcome::NotHeic));
            }
        }
        drop(s);
        self.changed.notify_all();
    }

    /// No more input will be added; workers exit once the queue is empty.
    pub fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// Stops starting new files; running conversions are finished.
    pub fn cancel(&self) {
        let mut s = self.lock();
        s.cancelled = true;
        s.closed = true;
        while let Some(p) = s.queue.pop_front() {
            s.results.push((p, Outcome::Cancelled));
        }
        drop(s);
        self.changed.notify_all();
    }

    pub fn is_idle(&self) -> bool {
        let s = self.lock();
        s.queue.is_empty() && s.active == 0
    }

    pub fn progress(&self) -> Progress {
        let s = self.lock();
        let mut p = Progress { total: s.total, cancelled: s.cancelled, ..Progress::default() };
        for (_, o) in &s.results {
            match o {
                Outcome::Converted(_) => p.converted += 1,
                Outcome::Failed(_) => p.failed += 1,
                Outcome::NotHeic | Outcome::NoHeicInFolder => p.skipped += 1,
                Outcome::Cancelled => {}
            }
        }
        p.finished = s.total - s.queue.len() - s.active;
        p.done = s.closed && s.queue.is_empty() && s.active == 0;
        p
    }

    pub fn results(&self) -> Vec<(PathBuf, Outcome)> {
        self.lock().results.clone()
    }

    /// Blocks until the batch is closed and all work is done.
    pub fn wait_done(&self) {
        let mut s = self.lock();
        while !(s.closed && s.queue.is_empty() && s.active == 0) {
            s = self.changed.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn spawn_workers(self: &Arc<Self>, count: usize) -> Vec<JoinHandle<()>> {
        (0..count)
            .map(|i| {
                let batch = Arc::clone(self);
                std::thread::Builder::new()
                    .name(format!("convert-{i}"))
                    .spawn(move || batch.worker())
                    .expect("spawn worker thread")
            })
            .collect()
    }

    fn worker(&self) {
        loop {
            let path = {
                let mut s = self.lock();
                loop {
                    if let Some(p) = s.queue.pop_front() {
                        s.active += 1;
                        break p;
                    }
                    if s.closed {
                        return;
                    }
                    s = self.changed.wait(s).unwrap_or_else(|e| e.into_inner());
                }
            };
            let outcome = match catch_unwind(AssertUnwindSafe(|| convert_file(&path))) {
                Ok(Ok(target)) => Outcome::Converted(target),
                Ok(Err(e)) => Outcome::Failed(e.to_string()),
                Err(_) => Outcome::Failed("internal error".into()),
            };
            let mut s = self.lock();
            s.active -= 1;
            s.results.push((path, outcome));
            drop(s);
            self.changed.notify_all();
        }
    }
}

fn enqueue(s: &mut State, path: PathBuf) {
    let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if s.seen.insert(key) {
        s.total += 1;
        s.queue.push_back(path);
    }
}
