// Encryption for everything the vault writes to disk: the browser build's
// IndexedDB records and the backup files of both builds.
//
// - Key: PBKDF2-SHA-256 (600 000 iterations, 16-byte random salt) from the
//   operator's passphrase, to a non-extractable AES-256-GCM key. WebCrypto
//   has no memory-hard KDF; the iteration count follows OWASP's 2023 advice.
// - Each value gets a fresh 96-bit IV, and "additional data" naming where it
//   belongs (e.g. "personas:<id>"), so a ciphertext cannot be swapped into
//   another record without failing authentication.
// - The key lives in memory only: closing the tab, or locking, forgets it.

const te = new TextEncoder();
const td = new TextDecoder();

export const ITERATIONS = 600_000;
export const MIN_PASSPHRASE = 10;
export const SEALED_FORMAT = "simulacre-vault-sealed";

export const randomBytes = (n) => crypto.getRandomValues(new Uint8Array(n));

export async function deriveKey(passphrase, salt, iterations = ITERATIONS) {
  const base = await crypto.subtle.importKey("raw", te.encode(passphrase.normalize("NFC")), "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", hash: "SHA-256", salt, iterations },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

/** Encrypts bytes; returns { iv, ct } (both Uint8Array). */
export async function seal(key, bytes, aad) {
  const iv = randomBytes(12);
  const ct = await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData: te.encode(aad) }, key, bytes);
  return { iv, ct: new Uint8Array(ct) };
}

/** Decrypts { iv, ct }; throws if the key or the additional data is wrong. */
export async function open(key, box, aad) {
  return new Uint8Array(await crypto.subtle.decrypt({ name: "AES-GCM", iv: box.iv, additionalData: te.encode(aad) }, key, box.ct));
}

export const sealJson = (key, value, aad) => seal(key, te.encode(JSON.stringify(value)), aad);
export const openJson = async (key, box, aad) => JSON.parse(td.decode(await open(key, box, aad)));

// ── Base64 and gzip ─────────────────────────────────────────────────────────
export function toBase64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
export function fromBase64(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
const pipe = async (bytes, stream) => new Uint8Array(await new Response(new Blob([bytes]).stream().pipeThrough(stream)).arrayBuffer());
const gzip = (bytes) => pipe(bytes, new CompressionStream("gzip"));
const gunzip = (bytes) => pipe(bytes, new DecompressionStream("gzip"));

// ── Sealed backup files ─────────────────────────────────────────────────────
/** True when `text` is a passphrase-protected backup. */
export function isSealed(text) {
  try { return JSON.parse(text)?.format === SEALED_FORMAT; } catch { return false; }
}

/** Wraps a plain backup (JSON text) into a passphrase-protected file. */
export async function sealBackup(text, passphrase) {
  const salt = randomBytes(16);
  const key = await deriveKey(passphrase, salt);
  const box = await seal(key, await gzip(te.encode(text)), SEALED_FORMAT);
  return JSON.stringify({
    format: SEALED_FORMAT,
    version: 1,
    kdf: { name: "PBKDF2", hash: "SHA-256", iterations: ITERATIONS, salt: toBase64(salt) },
    cipher: "AES-256-GCM",
    iv: toBase64(box.iv),
    data: toBase64(box.ct),
  });
}

/** Opens a sealed backup; throws Error("wrong passphrase") when it cannot. */
export async function openBackup(text, passphrase) {
  const f = JSON.parse(text);
  if (f.version !== 1 || f.kdf?.name !== "PBKDF2" || f.kdf.hash !== "SHA-256" || f.cipher !== "AES-256-GCM") throw new Error("unsupported sealed backup");
  const iterations = Number(f.kdf.iterations);
  if (!(iterations >= 100_000 && iterations <= 10_000_000)) throw new Error("unsupported sealed backup");
  const key = await deriveKey(passphrase, fromBase64(f.kdf.salt), iterations);
  let bytes;
  try { bytes = await open(key, { iv: fromBase64(f.iv), ct: fromBase64(f.data) }, SEALED_FORMAT); } catch { throw new Error("wrong passphrase"); }
  return td.decode(await gunzip(bytes));
}
