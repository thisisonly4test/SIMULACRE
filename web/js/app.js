import "./migrate.js";
import { api, onAuthNeeded, setToken } from "./api.js";
import { tokenPrompt } from "./components.js";
import { lang, setLabels, setLang, t } from "./i18n.js";
import { puppetMark } from "./puppet.js";
import { initUv, isUv, setUv } from "./security.js";
import { clear, h, toast } from "./util.js";
import { contaminationView, ctx, doctrineView, forgeView, personaView, poolView, vaultView } from "./views.js";

const ROUTES = [
  { re: /^#?\/?$/, nav: "forge", view: forgeView },
  { re: /^#\/pool/, nav: "pool", view: poolView },
  { re: /^#\/vault/, nav: "vault", view: vaultView },
  { re: /^#\/p\/([\w-]+)/, nav: "vault", view: personaView },
  { re: /^#\/contamination/, nav: "contam", view: contaminationView },
  { re: /^#\/doctrine/, nav: "doctrine", view: doctrineView },
];

let cleanup = null;
const main = () => document.getElementById("main");

function shell() {
  const navLink = (key, href) => h("a", { href, "data-nav": key }, t(`nav_${key}`), key === "vault" ? h("span", { class: "count", id: "vault-count" }) : null);
  const langBtn = (l) => h("button", { class: "lang-btn", "aria-pressed": String(lang() === l), onclick: () => { setLang(l); render(); route(); } }, l.toUpperCase());
  const uvBtn = h("button", { class: "uv-toggle", "aria-pressed": String(isUv()), title: `${t("uv")} (U)`, onclick: () => setUv(!isUv()) }, h("span", { class: "uv-bulb", "aria-hidden": "true" }), "UV");
  const top = h("header", { class: "topbar" }, h("div", { class: "topbar-inner" },
    h("a", { class: "brand", href: "#/" }, puppetMark(), h("span", {}, "Simulacre")),
    h("span", { class: "classified", "aria-hidden": "true" }, t("classified")),
    h("nav", { class: "nav", "aria-label": "Sections" },
      navLink("forge", "#/"), navLink("pool", "#/pool"), navLink("vault", "#/vault"), navLink("contam", "#/contamination"), navLink("doctrine", "#/doctrine")),
    uvBtn,
    h("div", { class: "lang-switch", role: "group", "aria-label": "Language / Langue" }, langBtn("fr"), langBtn("en"))));
  const foot = h("footer", { class: "foot" }, h("blockquote", { class: "epigraph" }, h("p", {}, t("epigraph")), h("cite", {}, t("epigraph_src"))), h("span", {}, t("footer")), h("span", { class: "mono" }, `SIMULACRE v${ctx.meta.version} · UTO`));
  // Invisible-ink layer: fluorescent fibres, shown only under UV light.
  const uv = h("div", { class: "uv-layer", "aria-hidden": "true" }, h("div", { class: "uv-fibres" }));
  return [h("a", { class: "skip", href: "#main" }, t("skip")), top, h("main", { id: "main", tabindex: "-1" }), foot, uv];
}

function render() {
  clear(document.body).append(...shell());
  refreshNav();
}

async function refreshNav() {
  try {
    const s = await api.stats();
    const el = document.getElementById("vault-count");
    if (el) el.textContent = s.total ? String(s.total) : "";
  } catch { /* offline or locked */ }
}
ctx.refreshNav = refreshNav;

async function route() {
  cleanup?.();
  cleanup = null;
  const hash = location.hash || "#/";
  const r = ROUTES.find((x) => x.re.test(hash.split("?")[0])) || ROUTES[0];
  document.querySelectorAll(".nav a").forEach((a) => { if (a.dataset.nav === r.nav) a.setAttribute("aria-current", "page"); else a.removeAttribute("aria-current"); });
  // Each navigation renders into its own container. A view still loading when
  // the user navigates away then writes into a detached node, never over the
  // next page.
  const root = h("div", { class: "view" });
  clear(main()).append(root);
  const m = hash.split("?")[0].match(r.re);
  try {
    const res = await r.view(root, m?.[1]);
    if (typeof res === "function") { if (root.isConnected) cleanup = res; else res(); }
  } catch (e) {
    if (!root.isConnected) return;
    root.append(h("div", { class: "empty" }, h("b", {}, t("err")), e.message));
    toast(e.message, true);
  }
  if (!root.isConnected) return;
  if (r.nav !== "forge") window.scrollTo({ top: 0 });
}

onAuthNeeded(async () => setToken(await tokenPrompt()));

async function boot() {
  setLang(lang());
  initUv();
  try {
    ctx.meta = await api.meta();
  } catch (e) {
    document.body.textContent = `SIMULACRE: ${e.message}`;
    return;
  }
  setLabels(ctx.meta.labels);
  render();
  window.addEventListener("hashchange", route);
  route();
}

boot();
