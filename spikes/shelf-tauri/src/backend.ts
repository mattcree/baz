import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { Backend, SearchResponse, SearchWindow } from "./types";
import type { WorkerResponse } from "./worker/search.worker";

export const isTauri = "__TAURI_INTERNALS__" in window;

/** Tauri mode: index lives in Rust; only the visible window crosses IPC. */
class TauriBackend implements Backend {
  readonly mode = "tauri" as const;

  async init() {
    return await invoke<{ albums: number; tracks: number }>("stats");
  }

  async search(query: string, offset: number, limit: number): Promise<SearchResponse> {
    const t0 = performance.now();
    const w = await invoke<SearchWindow>("search", { query, offset, limit });
    return { ...w, rttMs: performance.now() - t0 };
  }

  artUrl(id: string): string {
    // Custom asset protocol — art bytes never travel over IPC as base64.
    return convertFileSrc(`${id}.png`, "shelfart");
  }
}

/** Browser mode: index lives in a Web Worker; same window discipline. */
class WorkerBackend implements Backend {
  readonly mode = "browser" as const;
  private worker = new Worker(new URL("./worker/search.worker.ts", import.meta.url), {
    type: "module",
  });
  private nextId = 1;
  private pending = new Map<
    number,
    { resolve: (r: WorkerResponse) => void; reject: (e: Error) => void; t0: number }
  >();

  constructor() {
    this.worker.onmessage = (ev: MessageEvent<WorkerResponse>) => {
      const p = this.pending.get(ev.data.reqId);
      if (!p) return;
      this.pending.delete(ev.data.reqId);
      if (ev.data.kind === "error") p.reject(new Error(ev.data.message));
      else p.resolve(ev.data);
    };
  }

  private request(msg: object): Promise<{ resp: WorkerResponse; rttMs: number }> {
    const reqId = this.nextId++;
    const t0 = performance.now();
    return new Promise((resolve, reject) => {
      this.pending.set(reqId, {
        resolve: (resp) => resolve({ resp, rttMs: performance.now() - t0 }),
        reject,
        t0,
      });
      this.worker.postMessage({ ...msg, reqId });
    });
  }

  async init() {
    const { resp } = await this.request({ kind: "init", url: "/dataset/albums.jsonl" });
    if (resp.kind !== "init") throw new Error("bad init response");
    return { albums: resp.albums, tracks: resp.tracks };
  }

  async search(query: string, offset: number, limit: number): Promise<SearchResponse> {
    const { resp, rttMs } = await this.request({ kind: "search", query, offset, limit });
    if (resp.kind !== "search") throw new Error("bad search response");
    const { total, items, index_us } = resp;
    return { total, offset: resp.offset, items, index_us, rttMs };
  }

  artUrl(id: string): string {
    return `/dataset/art/${id}.png`;
  }
}

export function createBackend(): Backend {
  return isTauri ? new TauriBackend() : new WorkerBackend();
}
