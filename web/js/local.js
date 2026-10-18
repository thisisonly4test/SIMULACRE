// Serverless backend: the same surface as the HTTP API, answered in the
// browser. The engine is simulacre-core compiled to WebAssembly (web/pkg/);
// the vault, its journal and the face library live in IndexedDB, so nothing
// leaves this browser. Only face pictures come from the network, through the
// hosting Worker's /api/face relay (thispersondoesnotexist.com sends no CORS
// headers, so the browser cannot fetch it directly).
//
// The vault is encrypted at rest (see vaultcrypto.js): every legend, journal
// entry and face picture is stored sealed with AES-GCM under a key derived
// from the operator's passphrase. Forging needs no key; anything that reads
// or writes the vault asks for the passphrase first, through the prompt the
// UI registers with setKeyPrompt(). Record ids, face ids and the link between
// a journal entry and its legend stay in clear, as IndexedDB keys.
//
// Errors are thrown as ApiError with the HTTP API's status codes and messages.

import { ApiError } from "./errors.js";
import { deriveKey, open, openJson, randomBytes, seal, sealJson, toBase64, fromBase64 } from "./vaultcrypto.js";

const DB_NAME = "simulacre";
const DB_VERSION = 2;
const FACE_SIZE = 512;
const MAX_UPLOAD = 8 * 1024 * 1024;
const AUTO_LOCK_MS = 15 * 60 * 1000;
/** Shown in place of a face while the vault is locked. */
const LOCKED_FACE = `data:image/svg+xml,${encodeURIComponent("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 40 50'><rect width='40' height='50' fill='#ddd5c2'/><g fill='none' stroke='#8a8170' stroke-width='2.2'><rect x='12' y='23' width='16' height='13' rx='2'/><path d='M15 23v-4a5 5 0 0 1 10 0v4'/></g></svg>")}`;

let wasm = null;
let db = null;
let key = null;                // AES-GCM key while unlocked, in memory only
let unlocking = null;          // shared promise while a passphrase prompt is open
let keyPrompt = null;          // (mode, error) => Promise<{passphrase}|{reset:true}|null>
let onLocked = null;
const faceUrls = new Map();   // face id → object URL, so <img> can show them synchronously

// ── IndexedDB ───────────────────────────────────────────────────────────────
function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const d = req.result;
      if (!d.objectStoreNames.contains("personas")) d.createObjectStore("personas", { keyPath: "id" });
      if (!d.objectStoreNames.contains("journal")) d.createObjectStore("journal", { keyPath: "id", autoIncrement: true }).createIndex("persona", "persona_id");
      if (!d.objectStoreNames.contains("faces")) d.createObjectStore("faces", { keyPath: "id" });
      if (!d.objectStoreNames.contains("meta")) d.createObjectStore("meta", { keyPath: "k" });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    req.onblocked = () => reject(new Error("database blocked by another tab"));
  });
}

const done = (req) => new Promise((resolve, reject) => { req.onsuccess = () => resolve(req.result); req.onerror = () => reject(req.error); });

/** Runs `fn(stores)` in one transaction and resolves once it has committed.
 *  `fn` may only await IndexedDB requests: any other await (WebCrypto…) would
 *  let the transaction commit early, so encryption happens before or after. */
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

// ── Key management ──────────────────────────────────────────────────────────
const cryptoMeta = () => tx("meta", "readonly", (s) => done(s.meta.get("crypto")));

/** Lets the UI ask for the passphrase. `mode` is "create" or "unlock". */
export const setKeyPrompt = (fn) => { keyPrompt = fn; };
export const setOnLocked = (fn) => { onLocked = fn; };

async function unlockWith(passphrase) {
  const m = await cryptoMeta();
  const k = await deriveKey(passphrase, m.salt, m.iterations);
  try { await open(k, m.verifier, "verifier"); } catch { return null; }
  return k;
}

