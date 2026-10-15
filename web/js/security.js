// Security-printing toolkit: guilloché engraving, MRZ lines, UV lamp,
// holographic foil. Everything is generated at runtime — no images shipped.

// ── Guilloché ───────────────────────────────────────────────────────────────
/** Wave-line banknote tile, as a data: URI for CSS backgrounds. */
export function waveTile(color, opacity = 0.35) {
  const W = 320, H = 160;
  let paths = "";
  for (let j = 0; j < 22; j++) {
    let d = "";
    for (let x = 0; x <= W; x += 4) {
      const y = j * 8 + 6 * Math.sin((x / W) * Math.PI * 4 + j * 0.45) + 3 * Math.sin((x / W) * Math.PI * 10 - j * 0.3);
      d += `${x ? "L" : "M"}${x} ${y.toFixed(1)}`;
    }
    paths += `<path d='${d}'/>`;
  }
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${W}' height='${H}'><g fill='none' stroke='${color}' stroke-width='.5' opacity='${opacity}'>${paths}</g></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

/** Random fluorescent fibres — invisible on paper, glowing under UV. */
export function fibresTile() {
  const W = 420, H = 420;
  const cols = ["#ff3df2", "#3dfff2", "#f6ff3d", "#7d5cff"];
  let out = "";
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 70; i++) {
    const x = rnd() * W, y = rnd() * H, a = rnd() * Math.PI, l = 8 + rnd() * 22;
    const cx = x + Math.cos(a + 0.6) * l * 0.6, cy = y + Math.sin(a + 0.6) * l * 0.6;
    out += `<path d='M${x.toFixed(1)} ${y.toFixed(1)} Q${cx.toFixed(1)} ${cy.toFixed(1)} ${(x + Math.cos(a) * l).toFixed(1)} ${(y + Math.sin(a) * l).toFixed(1)}' stroke='${cols[i % 4]}'/>`;
  }
  const words = "FICTIF · FICTIONAL · SPECIMEN · SIMULACRE · ";
  let text = "";
  for (let r = 0; r < 14; r++) text += `<text x='${(r % 2) * -60}' y='${r * 30 + 18}'>${words.repeat(4)}</text>`;
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${W}' height='${H}'><g fill='none' stroke-width='1.1' stroke-linecap='round'>${out}</g><g fill='#3dfff2' fill-opacity='.22' font-family='monospace' font-size='9' letter-spacing='2'>${text}</g></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

// ── MRZ (ICAO 9303 TD3) ─────────────────────────────────────────────────────
// Issuer and nationality are "UTO" — ICAO's own code for specimen documents —
// so the strip is unmistakably a specimen, never a usable document.
const fold = (x) => x.normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/ß/g, "SS").toUpperCase().replace(/[^A-Z]/g, "<");
const val = (c) => (c === "<" ? 0 : /\d/.test(c) ? +c : c.charCodeAt(0) - 55);
export const checkDigit = (str) => String([...str].reduce((acc, c, i) => acc + val(c) * [7, 3, 1][i % 3], 0) % 10);
const pad = (x, n) => (x + "<".repeat(n)).slice(0, n);
const yymmdd = (iso) => iso.slice(2, 4) + iso.slice(5, 7) + iso.slice(8, 10);

export function mrz(p) {
  const id = p.identity;
  const names = `${fold(id.last_name)}<<${fold(id.first_name)}`.replace(/<+$/, "");
  const l1 = pad(`P<UTO${names}`, 44);
  const doc = pad(p.recipe.seed.replace(/^SIM-/, "").replace(/-/g, "").toUpperCase(), 9);
  const birth = yymmdd(id.birth_date);
  const exp = String(Number(p.recipe.as_of.slice(0, 4)) + 10) + p.recipe.as_of.slice(4);
  const expiry = yymmdd(exp);
  const sex = id.gender === "female" ? "F" : "M";
  const personal = pad("SPECIMEN", 14);
  const parts = [doc, checkDigit(doc), "UTO", birth, checkDigit(birth), sex, expiry, checkDigit(expiry), personal, checkDigit(personal)];
  const composite = doc + checkDigit(doc) + birth + checkDigit(birth) + expiry + checkDigit(expiry) + personal + checkDigit(personal);
  const l2 = parts.join("") + checkDigit(composite);
  return [l1, l2];
}

// ── UV lamp & holographic foil ──────────────────────────────────────────────
const UV_KEY = "simulacre.uv";
export function initUv() {
  let on;
  try { on = localStorage.getItem(UV_KEY); } catch { on = null; }
  // Paper is the default, whatever the OS theme: UV is opt-in only.
  setUv(on === "1", false);
  addEventListener("keydown", (e) => {
    if (e.key.toLowerCase() === "u" && !/INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName || "") && !e.ctrlKey && !e.metaKey) setUv(!isUv());
  });
}
export const isUv = () => document.documentElement.dataset.theme === "uv";
export function setUv(on, persist = true) {
  const root = document.documentElement;
  root.dataset.theme = on ? "uv" : "paper";
  root.style.setProperty("--wave", waveTile(on ? "#7d5cff" : "#0d6b63", on ? 0.28 : 0.22));
  root.style.setProperty("--fibres", fibresTile());
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", on ? "#0a0614" : "#efe9dc");
  document.querySelectorAll(".uv-toggle").forEach((b) => b.setAttribute("aria-pressed", String(on)));
  if (persist) try { localStorage.setItem(UV_KEY, on ? "1" : "0"); } catch { /* private mode */ }
}

/** Iridescent foil that shifts as the pointer moves over the element. */
export function holo(el) {
  el.addEventListener("pointermove", (e) => {
    const r = el.getBoundingClientRect();
    el.style.setProperty("--hx", ((e.clientX - r.left) / r.width).toFixed(3));
    el.style.setProperty("--hy", ((e.clientY - r.top) / r.height).toFixed(3));
  });
  return el;
}
