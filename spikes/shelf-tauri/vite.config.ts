import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));

/** Serves ./dataset (albums.jsonl + art PNGs) in both `vite dev` and `vite preview`. */
function datasetServer(): Plugin {
  const root = path.resolve(here, "dataset");
  const handler = (
    req: { url?: string },
    res: import("node:http").ServerResponse,
    next: () => void,
  ) => {
    if (!req.url || !req.url.startsWith("/dataset/")) return next();
    const rel = decodeURIComponent(req.url.split("?")[0].slice("/dataset/".length));
    const file = path.normalize(path.join(root, rel));
    if (!file.startsWith(root) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.statusCode = 404;
      res.end("not found");
      return;
    }
    res.setHeader(
      "Content-Type",
      file.endsWith(".png") ? "image/png" : "application/x-ndjson; charset=utf-8",
    );
    res.setHeader("Cache-Control", "max-age=3600");
    fs.createReadStream(file).pipe(res);
  };
  return {
    name: "dataset-server",
    configureServer(server) {
      server.middlewares.use(handler);
    },
    configurePreviewServer(server) {
      server.middlewares.use(handler);
    },
  };
}

export default defineConfig({
  plugins: [solid(), datasetServer()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022" },
  worker: { format: "es" },
});