/** First passphrase: seals whatever an older, unencrypted vault holds. */
async function createKey(passphrase) {
  const salt = randomBytes(16);
  const iterations = 600_000;
  const k = await deriveKey(passphrase, salt, iterations);
  const verifier = await seal(k, randomBytes(16), "verifier");
  const [recs, journal, faces] = await tx(["personas", "journal", "faces"], "readonly", (s) =>
    Promise.all([done(s.personas.getAll()), done(s.journal.getAll()), done(s.faces.getAll())]));
  const plainRecs = await Promise.all(recs.filter((r) => !r.ct).map((r) => sealRecord(k, r)));
  const plainJournal = await Promise.all(journal.filter((j) => !j.ct).map(async (j) => ({ id: j.id, ...(await sealEntry(k, j)) })));
  const plainFaces = await Promise.all(faces.filter((f) => !f.ct).map(async (f) => sealFace(k, f.id, f.source, f.created_at, new Uint8Array(await f.blob.arrayBuffer()))));
  await tx(["personas", "journal", "faces", "meta"], "readwrite", async (s) => {
    for (const r of plainRecs) await done(s.personas.put(r));
    for (const j of plainJournal) await done(s.journal.put(j));
    for (const f of plainFaces) await done(s.faces.put(f));
    await done(s.meta.put({ k: "crypto", v: 1, salt, iterations, verifier }));
  });
  return k;
}

/** Erases the whole vault (forgotten passphrase). */
async function wipe() {
  await tx(["personas", "journal", "faces", "meta"], "readwrite", async (s) => {
    for (const n of ["personas", "journal", "faces", "meta"]) await done(s[n].clear());
  });
  dropFaces();
}

/** Resolves with the key, asking for the passphrase if needed. */
function needKey() {
  if (key) return Promise.resolve(key);
  unlocking ??= (async () => {
    if (!keyPrompt) throw new ApiError(423, "vault locked");
    let error = null;
    for (;;) {
      const mode = (await cryptoMeta()) ? "unlock" : "create";
      const answer = await keyPrompt(mode, error);
      if (!answer) throw new ApiError(423, "vault locked");
      if (answer.reset) { await wipe(); error = null; continue; }
      const k = mode === "unlock" ? await unlockWith(answer.passphrase) : await createKey(answer.passphrase);
      if (!k) { error = "wrong"; continue; }
      key = k;
      await loadFaces();
      touch();
      return key;
    }
  })().finally(() => { unlocking = null; });
  return unlocking;
}

function dropFaces() {
  for (const url of faceUrls.values()) URL.revokeObjectURL(url);
  faceUrls.clear();
}

/** Forgets the key (and the decrypted pictures). */
export function lock() {
  if (!key) return;
  key = null;
  dropFaces();
  onLocked?.();
}

// Locks itself after a quarter of an hour without any input.
let lastInput = Date.now();
const touch = () => { lastInput = Date.now(); };
for (const ev of ["pointerdown", "keydown", "wheel", "touchstart"]) addEventListener(ev, touch, { passive: true });
setInterval(() => { if (key && Date.now() - lastInput > AUTO_LOCK_MS) lock(); }, 30_000);

// ── Sealed rows ─────────────────────────────────────────────────────────────
async function sealRecord(k, r) {
  return { id: r.id, ...(await sealJson(k, { status: r.status, created_at: r.created_at, updated_at: r.updated_at, persona: r.persona }, `personas:${r.id}`)) };
}
async function openRecord(row) {
  return row.ct ? { id: row.id, ...(await openJson(key, row, `personas:${row.id}`)) } : row;
}
async function sealEntry(k, e) {
  return { persona_id: e.persona_id, ...(await sealJson(k, { at: e.at, kind: e.kind, text: e.text }, `journal:${e.persona_id}`)) };
}
async function openEntry(row) {
  return row.ct ? { id: row.id, persona_id: row.persona_id, ...(await openJson(key, row, `journal:${row.persona_id}`)) } : row;
}
async function sealFace(k, id, source, created_at, bytes) {
  return { id, source, created_at, ...(await seal(k, bytes, `faces:${id}`)) };
}
async function faceBytes(row) {
  return row.ct ? open(key, row, `faces:${row.id}`) : new Uint8Array(await row.blob.arrayBuffer());
}
async function loadFaces() {
  const rows = await tx("faces", "readonly", (s) => done(s.faces.getAll()));
  for (const f of rows) {
    if (faceUrls.has(f.id)) continue;
    try { faceUrls.set(f.id, URL.createObjectURL(new Blob([await faceBytes(f)], { type: "image/jpeg" }))); } catch { /* unreadable picture: left out */ }
  }
}

/** For reads: a vault that was never used is just empty, no passphrase
 *  needed yet (it is chosen on the first write). Otherwise, unlock. */
