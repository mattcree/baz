//! **Can a library tell us which instrument words are worth offering it?**
//!
//! No — and this bin is the receipt for that, kept because a negative result
//! nobody can reproduce is a rumour.
//!
//! The idea was the sharpest instance of *"drive the app from their own
//! library"*: the vocabulary chips were chosen by measuring candidates
//! against **one** collection and then hardcoded for everybody, so somebody
//! who owns no guitars is offered `electric guitars`. The fix looked obvious
//! — run the measurement locally.
//!
//! Three methods, three failures, on a real 5 076-track library that is about
//! a third rock and holds 576 electronic tracks:
//!
//! 1. **Distribution shape** — how far a word's best matches sit above its own
//!    median, in its own spread. `bagpipes` 3.22 beat `electric guitars` 2.85.
//!    It measures how confident the model's ordering is, not what the library
//!    contains.
//! 2. **Per-track argmax across words.** `steel drums` claimed 18% of the
//!    library and `banjo` 10%, while `synthesizers` took 1.3%. It measures
//!    each word's own offset into the audio manifold.
//! 3. **Z-score each word against the library, then compete.** The principled
//!    fix for (2), and it flattened towards uniform: `banjo` 9.9%,
//!    `didgeridoo` 5.3%, `gamelan` 4.3%, against `electric guitars` at 1.7%.
//!
//! The common cause is already on the record in
//! `docs/design/impl/vibe-eligibility/`: **CLAP text-audio similarities are
//! not comparable across different prompts.** Every method above needs them to
//! be, in one form or another.
//!
//! **What this does not overturn.** Instrument words still measurably *steer*
//! a request — `acoustic guitar` 0.142 pull, `piano`/`synthesizers`/`strings`
//! concentrating the matching genre 3.5–4.1×. That question is asked *within*
//! one word and judged against external labels, which is well-founded. This
//! one asks *across* words with no labels at all, and cannot be.
use rusqlite::Connection;
use std::error::Error;
use std::path::Path;

const WORDS: [&str; 24] = [
    "electric guitars",
    "acoustic guitar",
    "piano",
    "synthesizers",
    "strings",
    "brass",
    "female vocals",
    "male vocals",
    "no vocals",
    "drum machine",
    "orchestra",
    "choir",
    "saxophone",
    "banjo",
    "bagpipes",
    "sitar",
    "steel drums",
    "accordion",
    "harpsichord",
    "throat singing",
    "didgeridoo",
    "gamelan",
    "distorted guitars",
    "double bass",
];

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    let connection = Connection::open(Path::new(&arguments[1]))?;
    let mut statement = connection.prepare(
        "SELECT values_blob, semantic_blob FROM features WHERE semantic_blob IS NOT NULL",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut candidates = Vec::new();
    for (index, row) in rows.enumerate() {
        let (values, semantic) = row?;
        candidates.push(baz_vibe::Candidate::from_parts(
            std::path::PathBuf::from(format!("/{index}")),
            index as u64,
            String::new(),
            &values,
            &semantic,
        ));
    }
    eprintln!("{} tracks", candidates.len());
    // **Words compete per track**, which is the fix for what the first
    // attempt measured. Scoring each word's distribution on its own says how
    // peaked the model's opinion is, not whether the library holds any of the
    // thing — which is why `bagpipes` outscored `electric guitars` on a
    // library that is a third rock. Comparing words *against each other, on
    // the same track*, cancels the per-word offset: whatever a word's
    // absolute similarities look like, only one word can be a given track's
    // best answer.
    let requests: Vec<(String, Vec<f32>)> = WORDS
        .iter()
        .map(|word| Ok(((*word).to_owned(), baz_vibe::embed_request(word)?)))
        .collect::<Result<_, Box<dyn Error>>>()?;
    // **Normalise each word against the library before comparing words.**
    //
    // Neither earlier attempt worked. Distribution shape measured the model's
    // confidence, not the library's contents (`bagpipes` beat `electric
    // guitars`). Raw per-track argmax measured each word's own offset into the
    // audio manifold (`steel drums` claimed 18% of a rock library). Both fail
    // for the reason the eligibility sweep already recorded: **CLAP
    // text-audio similarities are not comparable across different prompts.**
    //
    // So each word is z-scored against its *own* distribution over this
    // library first — subtracting its mean and dividing by its spread — which
    // removes exactly the per-word offset and scale that were doing the
    // damage. Only then do words compete for a track.
    let z: Vec<Vec<f32>> = requests
        .iter()
        .map(|(_, request)| {
            let raw: Vec<f32> = candidates
                .iter()
                .map(|candidate| candidate.features.similarity(request))
                .collect();
            #[expect(
                clippy::cast_precision_loss,
                reason = "a library count, as the divisor of a mean"
            )]
            let count = raw.len() as f32;
            let mean = raw.iter().sum::<f32>() / count;
            let deviation = (raw.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / count).sqrt();
            raw.iter()
                .map(|v| {
                    if deviation > f32::EPSILON {
                        (v - mean) / deviation
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    let mut wins = vec![0_usize; WORDS.len()];
    for track in 0..candidates.len() {
        let mut best = (0, f32::MIN);
        for (word, scores) in z.iter().enumerate() {
            if scores[track] > best.1 {
                best = (word, scores[track]);
            }
        }
        wins[best.0] += 1;
    }
    let mut ranked: Vec<(usize, &str)> = wins.iter().copied().zip(WORDS.iter().copied()).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    let total = candidates.len();
    for (count, word) in ranked {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a percentage of a library count, printed to one decimal"
        )]
        let (share, bar) = {
            let share = 100.0 * count as f32 / total as f32;
            (share, "#".repeat((share / 2.0).round() as usize))
        };
        println!("{count:6}  {share:5.1}%  {word:<20} {bar}");
    }
    Ok(())
}
