//! **What this process is costing the machine** — resident memory and a CPU
//! figure, for Settings → Debug and nowhere else.
//!
//! The owner: *"I'd like to also backlog a resource usage feature in the app
//! to show how much RAM/CPU it is using in the debug menu"*.
//!
//! # It is a developer's reading, not a listener's
//!
//! Every other number baz shows a listener is a fact about *their music* —
//! how long a track is, how many albums a shelf holds, what the engine is
//! doing to the signal. This is a fact about **baz**, and the two must not be
//! confused: a resident-set figure on a health surface would read as a claim
//! that something is wrong, when the honest reading of 300 MiB is *this is
//! what a decoded artwork cache costs*. So it lives in the one section that
//! already says *session diagnostics; nothing here is written to disk*, and
//! its label says whose memory it is.
//!
//! It also gives item 37's memory-budget decision a place to be *measured*
//! rather than only reasoned about, inside the running app rather than in a
//! developer console the packaged build does not have.
//!
//! # The clock is the section's, not the process's
//!
//! Nothing here is sampled unless Settings → Debug is the visible section.
//! `app.rs`'s `add_place_clocks` installs the timer with the same guard every
//! other place-owned clock carries (ADR-0020's cost argument): a subscription
//! is a function of state, so navigating away drops the timer and the event
//! loop parks. A resource meter that ran while you listened would be the one
//! thing this module is a measurement *of*.
//!
//! # What each platform can answer
//!
//! **Linux** answers both, from `/proc/self/status` and `/proc/self/stat` —
//! two small reads of a virtual file, no dependency, no syscall wrapper. See
//! `platform::sample` for why those two files and not the more usual `statm`.
//!
//! **Windows and macOS answer neither, and say so.** The figures are there
//! (`GetProcessMemoryInfo`, `GetProcessTimes`, `task_info`) but every route to
//! them is a new crate or a hand-written `extern "system"` block, and a
//! dependency is a reviewed decision in this project rather than a convenience.
//! [`Reading::Unavailable`] is what those platforms get, the section prints the
//! reason, and the reversal is one reviewed dependency away. Answering with a
//! zero would be worse than answering with nothing.

use std::time::Duration;

/// One raw observation of the process, in the units the platform reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Resident set size: the physical memory the process is actually holding.
    ///
    /// Resident rather than virtual, because virtual size counts address space
    /// nothing has ever touched — a wgpu surface and a memory-mapped font make
    /// it a number with no relationship to what the machine has given up.
    pub rss_bytes: u64,
    /// Total CPU time this process has consumed, user and system together.
    ///
    /// A *total*, not a rate. The rate is a difference between two of these
    /// over a known interval, and [`Meter`] is where that division happens —
    /// keeping the platform read to a plain cumulative counter is what makes
    /// the arithmetic testable without a process to observe.
    pub cpu: Duration,
}

/// What Settings → Debug draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading {
    /// The platform cannot answer (see the module docs).
    Unavailable,
    /// A first observation: memory is known, the rate is not yet.
    ///
    /// A rate needs two samples and an interval between them, so the first
    /// tick after opening the section has nothing honest to say about CPU.
    /// It says so rather than printing a zero, which would be a claim.
    Warming { rss_bytes: u64 },
    /// Both figures.
    Live { rss_bytes: u64, cpu_percent: f32 },
}

/// The rolling observer: the previous sample, and the reading derived from it.
#[derive(Debug, Default)]
pub struct Meter {
    previous: Option<Sample>,
}

impl Meter {
    /// Fold one observation in, and say what to draw.
    ///
    /// `interval` is how long it has been since the last observation. It is
    /// passed rather than measured so that this whole function is pure and the
    /// percentage can be tested against arithmetic instead of against a clock.
    ///
    /// The percentage is **of one core**, which is the convention `top` and
    /// every process monitor uses: a build that saturates four cores reads
    /// 400 %, and clamping it to 100 would hide exactly the case worth seeing.
    /// It is clamped below at zero only to absorb a counter that appears to go
    /// backwards, which a suspended or migrated process can produce.
    pub fn observe(&mut self, sample: Sample, interval: Duration) -> Reading {
        let previous = self.previous.replace(sample);
        let Some(previous) = previous else {
            return Reading::Warming {
                rss_bytes: sample.rss_bytes,
            };
        };
        if interval.is_zero() {
            return Reading::Warming {
                rss_bytes: sample.rss_bytes,
            };
        }
        let spent = sample.cpu.saturating_sub(previous.cpu);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a percentage of one core is a small float by construction"
        )]
        let cpu_percent = ((spent.as_secs_f64() / interval.as_secs_f64()) * 100.0) as f32;
        Reading::Live {
            rss_bytes: sample.rss_bytes,
            cpu_percent: cpu_percent.max(0.0),
        }
    }

    /// Forget the previous sample.
    ///
    /// Called when the section is left, so that returning to it warms up again
    /// rather than dividing a fresh counter by however long the listener spent
    /// somewhere else — which would report a plausible-looking average of a
    /// period nobody was watching.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

