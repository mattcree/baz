//! Spike A (iced): virtualized 10k-album shelf with lazy art thumbnails and
//! search-as-you-type over 100k tracks. Instrumented; see RESULTS.md.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::keyboard::{self, key};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{
    column, container, image as iced_image, row, scrollable, stack, text, text_input, Space,
};
use iced::{window, Color, Element, Length, Size, Subscription, Task, Theme};
use lru::LruCache;
use shelf_iced::{dataset_dir, rss_mib, Index};

// Grid geometry (logical pixels).
const CELL_W: f32 = 178.0;
const CELL_H: f32 = 232.0;
const ART_PX: f32 = 160.0;
const GRID_PADDING: f32 = 12.0;
const THUMB_DECODE_PX: u32 = 256; // downscale target for GPU memory sanity
const LRU_CAPACITY: usize = 800; // ~800 * 256*256*4 B = ~200 MiB worst case
const OVERSCAN_ROWS: usize = 2;

fn scroll_id() -> scrollable::Id {
    scrollable::Id::new("shelf")
}

struct App {
    index: Arc<Index>,
    results: Vec<u32>, // indices into index.albums
    query: String,

    thumbs: LruCache<u32, iced_image::Handle>,
    pending_thumbs: HashSet<u32>,

    scroll_offset: f32,
    grid_size: Size, // viewport size of the scrollable area

    // Instrumentation
    started: Instant,
    first_frame: Option<Duration>,
    pending_commit: Option<(Instant, f64)>, // (keystroke instant, filter ms)
    frame_times: VecDeque<Instant>,
    show_fps: bool,
    fps: usize,
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    Scrolled(Viewport),
    WindowResized(Size),
    ThumbLoaded(u32, Option<(u32, u32, Vec<u8>)>),
    Frame(Instant),
    RssTick,
    ToggleFps,
}

fn main() -> iced::Result {
    let started = Instant::now();
    let jsonl = dataset_dir().join("albums.jsonl");
    let index = Index::load(&jsonl).unwrap_or_else(|e| {
        eprintln!(
            "failed to load {}: {e}\nrun `cargo run --release --bin gen_dataset` first",
            jsonl.display()
        );
        std::process::exit(1);
    });
    println!(
        "[startup] jsonl load (index hydration): {:.1} ms, case-fold index build: {:.1} ms ({} albums / {} tracks)",
        index.load_time_ms,
        index.index_time_ms,
        index.albums.len(),
        index.albums.len() * 10
    );
    if let Some(rss) = rss_mib() {
        println!("[rss] after hydration: {rss:.1} MiB");
    }

    iced::application("baz spike — iced shelf", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| Theme::Dark)
        .window_size(Size::new(1280.0, 860.0))
        .run_with(move || {
            let mut app = App {
                results: (0..index.albums.len() as u32).collect(),
                index: Arc::new(index),
                query: String::new(),
                thumbs: LruCache::new(NonZeroUsize::new(LRU_CAPACITY).unwrap()),
                pending_thumbs: HashSet::new(),
                scroll_offset: 0.0,
                grid_size: Size::new(1280.0, 800.0),
                started,
                first_frame: None,
                pending_commit: None,
                frame_times: VecDeque::new(),
                show_fps: false,
                fps: 0,
            };
            let task = app.request_visible_thumbs();
            (app, task)
        })
}

impl App {
    fn columns(&self) -> usize {
        (((self.grid_size.width - 2.0 * GRID_PADDING) / CELL_W).floor() as usize).max(1)
    }

    fn visible_range(&self) -> (usize, usize) {
        let cols = self.columns();
        let total_rows = self.results.len().div_ceil(cols);
        let first_row = ((self.scroll_offset / CELL_H).floor() as usize)
            .saturating_sub(OVERSCAN_ROWS)
            .min(total_rows);
        let rows_on_screen = (self.grid_size.height / CELL_H).ceil() as usize + 1 + OVERSCAN_ROWS;
        let end_row = (first_row + rows_on_screen + OVERSCAN_ROWS).min(total_rows);
        (first_row, end_row)
    }

