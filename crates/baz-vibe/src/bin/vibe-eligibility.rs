//! **Measure what "eligible" should mean**, before anything is built on top of
//! it.
//!
//! Design 21 §3 says the words decide *which* songs are eligible. Nothing in
//! the shipped engine draws that line — selection is one blended cost over
//! every analysed track — so before a match count, an eligible cloud or a
//! why-line can be honest, somebody has to choose the policy that draws it.
//! This is that measurement, and it is a sweep rather than a taste call
//! because CLAP similarity distributions move with the phrase: a floor that
//! keeps three hundred songs for *calm piano instrumental* can keep four for
//! *wistful but not tragic*.
//!
//! It reads an existing analysis store — the semantic vectors are already
//! there, one per track — joins each track to its library genre as a weak
//! relevance judgement, embeds every committed request with the shipping text
//! tower, and reports:
//!
//! 1. the similarity distribution per request;
//! 2. what each candidate policy keeps — fixed floor, top-K per cent, elbow;
//! 3. how much each kept set concentrates the labels the request asks for,
//!    as a lift over the corpus's own base rate;
//! 4. the tick-bucket boundaries the kept pool implies;
//! 5. every candidate vocabulary chip's discrimination and pull.
//!
//! Nothing here opens Baz's own data directory: both databases are given as
//! paths, and the intended use is a copy.

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Fixed cosine floors swept. CLAP text/audio cosines live in a narrow band
/// well under 0.5, so these are spaced where the mass actually is.
const FLOORS: [f32; 6] = [0.05, 0.10, 0.15, 0.20, 0.25, 0.30];
/// Top-K per cent cuts swept, as fractions of the analysed library.
const TOP_FRACTIONS: [f32; 5] = [0.005, 0.01, 0.02, 0.05, 0.10];
/// The elbow search never looks past this fraction of the library: past it a
/// "largest gap" is noise in the tail rather than the end of the answer.
const ELBOW_HORIZON: f32 = 0.25;
/// …and never cuts above this many tracks, so a pool always has room for a
/// playlist and its diversity rules.
const ELBOW_FLOOR: usize = 24;
/// **Distribution-relative floors**, in standard deviations above this
/// request's own mean. The fixed floors below are swept beside these to show
/// why they cannot work: the cosine distribution moves wholesale with the
/// phrase, so an absolute line means something different in every request.
const SIGMAS: [f32; 4] = [1.0, 1.5, 2.0, 2.5];

