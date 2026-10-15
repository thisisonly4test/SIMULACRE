// Thin fetch wrapper. Adds the optional bearer token and turns error bodies
// into exceptions carrying the server's message.

let token = (() => { try { return localStorage.getItem("simulacre.token") || ""; } catch { return ""; } })();
let onUnauthorized = null;

export function setToken(v) {
  token = v || "";
  try { localStorage.setItem("simulacre.token", token); } catch { /* ignore */ }
  // <img src="/faces/…"> cannot send headers; the server also accepts a cookie.
  document.cookie = `simulacre_token=${encodeURIComponent(token)}; path=/; SameSite=Strict`;
}
export const onAuthNeeded = (fn) => { onUnauthorized = fn; };

export class ApiError extends Error {
  constructor(status, message) { super(message); this.status = status; }
}

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

export const api = {
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
  facesFetch: (count) => req("POST", `/api/faces/fetch?count=${count}`),
  faceUpload: (blob) => req("POST", "/api/faces/upload", blob),
  faceUsers: (id) => req("GET", `/api/faces/${id}/users`),
};