    fn request_visible_thumbs(&mut self) -> Task<Message> {
        let cols = self.columns();
        let (first_row, end_row) = self.visible_range();
        let start = first_row * cols;
        let end = (end_row * cols).min(self.results.len());
        let mut tasks = Vec::new();
        for &album_idx in &self.results[start..end] {
            let id = self.index.albums[album_idx as usize].id;
            // `get` (not `contains`) so visibility refreshes LRU recency.
            if self.thumbs.get(&id).is_some() || self.pending_thumbs.contains(&id) {
                continue;
            }
            self.pending_thumbs.insert(id);
            let path = dataset_dir().join("art").join(format!("{id}.png"));
            tasks.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let img = image::open(&path).ok()?;
                        let thumb = img.thumbnail(THUMB_DECODE_PX, THUMB_DECODE_PX).into_rgba8();
                        let (w, h) = thumb.dimensions();
                        Some((w, h, thumb.into_raw()))
                    })
                    .await
                    .ok()
                    .flatten()
                },
                move |result| Message::ThumbLoaded(id, result),
            ));
        }
        Task::batch(tasks)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SearchChanged(q) => {
                let t0 = Instant::now();
                self.results = self.index.filter(&q);
                let filter_ms = t0.elapsed().as_secs_f64() * 1e3;
                println!(
                    "[search] {:?}: filter {:.2} ms, {} albums match",
                    q,
                    filter_ms,
                    self.results.len()
                );
                self.query = q;
                self.pending_commit = Some((t0, filter_ms));
                self.scroll_offset = 0.0;
                Task::batch([
                    scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                    self.request_visible_thumbs(),
                ])
            }
            Message::Scrolled(viewport) => {
                self.scroll_offset = viewport.absolute_offset().y;
                let b = viewport.bounds();
                self.grid_size = Size::new(b.width, b.height);
                self.request_visible_thumbs()
            }
            Message::WindowResized(size) => {
                // Approximation: grid viewport is the window minus the search bar.
                self.grid_size = Size::new(size.width, (size.height - 60.0).max(100.0));
                self.request_visible_thumbs()
            }
            Message::ThumbLoaded(id, result) => {
                self.pending_thumbs.remove(&id);
                if let Some((w, h, rgba)) = result {
                    self.thumbs
                        .put(id, iced_image::Handle::from_rgba(w, h, rgba));
                }
                Task::none()
            }
            Message::Frame(now) => {
                if self.first_frame.is_none() {
                    let t = self.started.elapsed();
                    self.first_frame = Some(t);
                    println!(
                        "[startup] startup-to-first-frame: {:.1} ms (interactive: index was hydrated pre-window, so this is also startup-to-interactive)",
                        t.as_secs_f64() * 1e3
                    );
                }
                if let Some((t0, filter_ms)) = self.pending_commit.take() {
                    println!(
                        "[search] time-to-view-commit: {:.2} ms (incl. filter {:.2} ms)",
                        t0.elapsed().as_secs_f64() * 1e3,
                        filter_ms
                    );
                }
                self.frame_times.push_back(now);
                while let Some(front) = self.frame_times.front() {
                    if now.duration_since(*front) > Duration::from_secs(1) {
                        self.frame_times.pop_front();
                    } else {
                        break;
                    }
                }
                self.fps = self.frame_times.len();
                Task::none()
            }
            Message::RssTick => {
                if let Some(rss) = rss_mib() {
                    println!(
                        "[rss] {rss:.1} MiB (thumb cache: {} decoded)",
                        self.thumbs.len()
                    );
                }
                Task::none()
            }
            Message::ToggleFps => {
                self.show_fps = !self.show_fps;
                self.frame_times.clear();
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard::on_key_press(|k, _mods| match k {
                keyboard::Key::Named(key::Named::F1) => Some(Message::ToggleFps),
                _ => None,
            }),
            iced::time::every(Duration::from_secs(5)).map(|_| Message::RssTick),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
        ];
        // Continuous frame events only when needed: before the first frame is
        // recorded, while a keystroke's view-commit is being timed, or while
        // the FPS overlay is up (it needs a steady sample stream).
        if self.first_frame.is_none() || self.pending_commit.is_some() || self.show_fps {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        let cols = self.columns();
        let total_rows = self.results.len().div_ceil(cols);
        let (first_row, end_row) = self.visible_range();

        let mut grid = column![].spacing(0.0).padding(GRID_PADDING);
        // Top spacer standing in for all rows above the viewport.
        grid = grid.push(Space::new(
            Length::Fill,
            Length::Fixed(first_row as f32 * CELL_H),
        ));
        for r in first_row..end_row {
            let mut cells = row![].spacing(0.0);
            for c in 0..cols {
                let slot = r * cols + c;
                if slot >= self.results.len() {
                    break;
                }
                cells = cells.push(self.cell(self.results[slot]));
            }
            grid = grid.push(container(cells).height(Length::Fixed(CELL_H)));
        }
        // Bottom spacer for all rows below.
        grid = grid.push(Space::new(
            Length::Fill,
            Length::Fixed((total_rows - end_row) as f32 * CELL_H),
        ));

        let shelf = scrollable(grid)
            .id(scroll_id())
            .on_scroll(Message::Scrolled)
            .width(Length::Fill)
            .height(Length::Fill);

        let status = text(format!(
            "{} / {} albums — F1: FPS overlay",
            self.results.len(),
            self.index.albums.len()
        ))
        .size(13);

        let top_bar = row![
            text_input("Search artists, albums, tracks…", &self.query)
                .on_input(Message::SearchChanged)
                .padding(10)
                .size(16),
            container(status).padding(10),
        ]
        .spacing(8)
        .padding(8);

        let content = column![top_bar, shelf];

        if self.show_fps {
            let overlay = container(
                text(format!("{} fps", self.fps))
                    .size(22)
                    .color(Color::from_rgb(0.2, 1.0, 0.4)),
            )
            .padding(10)
            .align_right(Length::Fill);
            stack![content, overlay].into()
        } else {
            content.into()
        }
    }

    fn cell(&self, album_idx: u32) -> Element<'_, Message> {
        let album = &self.index.albums[album_idx as usize];
        let art: Element<'_, Message> = match self.thumbs.peek(&album.id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(ART_PX))
                .height(Length::Fixed(ART_PX))
                .into(),
            None => container(text("…").size(30))
                .width(Length::Fixed(ART_PX))
                .height(Length::Fixed(ART_PX))
                .center_x(Length::Fixed(ART_PX))
                .center_y(Length::Fixed(ART_PX))
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.15, 0.15, 0.18).into()),
                    ..Default::default()
                })
                .into(),
        };
        container(
            column![
                art,
                text(album.title.as_str()).size(13),
                text(format!("{} · {}", album.artist, album.year)).size(12),
            ]
            .spacing(3)
            .width(Length::Fixed(ART_PX)),
        )
        .width(Length::Fixed(CELL_W))
        .height(Length::Fixed(CELL_H))
        .padding(6)
        .into()
    }
}