fn main() {
    if let Err(error) = run() {
        eprintln!("vibe eligibility: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() != 5 {
        return Err("usage: vibe-eligibility STORE LIBRARY REQUESTS OUTPUT".into());
    }
    let corpus = Corpus::read(Path::new(&arguments[1]), Path::new(&arguments[2]))?;
    eprintln!(
        "corpus: {} analysed tracks, {} with a genre",
        corpus.tracks.len(),
        corpus.tracks.iter().filter(|t| t.genre.is_some()).count()
    );
    let requests: Requests = serde_json::from_slice(&std::fs::read(Path::new(&arguments[3]))?)?;

    let mut report = Report {
        analysed_tracks: corpus.tracks.len(),
        labelled_tracks: corpus.tracks.iter().filter(|t| t.genre.is_some()).count(),
        requests: Vec::new(),
        chips: Vec::new(),
        policy_means: Vec::new(),
    };

    for request in &requests.requests {
        eprintln!("request {}", request.id);
        let expected = expected_genres(&request.id);
        let row = measure(&corpus, &request.id, &request.query, expected)?;
        report.requests.push(row);
    }
    for (label, prompt) in STARTING_POINTS {
        eprintln!("starting point {label}");
        let row = measure(&corpus, label, prompt, &[])?;
        report.requests.push(row);
    }
    report.policy_means = summarise_policies(&report.requests);

    for candidate in CHIPS {
        eprintln!("chip {}", candidate.word);
        report.chips.push(score_chip(&corpus, &candidate)?);
    }

    std::fs::write(
        Path::new(&arguments[4]),
        serde_json::to_string_pretty(&report)? + "\n",
    )?;
    Ok(())
}

// ---------------------------------------------------------------- the corpus

struct Track {
    similarity: f32,
    genre: Option<String>,
}

struct Corpus {
    tracks: Vec<Entry>,
}

struct Entry {
    semantic: Vec<f32>,
    genre: Option<String>,
}

impl Corpus {
    fn read(store: &Path, library: &Path) -> Result<Self, Box<dyn Error>> {
        let genres = read_genres(library)?;
        let connection = Connection::open(store)?;
        let mut statement = connection
            .prepare("SELECT path, semantic_blob FROM features WHERE semantic_blob IS NOT NULL")?;
        let mut tracks = Vec::new();
        let rows = statement.query_map([], |row| {
            let path: Vec<u8> = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((path, blob))
        })?;
        for row in rows {
            let (path, blob) = row?;
            let semantic = decode_floats(&blob);
            if semantic.is_empty() {
                continue;
            }
            let genre = genres.get(&path).cloned();
            tracks.push(Entry { semantic, genre });
        }
        if tracks.is_empty() {
            return Err("the store holds no semantic vectors".into());
        }
        Ok(Self { tracks })
    }

    /// Every track's cosine against one embedded request.
    fn against(&self, request: &[f32]) -> Vec<Track> {
        self.tracks
            .iter()
            .map(|entry| Track {
                similarity: entry
                    .semantic
                    .iter()
                    .zip(request)
                    .map(|(left, right)| left * right)
                    .sum(),
                genre: entry.genre.clone(),
            })
            .collect()
    }

    /// What share of the labelled corpus carries any of these genres — the
    /// base rate a kept set's concentration is a lift over.
    fn base_rate(&self, expected: &[&str]) -> f32 {
        let labelled: Vec<_> = self
            .tracks
            .iter()
            .filter_map(|t| t.genre.as_deref())
            .collect();
        if labelled.is_empty() || expected.is_empty() {
            return 0.0;
        }
        let hits = labelled
            .iter()
            .filter(|genre| matches(genre, expected))
            .count();
        #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
        {
            hits as f32 / labelled.len() as f32
        }
    }
}

fn read_genres(library: &Path) -> Result<HashMap<Vec<u8>, String>, Box<dyn Error>> {
    let connection = Connection::open(library)?;
    let mut statement =
        connection.prepare("SELECT path, genre FROM tracks WHERE genre IS NOT NULL")?;
    let mut genres = HashMap::new();
    let rows = statement.query_map([], |row| {
        let path: Vec<u8> = row.get(0)?;
        let genre: String = row.get(1)?;
        Ok((path, genre))
    })?;
    for row in rows {
        let (path, genre) = row?;
        genres.insert(path, genre);
    }
    Ok(genres)
}

fn decode_floats(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// A genre matches when any expected word appears in it: real tags are
/// `Pop;Rock`, `Pop-Rock`, `Retrospective Pop`, so an equality test would
/// throw away most of the judgements the library actually carries.
fn matches(genre: &str, expected: &[&str]) -> bool {
    let genre = genre.to_lowercase();
    expected
        .iter()
        .any(|word| genre.contains(&word.to_lowercase()))
}

// ------------------------------------------------------------- the judgements

/// **Weak relevance judgements, stated rather than assumed.**
///
/// Only the concrete requests get one: a genre is a defensible proxy for
/// *gentle acoustic jazz trio* and is not one for *wistful but not tragic*.
/// The subtle-affect, negative and arc requests are measured for their
/// distributions and excluded from the label scoring, which is why the report
/// carries the two sets apart instead of averaging a number that would be
/// partly meaningless.
fn expected_genres(id: &str) -> &'static [&'static str] {
    match id {
        "calm-piano" => &["classical", "new age"],
        "bright-rock" => &["rock", "alternative", "indie", "punk"],
        "gentle-jazz" => &["jazz", "blues"],
        "industrial" => &["electronic", "industrial", "metal"],
        "dream-pop" => &["pop", "alternative", "indie"],
        "focus" => &["classical", "new age", "ambient"],
        _ => &[],
    }
}

/// The six starting points of design 21 §4, measured beside the committed
/// requests because they are what most listeners will actually send.
const STARTING_POINTS: [(&str, &str); 6] = [
    (
        "start:sunday-morning",
        "gentle unhurried music for a slow morning",
    ),
    (
        "start:late-night-drive",
        "warm hypnotic music for driving at night",
    ),
    (
        "start:focus",
        "calm instrumental music without vocals for concentrating",
    ),
    ("start:workout", "fast loud driving music with a hard pulse"),
    (
        "start:wind-down",
        "quiet soft slow music for the end of the day",
    ),
    ("start:party", "upbeat energetic danceable music"),
];

// ------------------------------------------------------------- the chip sweep

/// One candidate for design 21 §4's vocabulary, with the row it would sit in
/// and — where a genre is a defensible proxy for it — the labels it should
/// pull towards.
struct ChipCandidate {
    row: &'static str,
    word: &'static str,
    expected: &'static [&'static str],
}