async function readable() {
  if (key) return true;
  const fresh = !(await cryptoMeta()) && !(await tx("personas", "readonly", (s) => done(s.personas.count())));
  if (fresh) return false;
  await needKey();
  return true;
}

async function getRecord(id) {
  if (!(await readable())) throw notFound();
  const row = await tx("personas", "readonly", (s) => done(s.personas.get(String(id))));
  if (!row) throw notFound();
  return openRecord(row);
}
async function allRecords() {
  if (!(await readable())) return [];
  const rows = await tx("personas", "readonly", (s) => done(s.personas.getAll()));
  const recs = await Promise.all(rows.map(openRecord));
  return recs.sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
}
const entry = (persona_id, kind, text) => sealEntry(key, { persona_id, at: now(), kind, text });
async function journalOf(id) {
  if (!(await readable())) return [];
  const rows = await tx("journal", "readonly", (s) => done(s.journal.index("persona").getAll(String(id))));
  const entries = await Promise.all(rows.map(openEntry));
  return entries.sort((a, b) => b.id - a.id);
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
  const bytes = new Uint8Array(await jpg.arrayBuffer());
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return { id: hex(digest.slice(0, 16)), bytes };
}

async function putFace(id, source, created_at, bytes) {
  const row = await sealFace(key, id, source, created_at, bytes);
  const added = await tx("faces", "readwrite", async (s) => {
    if (await done(s.faces.getKey(id))) return false;
    await done(s.faces.put(row));
    return true;
  });
  if (!faceUrls.has(id)) faceUrls.set(id, URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" })));
  return added;
}

async function storeFace(raw, source) {
  await needKey();
  const { id, bytes } = await normalize(raw);
  await putFace(id, source, now(), bytes);
  return { id, url: faceUrls.get(id), source };
}

export const faceUrl = (id) => faceUrls.get(id) || (key ? "" : LOCKED_FACE);

// ── Backend ─────────────────────────────────────────────────────────────────
export async function init() {
  const mod = await import("../pkg/simulacre_wasm.js");
  await mod.default();
  wasm = mod;
  db = await openDb();
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

  /** Never prompts: while locked, only the legend count is known. */
  stats: async () => {
    const statuses = J(wasm.rules()).statuses;
    const by_status = Object.fromEntries(statuses.map((s) => [s, 0]));
    if (!key) {
      const total = await tx("personas", "readonly", (s) => done(s.personas.count()));
      return { total, by_status, avg_coherence: 0, locked: true };
    }
    const recs = await allRecords();
    for (const r of recs) if (r.status in by_status) by_status[r.status]++;
    const avg = recs.length ? recs.reduce((a, r) => a + r.persona.coherence.score, 0) / recs.length : 0;
    return { total: recs.length, by_status, avg_coherence: Math.round(avg) };
  },
  vaultState: async () => ({ backend: "browser", encrypted: !!(await cryptoMeta()), unlocked: !!key }),
  unlock: async () => { await needKey(); return null; },
  lock: async () => { lock(); return null; },
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
    await needKey();
    await validateFace(persona);
    const p = J(engine(wasm.check, JSON.stringify(persona)));
    const t = now();
    const rec = { id: newId(), status: "draft", created_at: t, updated_at: t, persona: p };
    const [row, j] = await Promise.all([sealRecord(key, rec), entry(rec.id, "system", "created")]);
    await tx(["personas", "journal"], "readwrite", async (s) => {
      await done(s.personas.add(row));
      await done(s.journal.add(j));
    });
    return rec;
  },
  get: getRecord,
  update: async (id, persona) => {
    const before = await getRecord(id);
    await validateFace(persona);
    const p = J(engine(wasm.check, JSON.stringify(persona)));
    const rec = { ...before, persona: p, updated_at: now() };
    const [row, j] = await Promise.all([sealRecord(key, rec), entry(rec.id, "edit", wasm.edit_note(before.persona.coherence.score, p.coherence.score))]);
    await tx(["personas", "journal"], "readwrite", async (s) => {
      if (!(await done(s.personas.getKey(rec.id)))) throw notFound();
      await done(s.personas.put(row));
      await done(s.journal.add(j));
    });
    return rec;
  },
  status: async (id, status) => {
    if (!J(wasm.rules()).statuses.includes(status)) throw new ApiError(400, `invalid status '${status}'`);
    const before = await getRecord(id);
    const rec = { ...before, status, updated_at: now() };
    const row = await sealRecord(key, rec);
    const j = before.status !== status ? await entry(rec.id, "status", `${before.status} → ${status}`) : null;
    await tx(["personas", "journal"], "readwrite", async (s) => {
      if (!(await done(s.personas.getKey(rec.id)))) throw notFound();
      await done(s.personas.put(row));
      if (j) await done(s.journal.add(j));
    });
    return rec;
  },
  remove: async (id) => {
    await needKey();
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
    const at = now();
    const row = await sealEntry(key, { persona_id: String(id), at, kind: e.kind, text: e.text });
    const entryId = await tx("journal", "readwrite", (s) => done(s.journal.add(row)));
    return { id: entryId, persona_id: String(id), at, kind: e.kind, text: e.text };
  },
  savedQuiz: async (id, round) => J(engine(wasm.quiz, JSON.stringify((await getRecord(id)).persona), round >>> 0)),
  savedMd: async (id, lang) => {
    const rec = await getRecord(id);
    return engine(wasm.export_md, JSON.stringify(rec.persona), lang || "fr", JSON.stringify(await journalOf(id)));
  },
  savedJson: async (id) => JSON.stringify({ record: await getRecord(id), journal: await journalOf(id) }, null, 2),

  // ── Whole-vault backup ───────────────────────────────────────────────────
  // Same file format as the server's (simulacre-core::backup), so a backup
  // moves freely between the browser build and a self-hosted server. The UI
  // seals the file with a passphrase before it is saved.
  backup: async () => {
    const recs = await allRecords();
    const [journalRows, faceRows] = await tx(["journal", "faces"], "readonly", (s) => Promise.all([done(s.journal.getAll()), done(s.faces.getAll())]));
    const journal = (await Promise.all(journalRows.map(openEntry))).sort((a, b) => a.id - b.id);
    const byPersona = new Map();
    for (const j of journal) {
      if (!byPersona.has(j.persona_id)) byPersona.set(j.persona_id, []);
      byPersona.get(j.persona_id).push({ at: j.at, kind: j.kind, text: j.text });
    }
    const records = recs.map((r) => ({ id: r.id, status: r.status, created_at: r.created_at, updated_at: r.updated_at, persona: r.persona, journal: byPersona.get(r.id) || [] }));
    const faces = await Promise.all(faceRows.map(async (f) => ({ id: f.id, source: f.source, created_at: f.created_at, data: toBase64(await faceBytes(f)) })));
    return JSON.stringify({ ...J(wasm.backup_header()), exported_at: now(), records, faces });
  },
  restore: async (text) => {
    const b = J(engine(wasm.backup_parse, String(text)));
    await needKey();
    const report = { added: 0, updated: 0, skipped: 0, faces: 0, faces_missing: 0 };
    // Faces first, so the legends that use them pass validation.
    for (const f of b.faces) if (await putFace(f.id, f.source, f.created_at, fromBase64(f.data))) report.faces++;
    for (const r of b.records) {
      let persona = r.persona;
      if (persona.face && !(await tx("faces", "readonly", (s) => done(s.faces.getKey(persona.face.id))))) {
        persona = J(engine(wasm.check, JSON.stringify({ ...persona, face: null })));
        report.faces_missing++;
      }
      const existing = await tx("personas", "readonly", (s) => done(s.personas.get(r.id)));
      const before = existing ? await openRecord(existing) : null;
      if (before && !wasm.backup_should_replace(before.updated_at, r.updated_at)) { report.skipped++; continue; }
      const row = await sealRecord(key, { ...r, persona });
      const entries = await Promise.all([...r.journal.map((j) => sealEntry(key, { persona_id: r.id, ...j })), entry(r.id, "system", "imported from backup")]);
      await tx(["personas", "journal"], "readwrite", async (s) => {
        if (before) for (const k of await done(s.journal.index("persona").getAllKeys(r.id))) await done(s.journal.delete(k));
        await done(s.personas.put(row));
        for (const e of entries) await done(s.journal.add(e));
      });
      report[before ? "updated" : "added"]++;
    }
    return report;
  },

  facesFetch: async (count) => {
    await needKey();
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