/// The two lines the section draws, or the reason there are none.
#[must_use]
pub fn lines(reading: Reading) -> Vec<String> {
    match reading {
        Reading::Unavailable => vec![
            "Not available on this platform — Baz reads these from /proc, which \
             only Linux has."
                .to_owned(),
        ],
        Reading::Warming { rss_bytes } => vec![
            format!("Memory (resident) · {}", mib(rss_bytes)),
            "Processor · measuring…".to_owned(),
        ],
        Reading::Live {
            rss_bytes,
            cpu_percent,
        } => vec![
            format!("Memory (resident) · {}", mib(rss_bytes)),
            format!("Processor · {cpu_percent:.1} % of one core"),
        ],
    }
}

/// Bytes as mebibytes, to one place.
///
/// MiB rather than MB, and stated as MiB, because the page counts this is
/// derived from are binary and a decimal figure would be a conversion nobody
/// asked for. One decimal place is the resolution at which a leak is visible
/// over a minute without the last digit flickering every tick.
#[expect(
    clippy::cast_precision_loss,
    reason = "a resident set in bytes is far inside f64's exact integer range"
)]
fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// Read this process's resident set and CPU total.
#[must_use]
pub fn sample() -> Option<Sample> {
    platform::sample()
}

#[cfg(target_os = "linux")]
mod platform {
    use std::time::Duration;

    use super::Sample;

    /// The kernel's own scale for both figures.
    ///
    /// `utime`/`stime` in `/proc/self/stat` are counted in **`USER_HZ`**,
    /// which is 100 on Linux on every architecture and is deliberately
    /// decoupled from the kernel's internal `HZ` precisely so that it can be
    /// a stable userspace ABI constant. This is why neither read here needs
    /// `sysconf`, and so why this module adds **no dependency**: `libc` is in
    /// the tree only transitively, under the audio stack, and reaching for it
    /// directly to learn two constants the ABI already fixes would be a
    /// dependency taken for convenience rather than reviewed.
    const USER_HZ: u64 = 100;

    /// `/proc/self/status`'s `VmRSS:` line, and `/proc/self/stat`'s fields 14
    /// and 15.
    ///
    /// **`status` rather than `statm` for the memory**, which is the opposite
    /// of the usual advice and is chosen for one reason: `status` reports
    /// `VmRSS` **in kB directly**, where `statm` reports it in *pages* and a
    /// page is `sysconf(_SC_PAGESIZE)` — 4 KiB on x86-64 but legitimately 16
    /// or 64 KiB on aarch64. Assuming 4096 would silently under-report by
    /// 4× or 16× on an ARM machine, and finding out for certain costs the
    /// dependency this module is avoiding. A labelled line in kB has no such
    /// question in it.
    pub fn sample() -> Option<Sample> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let kb: u64 = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()?;

        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        // **Fields are counted from after the last `)`, not from the start of
        // the line.** Field 2 is the executable's name in parentheses and it
        // may itself contain spaces and parentheses; splitting the whole line
        // on whitespace is the classic way to misparse this file, and `baz`
        // would not expose it — which is exactly why it is worth not relying
        // on. Everything that matters is after the name.
        let after_name = stat.rsplit_once(')')?.1;
        let mut fields = after_name.split_whitespace();
        // The first field after the name is `state`, field 3 — so `utime`
        // (14) and `stime` (15) are eleven and twelve fields along.
        let utime: u64 = fields.nth(11)?.parse().ok()?;
        let stime: u64 = fields.next()?.parse().ok()?;

        Some(Sample {
            rss_bytes: kb.saturating_mul(1024),
            cpu: Duration::from_millis(utime.saturating_add(stime).saturating_mul(1000) / USER_HZ),
        })
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::Sample;