/// The candidates. Three rows, more per row than will ship: twelve places, and
/// which twelve is what the sweep decides.
const CHIPS: [ChipCandidate; 27] = [
    // What it is made of.
    ChipCandidate {
        row: "made of",
        word: "piano",
        expected: &["classical", "jazz"],
    },
    ChipCandidate {
        row: "made of",
        word: "acoustic guitar",
        expected: &["folk", "country"],
    },
    ChipCandidate {
        row: "made of",
        word: "electric guitars",
        expected: &["rock", "alternative", "indie", "punk"],
    },
    ChipCandidate {
        row: "made of",
        word: "synthesizers",
        expected: &["electronic"],
    },
    ChipCandidate {
        row: "made of",
        word: "strings",
        expected: &["classical"],
    },
    ChipCandidate {
        row: "made of",
        word: "brass",
        expected: &["jazz"],
    },
    ChipCandidate {
        row: "made of",
        word: "female vocals",
        expected: &[],
    },
    ChipCandidate {
        row: "made of",
        word: "no vocals",
        expected: &["classical", "soundtrack"],
    },
    ChipCandidate {
        row: "made of",
        word: "drum machine",
        expected: &["electronic"],
    },
    // What it feels like.
    ChipCandidate {
        row: "feels like",
        word: "warm",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "dark",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "melancholy",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "euphoric",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "tense",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "dreamy",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "raw",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "hopeful",
        expected: &[],
    },
    ChipCandidate {
        row: "feels like",
        word: "nostalgic",
        expected: &[],
    },
    // How it moves.
    ChipCandidate {
        row: "moves like",
        word: "slow",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "driving",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "hypnotic",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "sparse",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "dense",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "danceable",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "steady",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "swelling",
        expected: &[],
    },
    ChipCandidate {
        row: "moves like",
        word: "restless",
        expected: &[],
    },
];

/// The base phrases a chip is appended to when its pull is measured. Ordinary
/// requests rather than probes: a chip has to work where it will be pressed.
const CHIP_BASES: [&str; 5] = [
    "gentle unhurried music for a slow morning",
    "warm hypnotic music for driving at night",
    "fast loud driving music with a hard pulse",
    "quiet soft slow music for the end of the day",
    "upbeat energetic danceable music",
];

/// The set a chip's pull is measured over: two per cent of the library, which
/// is a pool of about a hundred on the corpus this was swept on.
const CHIP_SET: f32 = 0.02;

// ----------------------------------------------------------------- the report

#[derive(Deserialize)]
struct Requests {
    requests: Vec<Request>,
}

#[derive(Deserialize)]
struct Request {
    id: String,
    query: String,
}

#[derive(Serialize)]
struct Report {
    analysed_tracks: usize,
    labelled_tracks: usize,
    requests: Vec<RequestRow>,
    policy_means: Vec<PolicyMean>,
    chips: Vec<ChipRow>,
}

#[derive(Serialize)]
struct RequestRow {
    id: String,
    query: String,
    judged: bool,
    base_rate: f32,
    distribution: Distribution,
    policies: Vec<PolicyRow>,
    /// Where the two tick boundaries fall inside the recommended pool, as
    /// cosines and as fractions of the pool.
    tick_boundaries: Option<[f32; 2]>,
    /// **Against the dumbest possible baseline**: the tracks whose genre tag
    /// already contains the word the request is obviously about.
    ///
    /// Doc 23 §7.2. The question is not *which is more accurate* — judging a
    /// tag filter by tags would be circular and it would win by construction
    /// — but **whether the model is doing anything a tag filter does not
    /// already do**. If the two sets are nearly the same, the tags are free,
    /// exact and instant, and the model is 350 MiB and an hour of analysis
    /// spent reproducing them.
    tags: Option<TagBaseline>,
}

