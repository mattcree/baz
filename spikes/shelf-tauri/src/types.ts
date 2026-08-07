export interface Album {
  id: string;
  title: string;
  artist: string;
  year: number;
  tracks: string[];
}

export interface AlbumHit {
  id: string;
  title: string;
  artist: string;
  year: number;
}

/** A visible window of results — never the full result set. */
export interface SearchWindow {
  total: number;
  offset: number;
  items: AlbumHit[];
  /** Time the index itself spent, in microseconds. */
  index_us: number;
}

export interface SearchResponse extends SearchWindow {
  /** Full round-trip as seen by the caller (IPC or worker postMessage), ms. */
  rttMs: number;
}

export interface Backend {
  readonly mode: "tauri" | "browser";
  init(): Promise<{ albums: number; tracks: number }>;
  search(query: string, offset: number, limit: number): Promise<SearchResponse>;
  artUrl(id: string): string;
}

declare global {
  interface Window {
    __htmlStart: number;
  }
}
