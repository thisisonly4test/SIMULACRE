// Stand-in for the Cloudflare deployment: serves web/ as static assets with
// the headers from web/_headers, relays /api/face to an offline face source,
// and answers 404 on every other /api route (as worker/index.js does).
import http from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { png } from "./mock-faces.mjs";

const root = resolve("web");
const port = Number(process.argv[2] || 7990);
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".wasm": "application/wasm", ".json": "application/json" };

// Only the "/*" block of _headers is used by the site.
const headers = Object.fromEntries(
  (await readFile(join(root, "_headers"), "utf8")).split("\n").filter((l) => /^\s+\S+:/.test(l)).map((l) => {
    const i = l.indexOf(":");
    return [l.slice(0, i).trim(), l.slice(i + 1).trim()];
  }),
);

let n = 1000;
http.createServer(async (req, res) => {
  const path = new URL(req.url, "http://x").pathname;
  if (path === "/health") return res.end("ok");
  if (path === "/api/face") {
    res.writeHead(200, { "Content-Type": "image/png", "Cache-Control": "no-store" });
    return res.end(png(160, ++n));
  }
  if (path === "/api/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    return res.end('{"ok":true,"backend":"browser"}');
  }
  if (path.startsWith("/api/")) {
    res.writeHead(404, { "Content-Type": "application/json" });
    return res.end('{"error":"not found"}');
  }
  const file = normalize(join(root, path === "/" ? "index.html" : path));
  if (!file.startsWith(root) || file.endsWith("_headers")) { res.writeHead(404); return res.end(); }
  try {
    const body = await readFile(file);
    res.writeHead(200, { ...headers, "Content-Type": TYPES[extname(file)] || "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end();
  }
}).listen(port, "127.0.0.1");