#[derive(Serialize)]
struct TagBaseline {
    /// How many tracks the tag filter alone returns.
    kept: usize,
    /// How much of the model's pool the tag filter also holds.
    overlap: f32,
    /// How much of the tag filter's set the model found.
    recall: f32,
    /// …and how much of the model's pool is *not* in the tag set, which is
    /// the only place its value can be.
    beyond_tags: f32,
}

#[derive(Serialize)]
struct Distribution {
    minimum: f32,
    p50: f32,
    p90: f32,
    p99: f32,
    maximum: f32,
    mean: f32,
    deviation: f32,
}

#[derive(Serialize, Clone)]
struct PolicyRow {
    policy: String,
    kept: usize,
    /// Kept as a share of the analysed library.
    share: f32,
    /// Share of the *kept* set carrying an expected label.
    precision: f32,
    /// That share over the corpus's own — 1.0 is no concentration at all.
    lift: f32,
}

#[derive(Serialize)]
struct PolicyMean {
    policy: String,
    mean_kept: f32,
    /// How much the kept count varies across requests, relative to its own
    /// mean. A count readout is only informative if this is large.
    kept_variation: f32,
    smallest_kept: usize,
    largest_kept: usize,
    /// Averaged over the judged requests only.
    mean_lift: f32,
    /// How often the policy kept fewer tracks than a playlist needs.
    unfillable: usize,
}

#[derive(Serialize)]
struct ChipRow {
    row: String,
    word: String,
    /// How far the chip's own best two per cent sits above the corpus mean,
    /// in corpus standard deviations. A word the tower has no opinion about
    /// scores near zero.
    discrimination: f32,
    /// How much appending the chip moves a base request's pool *towards the
    /// chip's own meaning*, averaged over the five bases. Positive is the
    /// whole point of a vocabulary.
    pull: f32,
    /// How much appending the chip displaces the pool at all — a chip that
    /// changes nothing is decoration.
    displacement: f32,
    /// Genre lift of the chip's own set, where a proxy was stated.
    lift: Option<f32>,
}

// ------------------------------------------------------------ the measurement

fn measure(
    corpus: &Corpus,
    id: &str,
    query: &str,
    expected: &[&str],
) -> Result<RequestRow, Box<dyn Error>> {
    let embedding = baz_vibe::embed_request(query)?;
    let mut scored = corpus.against(&embedding);
    scored.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    let similarities: Vec<f32> = scored.iter().map(|track| track.similarity).collect();

    let mut policies = Vec::new();
    for floor in FLOORS {
        let kept = similarities.iter().filter(|value| **value >= floor).count();
        policies.push(policy_row(
            format!("floor {floor:.2}"),
            &scored,
            kept,
            corpus,
            expected,
        ));
    }
    for fraction in TOP_FRACTIONS {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a bounded share of a library count"
        )]
        let kept = ((scored.len() as f32) * fraction).round() as usize;
        policies.push(policy_row(
            format!("top {:.1}%", fraction * 100.0),
            &scored,
            kept.max(1),
            corpus,
            expected,
        ));
    }
    let spread = distribution(&similarities);
    for sigma in SIGMAS {
        let floor = spread.deviation.mul_add(sigma, spread.mean);
        let kept = similarities.iter().filter(|value| **value >= floor).count();
        policies.push(policy_row(
            format!("mean+{sigma:.1}sd"),
            &scored,
            kept.max(1),
            corpus,
            expected,
        ));
    }
    let elbow = elbow_cut(&similarities);
    policies.push(policy_row(
        "elbow".to_owned(),
        &scored,
        elbow,
        corpus,
        expected,
    ));

    // Tick boundaries are read inside the elbow pool, as its own terciles —
    // the honest form given that the distribution moves with the phrase.
    let tick_boundaries =
        (elbow >= 3).then(|| [similarities[elbow * 2 / 3], similarities[elbow / 3]]);

    // The tag baseline, over the same corpus and the same expected words.
    let tags = (!expected.is_empty()).then(|| {
        let tagged: Vec<bool> = scored
            .iter()
            .map(|track| {
                track
                    .genre
                    .as_deref()
                    .is_some_and(|genre| matches(genre, expected))
            })
            .collect();
        let kept = tagged.iter().filter(|held| **held).count();
        let shared = tagged[..elbow].iter().filter(|held| **held).count();
        let union = elbow + kept - shared;
        #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
        TagBaseline {
            kept,
            overlap: if union == 0 {
                0.0
            } else {
                shared as f32 / union as f32
            },
            recall: if kept == 0 {
                0.0
            } else {
                shared as f32 / kept as f32
            },
            beyond_tags: if elbow == 0 {
                0.0
            } else {
                (elbow - shared) as f32 / elbow as f32
            },
        }
    });

    Ok(RequestRow {
        id: id.to_owned(),
        query: query.to_owned(),
        judged: !expected.is_empty(),
        base_rate: corpus.base_rate(expected),
        distribution: distribution(&similarities),
        policies,
        tick_boundaries,
        tags,
    })
}

