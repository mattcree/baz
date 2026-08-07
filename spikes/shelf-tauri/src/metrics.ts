// Instrumentation: keystroke latency percentiles, FPS meter, startup timing.

export interface KeystrokeSample {
  /** Backend round-trip (search) time, ms. */
  filterMs: number;
  /** Input event → next painted frame after results committed, ms. */
  commitMs: number;
  /** Index-only time reported by the backend, ms. */
  indexMs: number;
  /** RTT overhead = filterMs - indexMs (IPC/worker serialization cost), ms. */
  rttOverheadMs: number;
}

export function pct(sorted: number[], p: number): number {
  if (sorted.length === 0) return NaN;
  return sorted[Math.round((p / 100) * (sorted.length - 1))];
}

function fmt(v: number): string {
  return `${v.toFixed(2)}ms`;
}

export class KeystrokeStats {
  private samples: KeystrokeSample[] = [];
  private idleTimer: ReturnType<typeof setTimeout> | undefined;
  onReport: ((report: string) => void) | undefined;

  add(s: KeystrokeSample): void {
    this.samples.push(s);
    performance.mark("keystroke-committed");
    clearTimeout(this.idleTimer);
    // Report after the user pauses typing for a moment.
    this.idleTimer = setTimeout(() => this.report(), 900);
  }

  report(): void {
    if (this.samples.length === 0) return;
    const by = (k: keyof KeystrokeSample) =>
      this.samples.map((s) => s[k]).sort((a, b) => a - b);
    const filter = by("filterMs");
    const commit = by("commitMs");
    const index = by("indexMs");
    const overhead = by("rttOverheadMs");
    const line =
      `[keystrokes n=${this.samples.length}] ` +
      `filter p50=${fmt(pct(filter, 50))} p95=${fmt(pct(filter, 95))} | ` +
      `commit p50=${fmt(pct(commit, 50))} p95=${fmt(pct(commit, 95))} | ` +
      `index p50=${fmt(pct(index, 50))} | ` +
      `ipc/worker overhead p50=${fmt(pct(overhead, 50))} p95=${fmt(pct(overhead, 95))}`;
    console.log(line);
    this.onReport?.(line);
    this.samples = [];
  }
}

/** rAF-based FPS meter. Call start() once; read fps in the overlay. */
export class FpsMeter {
  fps = 0;
  private frames: number[] = [];
  private running = false;
  onUpdate: ((fps: number) => void) | undefined;

  start(): void {
    if (this.running) return;
    this.running = true;
    const tick = (t: number) => {
      if (!this.running) return;
      this.frames.push(t);
      const cutoff = t - 1000;
      while (this.frames.length && this.frames[0] < cutoff) this.frames.shift();
      this.fps = this.frames.length;
      this.onUpdate?.(this.fps);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  stop(): void {
    this.running = false;
    this.frames = [];
  }
}

/** Resolves after the next frame has painted (double rAF). */
export function afterPaint(): Promise<number> {
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now()))),
  );
}
