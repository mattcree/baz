import {
  For,
  Show,
  batch,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { createBackend } from "./backend";
import { FpsMeter, KeystrokeStats, afterPaint } from "./metrics";
import type { AlbumHit } from "./types";

const CELL_W = 176; // px, incl. gap
const CELL_H = 224;
const OVERSCAN_ROWS = 3;

export default function App() {
  const backend = createBackend();
  const stats = new KeystrokeStats();
  const fpsMeter = new FpsMeter();

  const [query, setQuery] = createSignal("");
  const [total, setTotal] = createSignal(0);
  const [winOffset, setWinOffset] = createSignal(0);
  const [items, setItems] = createSignal<AlbumHit[]>([]);
  const [libInfo, setLibInfo] = createSignal("loading library…");
  const [report, setReport] = createSignal("");
  const [lastRtt, setLastRtt] = createSignal("");
  const [showFps, setShowFps] = createSignal(false);
  const [fps, setFps] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [viewport, setViewport] = createSignal({ w: 800, h: 600 });

  let scroller!: HTMLDivElement;
  let seq = 0; // stale-response guard

  const cols = createMemo(() => Math.max(1, Math.floor(viewport().w / CELL_W)));
  const rowCount = createMemo(() => Math.ceil(total() / cols()));
  const firstVisibleRow = createMemo(() => Math.floor(scrollTop() / CELL_H));
  const visibleRows = createMemo(() => Math.ceil(viewport().h / CELL_H) + 1);

  /** Indices the grid wants rendered right now (visible + overscan). */
  const renderRange = createMemo(() => {
    const start = Math.max(0, (firstVisibleRow() - OVERSCAN_ROWS) * cols());
    const end = Math.min(total(), (firstVisibleRow() + visibleRows() + OVERSCAN_ROWS) * cols());
    return { start, end };
  });

  const cells = createMemo(() => {
    const { start, end } = renderRange();
    const off = winOffset();
    const win = items();
    const c = cols();
    const out: { idx: number; item: AlbumHit | undefined; x: number; y: number }[] = [];
    for (let idx = start; idx < end; idx++) {
      out.push({
        idx,
        item: win[idx - off],
        x: (idx % c) * CELL_W,
        y: Math.floor(idx / c) * CELL_H,
      });
    }
    return out;
  });

  /** Fetch the window covering the render range (plus headroom), if we drifted out of it. */
  async function ensureWindow(force: boolean) {
    const { start, end } = renderRange();
    const off = winOffset();
    const have = items().length;
    if (!force && start >= off && end <= off + have) return;
    const fetchOffset = Math.max(0, (firstVisibleRow() - OVERSCAN_ROWS * 2) * cols());
    const fetchLimit = (visibleRows() + OVERSCAN_ROWS * 4) * cols();
    const mySeq = ++seq;
    const w = await backend.search(query(), fetchOffset, fetchLimit);
    if (mySeq !== seq) return; // stale
    batch(() => {
      setTotal(w.total);
      setWinOffset(w.offset);
      setItems(w.items);
    });
    setLastRtt(
      `${backend.mode === "tauri" ? "IPC" : "worker"} rtt ${w.rttMs.toFixed(2)}ms (index ${(w.index_us / 1000).toFixed(2)}ms)`,
    );
    if (backend.mode === "tauri") {
      console.log(`[ipc] q=${JSON.stringify(query())} rtt=${w.rttMs.toFixed(2)}ms index=${(w.index_us / 1000).toFixed(3)}ms`);
    }
  }

  async function onInput(value: string) {
    const t0 = performance.now();
    performance.mark("keystroke-input");
    setQuery(value);
    scroller.scrollTop = 0;
    setScrollTop(0);
    const mySeq = ++seq;
    const fetchLimit = (visibleRows() + OVERSCAN_ROWS * 4) * cols();
    const w = await backend.search(value, 0, fetchLimit);
    if (mySeq !== seq) return;
    performance.mark("keystroke-results");
    batch(() => {
      setTotal(w.total);
      setWinOffset(0);
      setItems(w.items);
    });
    setLastRtt(
      `${backend.mode === "tauri" ? "IPC" : "worker"} rtt ${w.rttMs.toFixed(2)}ms (index ${(w.index_us / 1000).toFixed(2)}ms)`,
    );
    if (backend.mode === "tauri") {
      console.log(`[ipc] q=${JSON.stringify(value)} rtt=${w.rttMs.toFixed(2)}ms index=${(w.index_us / 1000).toFixed(3)}ms`);
    }
    const painted = await afterPaint();
    performance.measure("keystroke-commit", "keystroke-input");
    stats.add({
      filterMs: w.rttMs,
      commitMs: painted - t0,
      indexMs: w.index_us / 1000,
      rttOverheadMs: w.rttMs - w.index_us / 1000,
    });
  }

  onMount(async () => {
    stats.onReport = setReport;
    fpsMeter.onUpdate = setFps;

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "F1") {
        e.preventDefault();
        setShowFps((v) => !v);
        if (showFps()) fpsMeter.start();
        else fpsMeter.stop();
      }
    };
    window.addEventListener("keydown", onKey);
    onCleanup(() => window.removeEventListener("keydown", onKey));

    const ro = new ResizeObserver(() => {
      setViewport({ w: scroller.clientWidth, h: scroller.clientHeight });
      void ensureWindow(false);
    });
    ro.observe(scroller);
    onCleanup(() => ro.disconnect());
    setViewport({ w: scroller.clientWidth, h: scroller.clientHeight });

    const info = await backend.init();
    setLibInfo(`${info.albums.toLocaleString()} albums / ${info.tracks.toLocaleString()} tracks [${backend.mode}]`);
    await ensureWindow(true);
    await afterPaint();
    const t = performance.now();
    console.log(
      `[startup] interactive at ${t.toFixed(0)}ms after navigation (${(t - window.__htmlStart).toFixed(0)}ms after index.html start) mode=${backend.mode}`,
    );
    setLibInfo((s) => `${s} — interactive in ${t.toFixed(0)}ms`);
  });

  let scrollScheduled = false;
  function onScroll() {
    if (scrollScheduled) return;
    scrollScheduled = true;
    requestAnimationFrame(() => {
      scrollScheduled = false;
      setScrollTop(scroller.scrollTop);
      void ensureWindow(false);
    });
  }

  return (
    <div class="app">
      <header>
        <input
          type="search"
          placeholder="Search 100,000 tracks… (try: artist 19, track 07, größenwahn)"
          onInput={(e) => void onInput(e.currentTarget.value)}
          autofocus
        />
        <span class="count">{total().toLocaleString()} albums</span>
      </header>
      <div class="shelf" ref={scroller} onScroll={onScroll}>
        <div class="spacer" style={{ height: `${rowCount() * CELL_H}px` }}>
          <For each={cells()}>
            {(cell) => (
              <div
                class="cell"
                style={{ transform: `translate(${cell.x}px, ${cell.y}px)` }}
              >
                <Show when={cell.item} fallback={<div class="art placeholder" />}>
                  {(item) => (
                    <>
                      <img
                        class="art"
                        src={backend.artUrl(item().id)}
                        loading="lazy"
                        decoding="async"
                        alt=""
                      />
                      <div class="meta">
                        <div class="title" title={item().title}>{item().title}</div>
                        <div class="artist">{item().artist} · {item().year}</div>
                      </div>
                    </>
                  )}
                </Show>
              </div>
            )}
          </For>
        </div>
      </div>
      <footer>
        <span>{libInfo()}</span>
        <span class="rtt">{lastRtt()}</span>
        <span class="report">{report()}</span>
        <span class="hint">F1: FPS overlay</span>
      </footer>
      <Show when={showFps()}>
        <div class="fps" classList={{ bad: fps() < 50 }}>{fps()} fps</div>
      </Show>
    </div>
  );
}