fn policy_row(
    policy: String,
    scored: &[Track],
    kept: usize,
    corpus: &Corpus,
    expected: &[&str],
) -> PolicyRow {
    let kept = kept.min(scored.len());
    let labelled: Vec<_> = scored[..kept]
        .iter()
        .filter_map(|track| track.genre.as_deref())
        .collect();
    #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
    let precision = if labelled.is_empty() || expected.is_empty() {
        0.0
    } else {
        labelled
            .iter()
            .filter(|genre| matches(genre, expected))
            .count() as f32
            / labelled.len() as f32
    };
    let base = corpus.base_rate(expected);
    #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
    let share = kept as f32 / scored.len() as f32;
    PolicyRow {
        policy,
        kept,
        share,
        precision,
        lift: if base > 0.0 { precision / base } else { 0.0 },
    }
}

/// **Where the answer stops falling steeply** — the knee of the ranked
/// similarity curve, as its furthest point below the chord joining the two
/// ends of the search window.
///
/// A plain largest-gap rule cannot work here and the first sweep proved it: a
/// decaying curve's biggest single step is almost always at its head, so
/// largest-gap pinned every request to the smallest pool it was allowed. The
/// chord distance asks the question that was meant instead — where does this
/// curve bend — and is invariant to how steep the head happens to be.
fn elbow_cut(similarities: &[f32]) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a bounded share of a library count"
    )]
    let horizon = (((similarities.len() as f32) * ELBOW_HORIZON) as usize).max(ELBOW_FLOOR + 1);
    let horizon = horizon.min(similarities.len().saturating_sub(1));
    let (high, low) = (similarities[0], similarities[horizon]);
    let fall = high - low;
    if fall <= f32::EPSILON {
        return ELBOW_FLOOR.min(similarities.len());
    }
    let mut best = (ELBOW_FLOOR.min(similarities.len()), f32::MIN);
    for (index, similarity) in similarities
        .iter()
        .enumerate()
        .take(horizon)
        .skip(ELBOW_FLOOR)
    {
        #[expect(clippy::cast_precision_loss, reason = "bounded library counts")]
        let chord = high - fall * (index as f32 / horizon as f32);
        let below = chord - similarity;
        if below > best.1 {
            best = (index, below);
        }
    }
    best.0
}

fn distribution(sorted_descending: &[f32]) -> Distribution {
    let count = sorted_descending.len();
    let at = |quantile: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a bounded quantile of a library count"
        )]
        let index = (((count - 1) as f32) * (1.0 - quantile)) as usize;
        sorted_descending[index.min(count - 1)]
    };
    #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
    let mean = sorted_descending.iter().sum::<f32>() / count as f32;
    #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
    let deviation = (sorted_descending
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / count as f32)
        .sqrt();
    Distribution {
        minimum: sorted_descending[count - 1],
        p50: at(0.50),
        p90: at(0.90),
        p99: at(0.99),
        maximum: sorted_descending[0],
        mean,
        deviation,
    }
}

