//! Bounded, session-scoped application health and event history.
//!
//! Scanner and playback workers already report failures as facts. This module
//! keeps the last few in a render-ready chronological log so those facts are
//! visible without a terminal. It performs no probing of its own.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const EVENT_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    Ready,
    Working,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) level: Level,
    pub(crate) label: &'static str,
}

impl Summary {
    pub(crate) fn resolve(
        scanning: bool,
        unavailable: usize,
        files_skipped: usize,
        problem: bool,
        attention: Option<Level>,
    ) -> Self {
        if unavailable > 0 {
            Self {
                level: Level::Warning,
                label: if unavailable == 1 {
                    "1 folder offline"
                } else {
                    "Folders offline"
                },
            }
        } else if problem {
            Self {
                level: Level::Error,
                label: "Needs attention",
            }
        } else if attention == Some(Level::Error) {
            Self {
                level: Level::Error,
                label: "New error",
            }
        } else if attention == Some(Level::Warning) {
            Self {
                level: Level::Warning,
                label: "New warning",
            }
        } else if files_skipped > 0 {
            Self {
                level: Level::Warning,
                label: "Files skipped",
            }
        } else if scanning {
            Self {
                level: Level::Working,
                label: "Scanning",
            }
        } else {
            Self {
                level: Level::Ready,
                label: "Ready",
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Event {
    pub(crate) level: Level,
    pub(crate) title: String,
    pub(crate) detail: String,
    at: Instant,
}

impl Event {
    pub(crate) fn age(&self) -> String {
        age_label(self.at.elapsed())
    }
}

#[derive(Debug, Default)]
pub(crate) struct Log {
    events: VecDeque<Event>,
    attention: Option<Level>,
}

impl Log {
    pub(crate) fn record(
        &mut self,
        level: Level,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) {
        if self.events.len() == EVENT_LIMIT {
            self.events.pop_front();
        }
        self.events.push_back(Event {
            level,
            title: title.into(),
            detail: detail.into(),
            at: Instant::now(),
        });
        self.attention = match (self.attention, level) {
            (_, Level::Error) => Some(Level::Error),
            (None, Level::Warning) => Some(Level::Warning),
            (attention, Level::Ready | Level::Working | Level::Warning) => attention,
        };
    }

    pub(crate) fn newest(&self) -> impl DoubleEndedIterator<Item = &Event> {
        self.events.iter().rev()
    }

    pub(crate) fn attention(&self) -> Option<Level> {
        self.attention
    }

    pub(crate) fn acknowledge(&mut self) {
        self.attention = None;
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.events.len()
    }
}

fn age_label(age: Duration) -> String {
    let seconds = age.as_secs();
    if seconds < 60 {
        "just now".to_owned()
    } else if seconds < 3_600 {
        format!("{} min ago", seconds / 60)
    } else {
        format!("{} hr ago", seconds / 3_600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_problems_outrank_work_and_ready_state() {
        assert_eq!(
            Summary::resolve(true, 3, 0, true, None).level,
            Level::Warning
        );
        assert_eq!(
            Summary::resolve(true, 3, 0, false, None).level,
            Level::Warning
        );
        assert_eq!(
            Summary::resolve(true, 0, 0, false, None).level,
            Level::Working
        );
        assert_eq!(
            Summary::resolve(false, 0, 0, false, None).level,
            Level::Ready
        );
    }

    #[test]
    fn the_log_keeps_only_the_newest_bounded_history() {
        let mut log = Log::default();
        for index in 0..EVENT_LIMIT + 5 {
            log.record(Level::Ready, format!("event {index}"), "");
        }
        assert_eq!(log.len(), EVENT_LIMIT);
        assert_eq!(
            log.newest().next().map(|event| event.title.as_str()),
            Some("event 68")
        );
        assert_eq!(
            log.newest().next_back().map(|event| event.title.as_str()),
            Some("event 5")
        );
    }

    #[test]
    fn a_new_failure_asks_for_attention_until_the_log_is_acknowledged() {
        let mut log = Log::default();
        log.record(Level::Warning, "offline", "root");
        assert_eq!(log.attention(), Some(Level::Warning));
        log.record(Level::Error, "write failed", "database");
        assert_eq!(log.attention(), Some(Level::Error));
        log.acknowledge();
        assert_eq!(log.attention(), None);
    }
}
