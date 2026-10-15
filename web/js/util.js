// DOM helpers. Everything user- or persona-provided goes through textContent:
// no innerHTML with data, ever.

export function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k === "text") el.textContent = v;
    else if (k === "html") el.innerHTML = v; // only for static, trusted i18n strings
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "vars") for (const [n, val] of Object.entries(v)) el.style.setProperty(n, val);
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat(Infinity)) {
    if (kid == null || kid === false) continue;
    el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

const SVGNS = "http://www.w3.org/2000/svg";
export function s(tag, attrs = {}, ...kids) {
  const el = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v != null) el.setAttribute(k, v);
  for (const kid of kids.flat()) if (kid != null) el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  return el;
}

export const $ = (sel, root = document) => root.querySelector(sel);
export const clear = (el) => { while (el.firstChild) el.firstChild.remove(); return el; };

export function flag(cc) {
  if (!cc || cc.length !== 2) return "";
  return String.fromCodePoint(...[...cc.toUpperCase()].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65));
}

export function hashHue(str) {
  let x = 0;
  for (const c of str) x = (x * 31 + c.charCodeAt(0)) >>> 0;
  return x % 360;
}

/** Placeholder portrait: a deterministic silhouette tinted by the seed. */
export function silhouette(seed, initials) {
  const hue = hashHue(seed || "x");
  return s("svg", { viewBox: "0 0 100 100", role: "img", "aria-label": initials },
    s("defs", {}, s("linearGradient", { id: `g${hue}`, x1: 0, y1: 0, x2: 0, y2: 1 },
      s("stop", { offset: "0", "stop-color": `hsl(${hue} 30% 22%)` }),
      s("stop", { offset: "1", "stop-color": `hsl(${hue} 25% 9%)` }))),
    s("rect", { width: 100, height: 100, fill: `url(#g${hue})` }),
    s("circle", { cx: 50, cy: 40, r: 17, fill: `hsl(${hue} 18% 32%)` }),
    s("path", { d: "M14 100c3-22 18-32 36-32s33 10 36 32z", fill: `hsl(${hue} 18% 32%)` }),
    s("text", { x: 50, y: 92, "text-anchor": "middle", "font-family": "ui-monospace, monospace", "font-size": 11, "font-weight": 700, fill: `hsl(${hue} 50% 75%)` }, initials));
}

export const initials = (name) => (name || "?").split(/\s+/).map((p) => p[0] || "").join("").slice(0, 2).toUpperCase();

export const PLATFORM_COLORS = {
  instagram: "#e1306c", tiktok: "#25f4ee", x: "#e7e9ea", facebook: "#1877f2", reddit: "#ff4500", linkedin: "#0a66c2",
  youtube: "#ff3d3d", twitch: "#9146ff", discord: "#5865f2", github: "#c9d1d9", pinterest: "#e60023", strava: "#fc4c02",
  mastodon: "#6364ff", snapchat: "#fffc00", steam: "#66c0f4", vinted: "#09b1ba", flickr: "#ff0084", goodreads: "#d6c7a1",
  letterboxd: "#40bcf4", tripadvisor: "#34e0a1", ravelry: "#ee6e62", inaturalist: "#74ac00", vivino: "#a8213f", generic: "#8b919a",
};
const ABBR = { instagram: "IG", tiktok: "TT", x: "X", facebook: "FB", reddit: "RD", linkedin: "in", youtube: "YT", twitch: "TW", discord: "DC", github: "GH", pinterest: "P", strava: "ST", mastodon: "M", snapchat: "SC", steam: "SM", vinted: "V", flickr: "FL", goodreads: "GR", letterboxd: "LB", tripadvisor: "TA", ravelry: "RV", inaturalist: "iN", vivino: "VV", generic: "•" };
export function pbadge(platform) {
  return h("span", { class: "pbadge", vars: { "--pc": PLATFORM_COLORS[platform] || "#8b919a" }, title: platform }, ABBR[platform] || platform.slice(0, 2).toUpperCase());
}

let toastBox;
export function toast(msg, err = false) {
  toastBox ||= document.body.appendChild(h("div", { class: "toasts", role: "status", "aria-live": "polite" }));
  const el = toastBox.appendChild(h("div", { class: `toast${err ? " err" : ""}` }, msg));
  setTimeout(() => el.remove(), err ? 6000 : 3200);
}

export function download(name, text, type = "text/plain") {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = h("a", { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export const today = () => new Date().toISOString().slice(0, 10);
export const scoreColor = (v) => (v >= 85 ? "var(--green)" : v >= 65 ? "var(--amber)" : "var(--red)");
export const clone = (o) => structuredClone(o);

/** Walk a dotted path ("accounts.2.handle") and set its value. */
export function setPath(obj, path, value) {
  const parts = path.split(".");
  let o = obj;
  for (const p of parts.slice(0, -1)) o = o[p];
  o[parts.at(-1)] = value;
}