    /// See the module docs: the figures exist on Windows and macOS, and every
    /// route to them is a reviewed dependency this has not spent.
    pub fn sample() -> Option<Sample> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Meter, Reading, Sample, lines};

    fn at(rss_mib: u64, cpu_ms: u64) -> Sample {
        Sample {
            rss_bytes: rss_mib * 1024 * 1024,
            cpu: Duration::from_millis(cpu_ms),
        }
    }

    /// **The first observation reports memory and refuses a rate**, because a
    /// rate needs two samples and the honest answer to *how much CPU* after
    /// one is *not yet*.
    #[test]
    fn the_first_sample_has_memory_and_no_rate() {
        let mut meter = Meter::default();
        assert_eq!(
            meter.observe(at(100, 0), Duration::from_secs(1)),
            Reading::Warming {
                rss_bytes: 100 * 1024 * 1024
            }
        );
    }

    /// **The percentage is of one core**, `top`'s convention: half a second of
    /// CPU over one second of wall clock is 50 %, and two seconds over one is
    /// 200 % rather than a clamped 100.
    #[test]
    fn the_rate_is_cpu_time_over_wall_time_on_one_core() {
        let mut meter = Meter::default();
        meter.observe(at(100, 0), Duration::from_secs(1));
        let Reading::Live { cpu_percent, .. } = meter.observe(at(100, 500), Duration::from_secs(1))
        else {
            panic!("a second sample reports a rate")
        };
        assert!((cpu_percent - 50.0).abs() < 0.01);

        let mut meter = Meter::default();
        meter.observe(at(100, 0), Duration::from_secs(1));
        let Reading::Live { cpu_percent, .. } =
            meter.observe(at(100, 2000), Duration::from_secs(1))
        else {
            panic!("a second sample reports a rate")
        };
        assert!(
            (cpu_percent - 200.0).abs() < 0.01,
            "a multi-core figure is clamped, which hides the case worth seeing"
        );
    }

    /// **A counter that appears to run backwards reports zero, not a negative
    /// percentage.** A suspended or migrated process can produce one, and a
    /// readout that went negative would be read as a defect in the readout.
    #[test]
    fn a_backwards_counter_reports_zero_rather_than_a_negative_rate() {
        let mut meter = Meter::default();
        meter.observe(at(100, 5_000), Duration::from_secs(1));
        assert_eq!(
            meter.observe(at(100, 1_000), Duration::from_secs(1)),
            Reading::Live {
                rss_bytes: 100 * 1024 * 1024,
                cpu_percent: 0.0
            }
        );
    }

    /// **Leaving the section forgets the sample**, so returning warms up again
    /// rather than dividing a fresh counter by however long the listener was
    /// elsewhere — which would report a plausible average of a period nobody
    /// was watching.
    #[test]
    fn leaving_the_section_forgets_the_previous_sample() {
        let mut meter = Meter::default();
        meter.observe(at(100, 0), Duration::from_secs(1));
        meter.reset();
        assert!(matches!(
            meter.observe(at(100, 900), Duration::from_secs(1)),
            Reading::Warming { .. }
        ));
    }

    /// A zero interval cannot be divided by, and the answer is the same
    /// refusal the first sample makes rather than an infinity.
    #[test]
    fn a_zero_interval_refuses_rather_than_dividing() {
        let mut meter = Meter::default();
        meter.observe(at(100, 0), Duration::from_secs(1));
        assert!(matches!(
            meter.observe(at(100, 900), Duration::ZERO),
            Reading::Warming { .. }
        ));
    }

    /// **Every state says whose numbers these are and in what units**, and
    /// none of them prints a bare figure that could read as a health verdict.
    #[test]
    fn every_reading_names_its_subject_and_its_units() {
        for reading in [
            Reading::Unavailable,
            Reading::Warming {
                rss_bytes: 123 * 1024 * 1024,
            },
            Reading::Live {
                rss_bytes: 123 * 1024 * 1024,
                cpu_percent: 12.5,
            },
        ] {
            let drawn = lines(reading).join(" ");
            assert!(
                drawn.contains("Memory") || drawn.contains("Not available"),
                "{drawn}"
            );
        }
        assert_eq!(
            lines(Reading::Live {
                rss_bytes: 123 * 1024 * 1024,
                cpu_percent: 12.5
            }),
            vec![
                "Memory (resident) · 123.0 MiB".to_owned(),
                "Processor · 12.5 % of one core".to_owned(),
            ]
        );
    }

    /// **The Linux read answers, and answers plausibly.** It is running inside
    /// a process, so the figures are real: a Rust test binary is never zero
    /// bytes resident, and its CPU total is never a century.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_running_process_reports_a_plausible_resident_set() {
        let sample = super::sample().expect("Linux answers from /proc");
        assert!(
            sample.rss_bytes > 1024 * 1024,
            "{} bytes resident is not a running test binary",
            sample.rss_bytes
        );
        assert!(sample.rss_bytes < 64 * 1024 * 1024 * 1024);
        assert!(sample.cpu < Duration::from_secs(60 * 60));
    }
}
