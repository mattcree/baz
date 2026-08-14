//! Bounded, session-only capture of Baz's tagged developer diagnostics.
//!
//! The console remains the development sink. This module adds the listener's
//! missing view of the same lines without creating a path-bearing disk log or
//! confusing diagnostics with the notification bell's curated event history.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const CAPACITY: usize = 256;

struct Log {
    started: Instant,
    lines: VecDeque<String>,
}

impl Log {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            lines: VecDeque::with_capacity(CAPACITY),
        }
    }

    fn push(&mut self, line: &str) {
        if self.lines.len() == CAPACITY {
            self.lines.pop_front();
        }
        let elapsed = self.started.elapsed().as_secs();
        self.lines
            .push_back(format!("[+{:02}:{:02}] {line}", elapsed / 60, elapsed % 60));
    }
}

fn log() -> &'static Mutex<Log> {
    static LOG: OnceLock<Mutex<Log>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(Log::new()))
}

/// Write one existing diagnostic to the console and retain it for Settings.
pub(crate) fn line(arguments: fmt::Arguments<'_>) {
    let line = arguments.to_string();
    std::println!("{line}");
    if let Ok(mut log) = log().lock() {
        log.push(&line);
    }
}

/// Newest diagnostics first, so opening Debug never requires a scroll-to-end
/// operation whose widget state could disagree with a newly arrived line.
pub(crate) fn snapshot() -> Vec<String> {
    log()
        .lock()
        .map(|log| log.lines.iter().rev().cloned().collect())
        .unwrap_or_default()
}

/// Log a tagged runtime diagnostic to both the developer console and the
/// bounded Settings → Debug stream.
#[macro_export]
macro_rules! baz_log {
    ($($arg:tt)*) => {
        $crate::diagnostic::line(format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, Log};

    #[test]
    fn the_session_log_is_bounded_and_retains_the_newest_lines() {
        let mut log = Log::new();
        for index in 0..CAPACITY + 20 {
            log.push(&format!("line {index}"));
        }
        assert_eq!(log.lines.len(), CAPACITY);
        assert!(
            log.lines
                .front()
                .is_some_and(|line| line.ends_with("line 20"))
        );
        assert!(
            log.lines
                .back()
                .is_some_and(|line| line.ends_with(&format!("line {}", CAPACITY + 19)))
        );
    }
}
