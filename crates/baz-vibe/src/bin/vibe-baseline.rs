//! Export the shipped conventional-feature comparator for blind vibe tests.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use baz_vibe::{Candidate, Features, Profile, analyze_and_store, prepare, select};
use serde::{Deserialize, Serialize};

const SCHEMA: u8 = 1;

#[derive(Deserialize)]
struct Corpus {
    schema: u8,
    tracks: Vec<Track>,
}

#[derive(Deserialize)]
struct Track {
    id: String,
    path: PathBuf,
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    genre: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Request {
    id: String,
    kind: String,
    query: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    avoid: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arc: Vec<Phase>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Phase {
    at: f32,
    query: String,
}

#[derive(Deserialize)]
struct Requests {
    schema: u8,
    requests: Vec<Request>,
}

#[derive(Serialize)]
struct Run {
    schema: u8,
    system: &'static str,
    corpus_ids: Vec<String>,
    corpus_fingerprint: String,
    analysis_seconds: f64,
    results: Vec<ResultRow>,
}

#[derive(Serialize)]
struct ResultRow {
    id: String,
    kind: String,
    request: Request,
    ranking: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("vibe baseline: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() != 6 {
        return Err("usage: vibe-baseline CORPUS REQUESTS CACHE OUTPUT LIMIT".into());
    }
    let corpus: Corpus = read_json(Path::new(&arguments[1]))?;
    let requests: Requests = read_json(Path::new(&arguments[2]))?;
    if corpus.schema != SCHEMA || requests.schema != SCHEMA {
        return Err(format!("expected schema {SCHEMA}").into());
    }
    let cache = Path::new(&arguments[3]);
    let output = Path::new(&arguments[4]);
    let limit: usize = arguments[5].to_string_lossy().parse()?;
    let started = Instant::now();
    let features = analyze_corpus(&corpus, cache)?;
    let candidates: Vec<_> = corpus
        .tracks
        .iter()
        .filter_map(|track| {
            features
                .get(&track.path)
                .cloned()
                .map(|features| Candidate {
                    path: track.path.clone(),
                    album: stable_id(&track.album),
                    artist: track.artist.clone(),
                    features,
                })
        })
        .collect();
    let path_to_id: HashMap<_, _> = corpus
        .tracks
        .iter()
        .map(|track| (&track.path, track.id.as_str()))
        .collect();
    let results = requests
        .requests
        .into_iter()
        .map(|request| {
            let ranking = rank(&request, &candidates, limit)
                .into_iter()
                .filter_map(|path| path_to_id.get(&path).copied().map(str::to_owned))
                .collect();
            ResultRow {
                id: request.id.clone(),
                kind: request.kind.clone(),
                request,
                ranking,
            }
        })
        .collect();
    let run = Run {
        schema: SCHEMA,
        system: "conventional-bliss-v1",
        corpus_ids: corpus.tracks.iter().map(|track| track.id.clone()).collect(),
        corpus_fingerprint: corpus_fingerprint(&corpus.tracks),
        analysis_seconds: started.elapsed().as_secs_f64(),
        results,
    };
    std::fs::write(output, serde_json::to_string_pretty(&run)? + "\n")?;
    Ok(())
}

fn corpus_fingerprint(tracks: &[Track]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let mut absorb = |text: &str| {
        for byte in (text.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(text.bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    for track in tracks {
        let path = track.path.to_string_lossy();
        for text in [
            track.id.as_str(),
            path.as_ref(),
            track.title.as_str(),
            track.artist.as_str(),
            track.album.as_str(),
            track.genre.as_str(),
        ] {
            absorb(text);
        }
        absorb(&track.tags.len().to_string());
        for tag in &track.tags {
            absorb(tag);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn analyze_corpus(
    corpus: &Corpus,
    cache: &Path,
) -> Result<HashMap<PathBuf, Features>, Box<dyn Error>> {
    let paths: Vec<_> = corpus
        .tracks
        .iter()
        .map(|track| track.path.clone())
        .collect();
    let prepared = prepare(cache, paths)?;
    let mut features = prepared.ready;
    for (number, path) in prepared.pending.into_iter().enumerate() {
        eprintln!("analyse {} · {}", number + 1, path.display());
        let analyzed = analyze_and_store(cache, path)?;
        features.insert(analyzed.path, analyzed.features);
    }
    Ok(features)
}

fn rank(request: &Request, candidates: &[Candidate], limit: usize) -> Vec<PathBuf> {
    if request.arc.is_empty() {
        return select(&profile(&request.query, &request.avoid), candidates, limit).paths;
    }
    let per_phase = limit.div_ceil(request.arc.len());
    let mut available = candidates.to_vec();
    let mut paths = Vec::with_capacity(limit);
    let mut used = HashSet::new();
    for phase in &request.arc {
        let selected = select(
            &profile(&phase.query, &request.avoid),
            &available,
            per_phase,
        );
        for path in selected.paths {
            if paths.len() == limit {
                break;
            }
            if used.insert(path.clone()) {
                paths.push(path);
            }
        }
        available.retain(|candidate| !used.contains(&candidate.path));
    }
    paths
}

fn profile(query: &str, avoid: &str) -> Profile {
    let mut energy = axis(
        query,
        &[
            "energetic",
            "aggressive",
            "driving",
            "euphoric",
            "fast",
            "intense",
            "peak",
            "danceable",
        ],
        &[
            "calm",
            "gentle",
            "slow",
            "sparse",
            "restrained",
            "focus",
            "ambient",
            "weightless",
        ],
    ) - axis(
        avoid,
        &[
            "energetic",
            "aggressive",
            "driving",
            "euphoric",
            "fast",
            "intense",
            "loud",
        ],
        &["calm", "gentle", "slow", "sparse", "restrained", "quiet"],
    );
    let brightness = axis(
        query,
        &["bright", "crisp", "open", "sunny", "cold"],
        &["warm", "dark", "soft", "intimate", "nocturnal"],
    ) - axis(
        avoid,
        &["bright", "crisp", "open", "sunny", "cold"],
        &["warm", "dark", "soft", "intimate", "nocturnal"],
    );
    if energy == 0 && brightness == 0 {
        energy = -1;
    }
    Profile {
        energy: energy.clamp(-2, 2),
        brightness: brightness.clamp(-2, 2),
        seed: None,
    }
}

fn axis(text: &str, high: &[&str], low: &[&str]) -> i8 {
    let words: HashSet<_> = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let high = high.iter().filter(|word| words.contains(**word)).count();
    let low = low.iter().filter(|word| words.contains(**word)).count();
    high.cmp(&low) as i8
}

fn stable_id(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_axes_are_explicit_and_negative_guidance_opposes_them() {
        assert_eq!(profile("bright energetic rock", "").energy, 1);
        assert_eq!(profile("bright energetic rock", "").brightness, 1);
        assert!(profile("dark tense music", "loud aggressive music").energy < 0);
    }

    #[test]
    fn stable_ids_are_stable_and_distinct() {
        assert_eq!(stable_id("album"), stable_id("album"));
        assert_ne!(stable_id("album"), stable_id("other"));
    }

    #[test]
    fn corpus_fingerprint_is_portable() {
        let tracks = [Track {
            id: "one".into(),
            path: PathBuf::from("/music/one.flac"),
            title: "One".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            genre: "Jazz".into(),
            tags: vec!["warm".into(), "live".into()],
        }];
        assert_eq!(corpus_fingerprint(&tracks), "fnv1a64:80b1207e4f81faab");
    }
}
