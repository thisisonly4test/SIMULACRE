// Serverless backend: the same surface as the HTTP API, answered in the
// browser. The engine is simulacre-core compiled to WebAssembly (web/pkg/);
// the vault, its journal and the face library live in IndexedDB, so nothing
// leaves this browser. Only face pictures come from the network, through the
// hosting Worker's /api/face relay (thispersondoesnotexist.com sends no CORS
// headers, so the browser cannot fetch it directly).
//
// Errors are thrown as ApiError with the HTTP API's status codes and messages.

import { ApiError } from "./errors.js";

const DB_NAME = "simulacre";
const DB_VERSION = 1;
const FACE_SIZE = 512;
const MAX_UPLOAD = 8 * 1024 * 1024;

let wasm = null;
let db = null;
const faceUrls = new Map();   // face id → object URL, so <img> can show them synchronously

// ── IndexedDB ───────────────────────────────────────────────────────────────
function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const d = req.result;
      d.createObjectStore("personas", { keyPath: "id" });
      d.createObjectStore("journal", { keyPath: "id", autoIncrement: true }).createIndex("persona", "persona_id");
      d.createObjectStore("faces", { keyPath: "id" });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    req.onblocked = () => reject(new Error("database blocked by another tab"));
  });
}

const done = (req) => new Promise((resolve, reject) => { req.onsuccess = () => resolve(req.result); req.onerror = () => reject(req.error); });

/** Runs `fn(stores)` in one transaction and resolves once it has committed. */
function tx(names, mode, fn) {
  return new Promise((resolve, reject) => {
    const t = db.transaction(names, mode);
    const stores = Object.fromEntries([names].flat().map((n) => [n, t.objectStore(n)]));
    let result;
    Promise.resolve(fn(stores)).then((r) => { result = r; }, (e) => { try { t.abort(); } catch { /* already done */ } reject(e); });
    t.oncomplete = () => resolve(result);
    t.onerror = () => reject(t.error);
    t.onabort = () => reject(t.error || new ApiError(500, "storage error"));
  });
}

