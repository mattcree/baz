/// <reference lib="webworker" />
// Browser-mode search index: same window discipline as the Rust/IPC side —
// only the requested [offset, offset+limit) slice ever crosses postMessage.

import type { Album, AlbumHit, SearchWindow } from "../types";

interface InitMsg {
  kind: "init";
  reqId: number;
  url: string;
}
interface SearchMsg {
  kind: "search";
  reqId: number;
  query: string;
  offset: number;
  limit: number;
}
export type WorkerRequest = InitMsg | SearchMsg;
export type WorkerResponse =
  | { kind: "init"; reqId: number; albums: number; tracks: number }
  | ({ kind: "search"; reqId: number } & SearchWindow)
  | { kind: "error"; reqId: number; message: string };

let albums: Album[] = [];
let hay: string[] = [];
let trackCount = 0;

function buildIndex(text: string): void {
  albums = [];
  hay = [];
  trackCount = 0;
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    const a = JSON.parse(line) as Album;
    albums.push(a);
    trackCount += a.tracks.length;
    hay.push(`${a.title}\n${a.artist}\n${a.tracks.join("\n")}`.toLowerCase());
  }
}

function search(query: string, offset: number, limit: number): SearchWindow {
  const t0 = performance.now();
  const q = query.trim().toLowerCase();
  const items: AlbumHit[] = [];
  let total = 0;
  if (q === "") {
    total = albums.length;
    for (let i = offset; i < Math.min(albums.length, offset + limit); i++) {
      const { id, title, artist, year } = albums[i];
      items.push({ id, title, artist, year });
    }
  } else {
    for (let i = 0; i < hay.length; i++) {
      if (hay[i].includes(q)) {
        if (total >= offset && items.length < limit) {
          const { id, title, artist, year } = albums[i];
          items.push({ id, title, artist, year });
        }
        total++;
      }
    }
  }
  return { total, offset, items, index_us: Math.round((performance.now() - t0) * 1000) };
}

self.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const msg = ev.data;
  try {
    if (msg.kind === "init") {
      const res = await fetch(msg.url);
      if (!res.ok) throw new Error(`fetch ${msg.url}: HTTP ${res.status}`);
      buildIndex(await res.text());
      post({ kind: "init", reqId: msg.reqId, albums: albums.length, tracks: trackCount });
    } else {
      post({ kind: "search", reqId: msg.reqId, ...search(msg.query, msg.offset, msg.limit) });
    }
  } catch (e) {
    post({ kind: "error", reqId: msg.reqId, message: e instanceof Error ? e.message : String(e) });
  }
};

function post(r: WorkerResponse): void {
  (self as unknown as Worker).postMessage(r);
}
