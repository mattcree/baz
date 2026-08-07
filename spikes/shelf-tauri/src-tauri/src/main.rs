#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use shelf_index::{Index, SearchWindow};
use std::borrow::Cow;
use std::path::PathBuf;
use tauri::http;

struct AppState {
    index: Index,
}

#[derive(serde::Serialize)]
struct Stats {
    albums: usize,
    tracks: usize,
}

#[tauri::command]
fn stats(state: tauri::State<AppState>) -> Stats {
    Stats {
        albums: state.index.album_count(),
        tracks: state.index.track_count(),
    }
}

/// The only search entry point. Returns ONLY the requested visible window —
/// the full library never crosses IPC.
#[tauri::command]
fn search(state: tauri::State<AppState>, query: String, offset: usize, limit: usize) -> SearchWindow {
    state.index.search(&query, offset, limit.min(1_000))
}

/// Locate ./dataset relative to where the app runs (tauri dev runs in
/// src-tauri, a packaged binary elsewhere). Override with BAZ_DATASET.
fn find_dataset() -> PathBuf {
    if let Ok(p) = std::env::var("BAZ_DATASET") {
        return PathBuf::from(p);
    }
    for cand in ["dataset", "../dataset", "../../dataset"] {
        let p = PathBuf::from(cand);
        if p.join("albums.jsonl").is_file() {
            return p;
        }
    }
    panic!("dataset not found — run `cargo run --release --features gen --bin gen-dataset` in the spike root, or set BAZ_DATASET");
}

fn main() {
    let dataset = find_dataset();
    let t0 = std::time::Instant::now();
    let index = Index::from_jsonl_path(&dataset.join("albums.jsonl")).expect("load albums.jsonl");
    eprintln!(
        "[rust] indexed {} albums / {} tracks in {:?}",
        index.album_count(),
        index.track_count(),
        t0.elapsed()
    );
    let art_dir = dataset.join("art").canonicalize().expect("canonicalize art dir");

    tauri::Builder::default()
        .manage(AppState { index })
        // Album art is served over a custom protocol (shelfart://localhost/{id}.png
        // on Linux/macOS, http://shelfart.localhost/{id}.png on Windows).
        // Bytes stream through the webview's network stack — never base64 over IPC.
        .register_uri_scheme_protocol("shelfart", move |_ctx, request| {
            let name = request.uri().path().trim_start_matches('/');
            let ok = name.ends_with(".png")
                && name.len() <= 64
                && name.trim_end_matches(".png").bytes().all(|b| b.is_ascii_digit());
            if ok {
                if let Ok(bytes) = std::fs::read(art_dir.join(name)) {
                    return http::Response::builder()
                        .status(200)
                        .header("Content-Type", "image/png")
                        .header("Cache-Control", "max-age=3600")
                        .body(Cow::<'static, [u8]>::Owned(bytes))
                        .unwrap();
                }
            }
            http::Response::builder()
                .status(404)
                .body(Cow::Borrowed(&[][..]))
                .unwrap()
        })
        .invoke_handler(tauri::generate_handler![stats, search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