// ── Helpers ─────────────────────────────────────────────────────────────────
const now = () => new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
const hex = (bytes) => [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
const newId = () => hex(crypto.getRandomValues(new Uint8Array(6)));
const notFound = () => new ApiError(404, "not found");

/** Calls into the engine; its errors become 400s, like the server's. */
function engine(fn, ...args) {
  try { return fn(...args); } catch (e) { throw new ApiError(400, e?.message || String(e)); }
}
const J = (s) => JSON.parse(s);

async function getRecord(id) {
  const rec = await tx("personas", "readonly", (s) => done(s.personas.get(String(id))));
  if (!rec) throw notFound();
  return rec;
}
async function allRecords() {
  const recs = await tx("personas", "readonly", (s) => done(s.personas.getAll()));
  return recs.sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
}
const addJournal = (store, persona_id, kind, text) => done(store.add({ persona_id, at: now(), kind, text }));
async function journalOf(id) {
  const rows = await tx("journal", "readonly", (s) => done(s.journal.index("persona").getAll(String(id))));
  return rows.sort((a, b) => b.id - a.id);
}

async function validateFace(persona) {
  const f = persona?.face;
  if (!f) return;
  const known = wasm.is_valid_face_id(String(f.id)) && (await tx("faces", "readonly", (s) => done(s.faces.getKey(f.id))));
  if (!known) throw new ApiError(400, "unknown face id");
}

// ── Faces ───────────────────────────────────────────────────────────────────
/** Same magic numbers the server accepts: JPEG, PNG, WebP. */
function sniff(b) {
  return (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff)
    || (b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47)
    || (b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45 && b[10] === 0x42 && b[11] === 0x50);
}

/** Decode, centre-crop, resize to 512², re-encode as JPEG: drops metadata and
 *  gives a content-addressed id (first 128 bits of SHA-256), as the server does. */
async function normalize(blob) {
  let bmp;
  try { bmp = await createImageBitmap(blob); } catch { throw new ApiError(400, "image: cannot decode"); }
  const { width: w, height: h } = bmp;
  if (w < 64 || h < 64) { bmp.close(); throw new ApiError(400, `image: too small (${w}×${h})`); }
  const side = Math.min(w, h);
  const canvas = Object.assign(document.createElement("canvas"), { width: FACE_SIZE, height: FACE_SIZE });
  const g = canvas.getContext("2d");
  g.imageSmoothingQuality = "high";
  g.drawImage(bmp, (w - side) / 2, (h - side) / 2, side, side, 0, 0, FACE_SIZE, FACE_SIZE);
  bmp.close();
  const jpg = await new Promise((r) => canvas.toBlob(r, "image/jpeg", 0.88));
  if (!jpg) throw new ApiError(500, "image: cannot encode");
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", await jpg.arrayBuffer()));
  return { id: hex(digest.slice(0, 16)), jpg };
}

async function storeFace(raw, source) {
  const { id, jpg } = await normalize(raw);
  await tx("faces", "readwrite", async (s) => {
    if (!(await done(s.faces.getKey(id)))) await done(s.faces.put({ id, source, created_at: now(), blob: jpg }));
  });
  if (!faceUrls.has(id)) faceUrls.set(id, URL.createObjectURL(jpg));
  return { id, url: faceUrls.get(id), source };
}

export const faceUrl = (id) => faceUrls.get(id) || "";

// ── Backend ─────────────────────────────────────────────────────────────────
export async function init() {
  const mod = await import("../pkg/simulacre_wasm.js");
  await mod.default();
  wasm = mod;
  db = await openDb();
  const faces = await tx("faces", "readonly", (s) => done(s.faces.getAll()));
  for (const f of faces) faceUrls.set(f.id, URL.createObjectURL(f.blob));
}

const vaultItem = (r) => ({ id: r.id, status: r.status, created_at: r.created_at, updated_at: r.updated_at, face: r.persona.face ?? null, ...J(engine(wasm.summary, JSON.stringify(r.persona))) });

export const local = {
  meta: async () => ({ ...J(wasm.meta()), storage: "browser" }),
  seed: async () => ({ seed: wasm.seed() }),
  forge: async (recipe) => J(engine(wasm.forge, JSON.stringify(recipe ?? {}), today())),
  pool: async (q) => {
    const query = {};
    for (const [k, v] of Object.entries(q || {})) if (v !== "" && v != null) query[k] = k === "offset" || k === "limit" ? Number(v) : v;
    return J(engine(wasm.pool, JSON.stringify(query), today()));
  },
  check: async (persona) => J(engine(wasm.check, JSON.stringify(persona))),
  quiz: async (persona, round) => J(engine(wasm.quiz, JSON.stringify(persona), round >>> 0)),
  exportMd: async (persona, lang) => engine(wasm.export_md, JSON.stringify(persona), lang || "fr", "[]"),

  stats: async () => {
    const recs = await allRecords();
    const statuses = J(wasm.rules()).statuses;
    const by_status = Object.fromEntries(statuses.map((s) => [s, 0]));
    for (const r of recs) if (r.status in by_status) by_status[r.status]++;
    const avg = recs.length ? recs.reduce((a, r) => a + r.persona.coherence.score, 0) / recs.length : 0;
    return { total: recs.length, by_status, avg_coherence: Math.round(avg) };
  },
  contamination: async () => {
    const recs = await allRecords();
    const live = recs.filter((r) => r.status !== "burned").map((r) => [r.id, r.persona]);
    const names = Object.fromEntries(recs.map((r) => [r.id, {
      codename: r.persona.codename, name: `${r.persona.identity.first_name} ${r.persona.identity.last_name}`, status: r.status, face: r.persona.face ?? null,
    }]));
    return { pairs: J(engine(wasm.contamination, JSON.stringify(live))), names };
  },

  list: async () => (await allRecords()).map(vaultItem),
  create: async (persona) => {
    await validateFace(persona);
    const p = J(engine(wasm.check, JSON.stringify(persona)));
    const t = now();
    const rec = { id: newId(), status: "draft", created_at: t, updated_at: t, persona: p };
    await tx(["personas", "journal"], "readwrite", async (s) => {
      await done(s.personas.add(rec));
      await addJournal(s.journal, rec.id, "system", "created");
    });
    return rec;
  },
  get: getRecord,
  update: async (id, persona) => {
    await validateFace(persona);
    const p = J(engine(wasm.check, JSON.stringify(persona)));
    return tx(["personas", "journal"], "readwrite", async (s) => {
      const before = await done(s.personas.get(String(id)));
      if (!before) throw notFound();
      const rec = { ...before, persona: p, updated_at: now() };
      await done(s.personas.put(rec));
      await addJournal(s.journal, rec.id, "edit", wasm.edit_note(before.persona.coherence.score, p.coherence.score));
      return rec;
    });
  },
  status: async (id, status) => {
    if (!J(wasm.rules()).statuses.includes(status)) throw new ApiError(400, `invalid status '${status}'`);
    return tx(["personas", "journal"], "readwrite", async (s) => {
      const before = await done(s.personas.get(String(id)));
      if (!before) throw notFound();
      const rec = { ...before, status, updated_at: now() };
      await done(s.personas.put(rec));
      if (before.status !== status) await addJournal(s.journal, rec.id, "status", `${before.status} → ${status}`);
      return rec;
    });
  },
  remove: async (id) => {
    await tx(["personas", "journal"], "readwrite", async (s) => {
      if (!(await done(s.personas.getKey(String(id))))) throw notFound();
      await done(s.personas.delete(String(id)));
      const keys = await done(s.journal.index("persona").getAllKeys(String(id)));
      for (const k of keys) await done(s.journal.delete(k));
    });
    return null;
  },
  journal: async (id) => { await getRecord(id); return journalOf(id); },
  journalAdd: async (id, kind, text) => {
    await getRecord(id);
    const e = J(engine(wasm.journal_entry, kind || undefined, String(text ?? "")));
    const entryId = await tx("journal", "readwrite", (s) => addJournal(s.journal, String(id), e.kind, e.text));
    return (await tx("journal", "readonly", (s) => done(s.journal.get(entryId))));
  },
  savedQuiz: async (id, round) => J(engine(wasm.quiz, JSON.stringify((await getRecord(id)).persona), round >>> 0)),
  savedMd: async (id, lang) => {
    const rec = await getRecord(id);
    return engine(wasm.export_md, JSON.stringify(rec.persona), lang || "fr", JSON.stringify(await journalOf(id)));
  },
  savedJson: async (id) => JSON.stringify({ record: await getRecord(id), journal: await journalOf(id) }, null, 2),

  facesFetch: async (count) => {
    const n = Math.min(Math.max(count | 0 || 6, 1), 9);
    const faces = [], seen = new Set();
    let last = "";
    for (let i = 0; i < n * 3 && faces.length < n; i++) {
      try {
        const res = await fetch(`/api/face?n=${Date.now()}-${i}`, { cache: "no-store" });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const f = await storeFace(await res.blob(), "thispersondoesnotexist");
        if (!seen.has(f.id)) { seen.add(f.id); faces.push(f); }
      } catch (e) { last = e.message; if (!faces.length && i >= 1) break; }
    }
    if (!faces.length) throw new ApiError(502, `face source unreachable (${last}) — upload a picture instead`);
    return { faces, requested: n };
  },
  faceUpload: async (blob) => {
    if (!blob?.size) throw new ApiError(400, "empty body");
    if (blob.size > MAX_UPLOAD) throw new ApiError(413, "file too large (max 8 MB)");
    if (!sniff(new Uint8Array(await blob.slice(0, 12).arrayBuffer()))) throw new ApiError(400, "only JPEG, PNG or WebP");
    const { id, url } = await storeFace(blob, "upload");
    return { id, url };
  },
  faceUsers: async (id) => {
    if (!wasm.is_valid_face_id(String(id))) throw new ApiError(400, "bad face id");
    return (await allRecords()).filter((r) => r.persona.face?.id === id).map((r) => r.id);
  },
};
