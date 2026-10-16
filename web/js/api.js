// The UI's single entry point to the engine. Two backends answer the same
// calls:
// - "server": the Rust HTTP server (`simulacre`), detected via /api/health;
// - "browser": no server (static hosting such as Cloudflare). The engine runs
//   as WebAssembly and the vault lives in IndexedDB (see local.js).
// The HTTP side adds the optional bearer token and turns error bodies into
// exceptions carrying the server's message.

import { ApiError } from "./errors.js";

export { ApiError };

let token = (() => { try { return localStorage.getItem("simulacre.token") || ""; } catch { return ""; } })();
let onUnauthorized = null;

export function setToken(v) {
  token = v || "";
  try { localStorage.setItem("simulacre.token", token); } catch { /* ignore */ }
  // <img src="/faces/…"> cannot send headers; the server also accepts a cookie.
  document.cookie = `simulacre_token=${encodeURIComponent(token)}; path=/; SameSite=Strict`;
}
export const onAuthNeeded = (fn) => { onUnauthorized = fn; };

async function req(method, url, body, raw = false) {
  const headers = {};
  if (token) headers.Authorization = `Bearer ${token}`;
  let payload;
  if (body instanceof Blob || body instanceof ArrayBuffer) payload = body;
  else if (body !== undefined) { headers["Content-Type"] = "application/json"; payload = JSON.stringify(body); }
  const res = await fetch(url, { method, headers, body: payload });
  if (res.status === 401 && onUnauthorized) {
    await onUnauthorized();
    return req(method, url, body, raw);
  }
  if (!res.ok) {
    let msg = res.statusText;
    try { msg = (await res.json()).error || msg; } catch { /* not json */ }
    throw new ApiError(res.status, msg);
  }
  if (raw) return res.text();
  if (res.status === 204) return null;
  return res.json();
}

if (token) setToken(token);

const http = {
  meta: () => req("GET", "/api/meta"),
  seed: () => req("GET", "/api/seed"),
  forge: (recipe) => req("POST", "/api/forge", recipe),
  pool: (q) => req("GET", `/api/pool?${new URLSearchParams(q)}`),
  check: (persona) => req("POST", "/api/check", persona),
  quiz: (persona, round) => req("POST", "/api/quiz", { persona, round }),
  exportMd: (persona, lang) => req("POST", "/api/export", { persona, lang }, true),
  stats: () => req("GET", "/api/stats"),
  contamination: () => req("GET", "/api/contamination"),
  list: () => req("GET", "/api/personas"),
  create: (persona) => req("POST", "/api/personas", persona),
  get: (id) => req("GET", `/api/personas/${encodeURIComponent(id)}`),
  update: (id, persona) => req("PUT", `/api/personas/${encodeURIComponent(id)}`, persona),
  status: (id, status) => req("PUT", `/api/personas/${encodeURIComponent(id)}/status`, { status }),
  remove: (id) => req("DELETE", `/api/personas/${encodeURIComponent(id)}`),
  journal: (id) => req("GET", `/api/personas/${encodeURIComponent(id)}/journal`),
  journalAdd: (id, kind, text) => req("POST", `/api/personas/${encodeURIComponent(id)}/journal`, { kind, text }),
  savedQuiz: (id, round) => req("GET", `/api/personas/${encodeURIComponent(id)}/quiz?round=${round}`),
  savedMd: (id, lang) => req("GET", `/api/personas/${encodeURIComponent(id)}/export.md?lang=${lang}`, undefined, true),
  savedJson: (id) => req("GET", `/api/personas/${encodeURIComponent(id)}/export.json`, undefined, true),
  backup: () => req("GET", "/api/vault/backup", undefined, true),
  restore: (text) => req("POST", "/api/vault/restore", new Blob([text], { type: "application/json" })),
  facesFetch: (count) => req("POST", `/api/faces/fetch?count=${count}`),
  faceUpload: (blob) => req("POST", "/api/faces/upload", blob),
  faceUsers: (id) => req("GET", `/api/faces/${id}/users`),
};

let mode = null;
let backend = null;

/** Picks the backend once: the Rust server if it answers /api/health. */
function ready() {
  backend ??= (async () => {
    try {
      const res = await fetch("/api/health", { cache: "no-store" });
      const body = res.ok && (res.headers.get("content-type") || "").includes("json") ? await res.json() : null;
      // The Cloudflare Worker answers too, with backend "browser".
      if (body?.ok === true && body.backend !== "browser") {
        mode = "server";
        return http;
      }
    } catch { /* no server reachable: fall through */ }
    const local = await import("./local.js");
    await local.init();
    mode = "browser";
    faceUrlFn = local.faceUrl;
    return local.local;
  })();
  return backend;
}

let faceUrlFn = (id) => `/faces/${id}.jpg`;
/** Where a stored face can be displayed from (synchronous, for <img src>). */
export const faceUrl = (id) => faceUrlFn(id);
/** "server" or "browser", once the first call has resolved. */
export const backendMode = () => mode;

export const api = Object.fromEntries(Object.keys(http).map((k) => [k, async (...args) => (await ready())[k](...args)]));