/// The per-policy summary the recommendation is actually read off: how many a
/// policy keeps, how much that count moves with the phrase, and how often it
/// keeps too few to fill a list.
fn summarise_policies(requests: &[RequestRow]) -> Vec<PolicyMean> {
    let mut names: Vec<String> = Vec::new();
    for request in requests {
        for policy in &request.policies {
            if !names.contains(&policy.policy) {
                names.push(policy.policy.clone());
            }
        }
    }
    names
        .into_iter()
        .map(|name| {
            let rows: Vec<&PolicyRow> = requests
                .iter()
                .filter_map(|request| request.policies.iter().find(|p| p.policy == name))
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
            let mean_kept = rows.iter().map(|row| row.kept as f32).sum::<f32>() / rows.len() as f32;
            #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
            let variation = (rows
                .iter()
                .map(|row| (row.kept as f32 - mean_kept).powi(2))
                .sum::<f32>()
                / rows.len() as f32)
                .sqrt()
                / mean_kept.max(1.0);
            let judged: Vec<_> = requests
                .iter()
                .filter(|request| request.judged)
                .filter_map(|request| request.policies.iter().find(|p| p.policy == name))
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
            let mean_lift = if judged.is_empty() {
                0.0
            } else {
                judged.iter().map(|row| row.lift).sum::<f32>() / judged.len() as f32
            };
            PolicyMean {
                policy: name,
                mean_kept,
                kept_variation: variation,
                smallest_kept: rows.iter().map(|row| row.kept).min().unwrap_or(0),
                largest_kept: rows.iter().map(|row| row.kept).max().unwrap_or(0),
                mean_lift,
                unfillable: rows.iter().filter(|row| row.kept < ELBOW_FLOOR).count(),
            }
        })
        .collect()
}

fn score_chip(corpus: &Corpus, candidate: &ChipCandidate) -> Result<ChipRow, Box<dyn Error>> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a bounded share of a library count"
    )]
    let size = (((corpus.tracks.len() as f32) * CHIP_SET) as usize).max(1);
    let own = top_set(corpus, candidate.word, size)?;
    let alone = corpus.against(&baz_vibe::embed_request(candidate.word)?);
    let mean = distribution(&sorted(&alone)).mean;
    let deviation = distribution(&sorted(&alone)).deviation;
    let best: Vec<f32> = sorted(&alone).into_iter().take(size).collect();
    #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
    let best_mean = best.iter().sum::<f32>() / best.len() as f32;
    let discrimination = if deviation > 0.0 {
        (best_mean - mean) / deviation
    } else {
        0.0
    };

    let mut pull = 0.0;
    let mut displacement = 0.0;
    for base in CHIP_BASES {
        let plain = top_set(corpus, base, size)?;
        let joined = top_set(corpus, &format!("{base}, {}", candidate.word), size)?;
        pull += overlap(&joined, &own) - overlap(&plain, &own);
        displacement += 1.0 - overlap(&joined, &plain);
    }
    #[expect(clippy::cast_precision_loss, reason = "five bases")]
    let bases = CHIP_BASES.len() as f32;

    let lift = (!candidate.expected.is_empty()).then(|| {
        let scored = alone;
        let mut ordered: Vec<&Track> = scored.iter().collect();
        ordered.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        let labelled: Vec<_> = ordered[..size]
            .iter()
            .filter_map(|track| track.genre.as_deref())
            .collect();
        #[expect(clippy::cast_precision_loss, reason = "library counts are small")]
        let precision = if labelled.is_empty() {
            0.0
        } else {
            labelled
                .iter()
                .filter(|genre| matches(genre, candidate.expected))
                .count() as f32
                / labelled.len() as f32
        };
        let base = corpus.base_rate(candidate.expected);
        if base > 0.0 { precision / base } else { 0.0 }
    });

    Ok(ChipRow {
        row: candidate.row.to_owned(),
        word: candidate.word.to_owned(),
        discrimination,
        pull: pull / bases,
        displacement: displacement / bases,
        lift,
    })
}

fn sorted(scored: &[Track]) -> Vec<f32> {
    let mut values: Vec<f32> = scored.iter().map(|track| track.similarity).collect();
    values.sort_by(|left, right| right.total_cmp(left));
    values
}

/// The indices of a prompt's best `size` tracks.
fn top_set(corpus: &Corpus, prompt: &str, size: usize) -> Result<Vec<usize>, Box<dyn Error>> {
    let embedding = baz_vibe::embed_request(prompt)?;
    let scored = corpus.against(&embedding);
    let mut order: Vec<usize> = (0..scored.len()).collect();
    order.sort_by(|left, right| {
        scored[*right]
            .similarity
            .total_cmp(&scored[*left].similarity)
    });
    order.truncate(size);
    Ok(order)
}

/// Jaccard overlap of two equally sized sets.
fn overlap(left: &[usize], right: &[usize]) -> f32 {
    let right: std::collections::HashSet<usize> = right.iter().copied().collect();
    let shared = left.iter().filter(|index| right.contains(index)).count();
    #[expect(clippy::cast_precision_loss, reason = "bounded set sizes")]
    {
        shared as f32 / (left.len() + right.len() - shared).max(1) as f32
    }
}
