import { api } from "./api.js";
import { section } from "./components.js";
import { marionette } from "./puppet.js";
import { applyEdit, commonActions, dossier } from "./dossier.js";
import { L, lab, lang, t } from "./i18n.js";
import { clear, download, flag, h, initials, pbadge, scoreColor, silhouette, sleep, toast } from "./util.js";

export const ctx = { meta: null, refreshNav: () => {} };

const archLabel = (key) => ctx.meta.archetypes.find((a) => a.key === key)?.label;
const routeParams = () => new URLSearchParams(location.hash.split("?")[1] || "");

function opt(value, label, selected) {
  return h("option", { value, selected: selected ? true : null }, label);
}

// ═════════════════════════════════════ FORGE ═════════════════════════════════
// Forge history: every forged legend is kept (edits, rerolls and face
// included) so the operator can step back to one they preferred. Linear like
// browser history: forging after going back drops the "forward" branch.
const HISTORY_KEY = "simulacre.forge-history";
const HISTORY_MAX = 30;
const forgeState = { recipe: null, persona: null, dirty: false, lastSeed: null, hist: loadHistory() };

function loadHistory() {
  try {
    const h = JSON.parse(sessionStorage.getItem(HISTORY_KEY) || "null");
    if (h && Array.isArray(h.entries) && Number.isInteger(h.pos) && h.pos < h.entries.length) return h;
  } catch { /* corrupt or unavailable */ }
  return { entries: [], pos: -1 };
}
function saveHistory() {
  const h = forgeState.hist;
  // Best effort: on quota errors, drop the oldest entries until it fits.
  for (let drop = 0; drop < h.entries.length; drop++) {
    try {
      sessionStorage.setItem(HISTORY_KEY, JSON.stringify(drop ? { entries: h.entries.slice(drop), pos: h.pos - drop } : h));
      return;
    } catch { if (h.pos - drop - 1 < 0) break; }
  }
  try { sessionStorage.removeItem(HISTORY_KEY); } catch { /* ignore */ }
}
const recipeKey = (r) => JSON.stringify([r.seed, r.locale, r.archetype || null, r.gender || null, r.age_min ?? null, r.age_max ?? null]);
/** A new forge: drop the forward branch, append, cap. Same recipe = refresh in place. */
function historyPush(p) {
  const h = forgeState.hist;
  const cur = h.entries[h.pos];
  if (cur && recipeKey(cur.p.recipe) === recipeKey(p.recipe)) {
    h.entries[h.pos] = { p, dirty: false };
  } else {
    h.entries.splice(h.pos + 1);
    h.entries.push({ p, dirty: false });
    if (h.entries.length > HISTORY_MAX) h.entries.splice(0, h.entries.length - HISTORY_MAX);
    h.pos = h.entries.length - 1;
  }
  saveHistory();
}
/** Edits, rerolls and face changes amend the current entry, never add one. */
function historyAmend(p, dirty) {
  const h = forgeState.hist;
  if (h.pos >= 0) { h.entries[h.pos] = { p, dirty }; saveHistory(); }
}
{
  const cur = forgeState.hist.entries[forgeState.hist.pos];
  if (cur) Object.assign(forgeState, { persona: cur.p, recipe: cur.p.recipe, dirty: cur.dirty, lastSeed: cur.p.recipe.seed });
}

export function forgeView(root) {
  const q = routeParams();
  const meta = ctx.meta;
  const seedIn = h("input", { class: "input", id: "seed", value: q.get("seed") || forgeState.recipe?.seed || "", placeholder: "SIM-XXXX-XXXX", spellcheck: "false", autocomplete: "off" });
  const locIn = h("select", { class: "input", id: "locale" }, opt("", t("any")), meta.locales.map((l) => opt(l.code, `${flag(l.country_code)} ${L(l.country)}`, (q.get("locale") || forgeState.recipe?.locale) === l.code)));
  const archIn = h("select", { class: "input", id: "archetype" }, opt("", t("any")), meta.archetypes.map((a) => opt(a.key, L(a.label), (q.get("archetype") || forgeState.recipe?.archetype) === a.key)));
  const genIn = h("select", { class: "input", id: "gender" }, opt("", t("any")), opt("female", t("female"), forgeState.recipe?.gender === "female"), opt("male", t("male"), forgeState.recipe?.gender === "male"));
  const ageMin = h("input", { class: "input", id: "age_min", type: "number", min: meta.limits.min_age, max: meta.limits.max_age, placeholder: String(meta.limits.min_age), value: forgeState.recipe?.age_min ?? "" });
  const ageMax = h("input", { class: "input", id: "age_max", type: "number", min: meta.limits.min_age, max: meta.limits.max_age, placeholder: String(meta.limits.max_age), value: forgeState.recipe?.age_max ?? "" });
  const desc = h("p", { class: "arch-desc" });
  const syncDesc = () => { const a = meta.archetypes.find((x) => x.key === archIn.value); desc.textContent = a ? L(a.desc) : t("any_arch"); };
  archIn.addEventListener("change", syncDesc);
  syncDesc();
  // type=submit: the form's submit handler is the single trigger (no double forge).
  const go = h("button", { class: "btn btn-primary btn-big", id: "forge-btn", type: "submit" }, "▶ " + t("forge"), h("span", { class: "kbd" }, t("shortcut")));
  const dice = h("button", { class: "btn btn-icon", type: "button", title: t("random_seed"), "aria-label": t("random_seed"), onclick: async () => { seedIn.value = (await api.seed()).seed; seedTouched = true; } }, "⚄");
  const log = h("ol", { class: "log", id: "log" });
  const histBar = h("nav", { class: "history", "aria-label": t("history") });
  const out = h("div", { id: "dossier" });

  const fillForm = (r) => {
    seedIn.value = r.seed;
    locIn.value = r.locale || "";
    archIn.value = r.archetype || "";
    genIn.value = r.gender || "";
    ageMin.value = r.age_min ?? "";
    ageMax.value = r.age_max ?? "";
    syncDesc();
  };

  // Re-pressing "Forge" without touching the seed means "someone else": the
  // seed shown is the one just forged, so none is sent and the server rolls a
  // fresh one. A seed typed by hand, rolled with the die, or arriving from a
  // link / the pool is honoured as-is.
  let seedTouched = !!q.get("seed");
  seedIn.addEventListener("input", () => { seedTouched = true; });
  const recipeFromForm = (keepRerolls) => ({
    seed: (keepRerolls || seedTouched || !forgeState.lastSeed ? seedIn.value.trim() : "") || undefined,
    locale: locIn.value || undefined,
    archetype: archIn.value || undefined,
    gender: genIn.value || undefined,
    age_min: ageMin.value ? Number(ageMin.value) : undefined,
    age_max: ageMax.value ? Number(ageMax.value) : undefined,
    as_of: keepRerolls ? forgeState.recipe?.as_of : undefined,
    rerolls: keepRerolls ? forgeState.recipe?.rerolls || {} : {},
  });

  const line = async (text, cls = "") => { log.append(h("li", { class: cls }, text)); log.scrollTop = log.scrollHeight; await sleep(55); };

  const renderHistory = () => {
    const { entries, pos } = forgeState.hist;
    clear(histBar);
    if (entries.length < 2) { histBar.hidden = true; return; }
    histBar.hidden = false;
    const chips = h("ol", { class: "hist-list" }, entries.map((e, i) => h("li", {},
      h("button", { class: "hist-chip", "aria-current": i === pos ? "true" : null, title: `${e.p.identity.first_name} ${e.p.identity.last_name} · ${e.p.recipe.seed}`, onclick: () => goTo(i) },
        h("b", {}, String(i + 1).padStart(2, "0")), ` ${e.p.identity.first_name} ${e.p.identity.last_name}`, e.dirty ? h("span", { class: "hist-dirty" }, " ✎") : null))));
    histBar.append(
      h("button", { class: "btn btn-sm", id: "hist-prev", disabled: pos <= 0, onclick: () => goTo(pos - 1), title: t("hist_prev") }, "◀ " + t("hist_prev")),
      h("span", { class: "hist-pos mono" }, `${pos + 1} / ${entries.length}`),
      h("button", { class: "btn btn-sm", id: "hist-next", disabled: pos >= entries.length - 1, onclick: () => goTo(pos + 1), title: t("hist_next") }, t("hist_next") + " ▶"),
      chips);
    chips.querySelector('[aria-current="true"]')?.scrollIntoView({ block: "nearest", inline: "center" });
  };

  const goTo = (i) => {
    const h0 = forgeState.hist;
    if (i < 0 || i >= h0.entries.length || i === h0.pos || busy) return;
    seq++; // cancel any in-flight forge
    h0.pos = i;
    saveHistory();
    const { p, dirty } = h0.entries[i];
    Object.assign(forgeState, { persona: p, recipe: p.recipe, dirty, lastSeed: p.recipe.seed });
    fillForm(p.recipe);
    seedTouched = false;
    clear(log).append(h("li", { class: "dim" }, `↺ ${t("hist_restored")} ${i + 1}/${h0.entries.length} — ${p.recipe.seed}`), h("li", { class: "ok" }, `✓ ${L(p.codename)} · ${p.identity.first_name} ${p.identity.last_name}`));
    render();
  };

  const render = () => {
    renderHistory();
    const p = forgeState.persona;
    if (!p) return;
    clear(out).append(dossier(p, {
      dirty: forgeState.dirty,
      archetypeLabel: archLabel,
      edit: async (path, value) => {
        try {
          forgeState.persona = await api.check(applyEdit(forgeState.persona, path, value));
          forgeState.dirty = true;
          historyAmend(forgeState.persona, true);
          render();
        } catch (e) { toast(e.message, true); }
      },
      reroll: async (sec) => {
        const r = { ...forgeState.persona.recipe, rerolls: { ...forgeState.persona.recipe.rerolls } };
        r.rerolls[sec] = (r.rerolls[sec] || 0) + 1;
        const face = forgeState.persona.face;
        try {
          let p2 = await api.forge(r);
          if (face) p2 = await api.check({ ...p2, face });
          forgeState.persona = p2;
          forgeState.recipe = p2.recipe;
          historyAmend(p2, forgeState.dirty);
          render();
          document.getElementById(`s-${sec === "appearance" ? "identity" : sec === "personality" || sec === "interests" ? "mind" : sec === "behavior" ? "rhythm" : sec}`)?.scrollIntoView({ block: "start" });
        } catch (e) { toast(e.message, true); }
      },
      setFace: async (f) => {
        forgeState.persona = await api.check({ ...forgeState.persona, face: { id: f.id, source: f.source } });
        historyAmend(forgeState.persona, forgeState.dirty);
        render();
      },
      actions: [
        h("button", { class: "btn btn-amber btn-sm", onclick: async (e) => {
          e.currentTarget.disabled = true;
          try {
            const rec = await api.create(forgeState.persona);
            toast(t("saved_toast"));
            ctx.refreshNav();
            location.hash = `#/p/${rec.id}`;
          } catch (err) { toast(err.message, true); e.currentTarget.disabled = false; }
        } }, "⬒ " + t("save")),
        ...commonActions(p, { exportMd: () => api.exportMd(forgeState.persona, lang()), quiz: (round) => api.quiz(forgeState.persona, round) }),
        h("button", { class: "btn btn-sm btn-ghost", onclick: async () => {
          const r = forgeState.persona.recipe;
          const url = `${location.origin}/#/?seed=${encodeURIComponent(r.seed)}&locale=${r.locale}${r.archetype ? "&archetype=" + r.archetype : ""}`;
          try { await navigator.clipboard.writeText(url); toast(t("copied")); } catch { toast(url); }
        } }, "⧉ " + t("permalink")),
      ],
    }));
  };

  let busy = false;
  let seq = 0;
  const doForge = async (keep = false) => {
    if (busy) return;
    busy = true;
    const my = ++seq;
    go.disabled = true;
    clear(log);
    try {
      const p = await api.forge(recipeFromForm(keep));
      if (my !== seq) return;
      forgeState.persona = p;
      forgeState.recipe = p.recipe;
      forgeState.dirty = false;
      forgeState.lastSeed = p.recipe.seed;
      seedIn.value = p.recipe.seed;
      seedTouched = false;
      historyPush(p);
      const errors = p.coherence.checks.filter((c) => c.severity === "error").length;
      await line(`$ simulacre forge --seed ${p.recipe.seed} --locale ${p.locale.code} --archetype ${p.archetype}`, "dim");
      await line(`› ${t("log_identity")} ……… ${p.identity.first_name} ${p.identity.last_name}, ${p.identity.age} — ${p.residence.place.city}`, "ok");
      await line(`› ${t("log_life")} ……… ${p.education.length} + ${p.career.length} ${t("log_jobs")}`, "ok");
      await line(`› ${t("log_family")} ……… ${lab("relationship", p.family.relationship)}, ${p.family.children.length + p.family.siblings.length + p.family.pets.length} +`, "ok");
      await line(`› ${t("log_mind")} ……… ${lab("ptype", p.personality.type_key)} [${p.personality.ocean.join(" ")}]`, "ok");
      await line(`› ${t("log_rhythm")} ……… ${lab("chronotype", p.behavior.chronotype)} ${p.behavior.wake}→${p.behavior.sleep}`, "ok");
      await line(`› ${t("log_digital")} ……… ${p.accounts.length} ${t("log_accounts")}: ${p.accounts.map((a) => a.platform).join(", ")}`, "ok");
      await line(`› ${t("log_voice")} ……… ${p.voice.length} ${t("log_posts")}`, "ok");
      await line(`› ${t("log_check")} ……… ${p.coherence.score}/100 · ${errors} ${t("log_errors")}`, errors ? "hit" : p.coherence.score < 85 ? "warn" : "ok");
      await line(`✓ ${t("log_done")}: ${L(p.codename)}`, "hit");
      render();
      out.scrollIntoView({ behavior: "smooth", block: "start" });
    } catch (e) {
      await line(`✗ ${e.message}`, "hit");
      toast(e.message, true);
    } finally {
      busy = false;
      go.disabled = false;
    }
  };

  const n = (num, label) => h("span", {}, h("i", {}, num), label);
  const form = h("form", { class: "forge-panel", onsubmit: (e) => { e.preventDefault(); doForge(false); } },
    h("header", { class: "form-head" }, h("b", {}, t("form_title")), h("span", { class: "mono" }, "Cerfa SIM-01 · ICAO 9303 SPECIMEN")),
    h("label", { class: "field f-seed" }, n("01", t("seed")), h("div", { class: "input-group" }, seedIn, dice)),
    h("label", { class: "field" }, n("02", t("locale")), locIn),
    h("label", { class: "field" }, n("03", t("gender")), genIn),
    h("label", { class: "field f-arch" }, n("04", t("archetype")), archIn),
    h("label", { class: "field" }, n("05", t("age_min")), ageMin),
    h("label", { class: "field" }, n("06", t("age_max")), ageMax),
    h("div", { class: "f-go" }, desc, go));
  form.addEventListener("keydown", (e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); doForge(false); } });

  root.append(
    h("section", { class: "hero" },
      h("div", { class: "hero-copy" },
        h("p", { class: "eyebrow" }, h("span", { class: "rec", "aria-hidden": "true" }), t("eyebrow")),
        h("h1", { class: "wordmark" }, h("span", { class: "wm-l" }, "Simulacre"), h("span", { class: "wm-s", "aria-hidden": "true" }, "Simulacre")),
        h("p", { class: "tagline", html: t("tagline") }),
        h("p", { class: "lede" }, t("lede")),
        h("p", { class: "hero-stats" },
          h("span", {}, h("b", {}, "1,1 × 10¹²"), ` ${t("stat_seeds")}`),
          h("span", {}, h("b", {}, "30+"), ` ${t("stat_checks")}`),
          h("span", {}, h("b", {}, meta.locales.length), ` ${t("stat_locales")}`)),
        h("p", { class: "uv-hint" }, h("span", { class: "kbd" }, "U"), " ", t("uv_hint"))),
      h("div", { class: "hero-puppet" }, marionette())),
    h("div", { class: "forge-row" },
      form,
      h("div", { class: "terminal", role: "log", "aria-live": "polite" },
        h("div", { class: "terminal-bar" }, h("b", {}, t("receipt")), h("span", { class: "mono" }, new Date().toISOString().slice(0, 10))), log)),
    histBar, out);

  if (q.get("seed") || !forgeState.persona) doForge(false).then(() => { if (q.get("seed")) history.replaceState(null, "", "#/"); });
  else { render(); log.append(h("li", { class: "dim" }, `$ simulacre forge --seed ${forgeState.recipe.seed}`)); }
}

// ═════════════════════════════════════ POOL ══════════════════════════════════
const poolState = { pool: Math.random().toString(36).slice(2, 10), locale: "", archetype: "", gender: "" };

function personCard(item, href, extra = []) {
  const sil = h("span", { class: "sil" }, item.face ? h("img", { src: `/faces/${item.face.id}.jpg`, alt: "", loading: "lazy" }) : silhouette(item.seed, initials(item.name)));
  return h("a", { class: "pcard", href },
    h("span", { class: "score", vars: { color: scoreColor(item.score) } }, item.score),
    h("div", { class: "pcard-head" }, sil, h("div", {}, h("span", { class: "cn" }, L(item.codename)), h("b", {}, item.name), h("span", { class: "meta" }, `${flag(item.country_code)} ${item.age} · ${item.city}`))),
    h("div", { class: "meta" }, item.title || lab("status", item.occupation)),
    h("div", { class: "chips" }, item.interests.slice(0, 3).map((i) => h("span", { class: "chip" }, L(i)))),
    h("div", { class: "chips" }, item.platforms.map((p) => pbadge(p))),
    extra);
}

export function poolView(root) {
  const meta = ctx.meta;
  const grid = h("div", { class: "cards" });
  const sentinel = h("div", { class: "sentinel" }, t("loading_more"));
  let offset = 0, busy = false;
  const load = async () => {
    if (busy) return;
    busy = true;
    try {
      const res = await api.pool({ pool: poolState.pool, offset, limit: 24, locale: poolState.locale, archetype: poolState.archetype, ...(poolState.gender ? { gender: poolState.gender } : {}) });
      for (const it of res.items) {
        const href = `#/?seed=${encodeURIComponent(it.seed)}&locale=${it.locale}${poolState.archetype ? "&archetype=" + poolState.archetype : ""}`;
        grid.append(personCard(it, href));
      }
      offset = res.next;
    } catch (e) { toast(e.message, true); }
    busy = false;
  };
  const reset = () => { offset = 0; clear(grid); load(); };
  const sel = (key, options) => {
    const s = h("select", { class: "input" }, options);
    s.value = poolState[key];
    s.addEventListener("change", () => { poolState[key] = s.value; reset(); });
    return s;
  };
  root.append(section({
    id: "pool", num: "∞", title: t("pool_title"), sub: t("pool_sub"),
    body: h("div", {},
      h("div", { class: "toolbar" },
        h("label", { class: "field" }, h("span", {}, t("locale")), sel("locale", [opt("", t("any")), meta.locales.map((l) => opt(l.code, `${flag(l.country_code)} ${L(l.country)}`))])),
        h("label", { class: "field" }, h("span", {}, t("archetype")), sel("archetype", [opt("", t("any")), meta.archetypes.map((a) => opt(a.key, L(a.label)))])),
        h("label", { class: "field" }, h("span", {}, t("gender")), sel("gender", [opt("", t("any")), opt("female", t("female")), opt("male", t("male"))])),
        h("button", { class: "btn", onclick: () => { poolState.pool = Math.random().toString(36).slice(2, 10); reset(); } }, "⟳ " + t("pool_new"))),
      grid, sentinel),
  }));
  const io = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) load(); }, { rootMargin: "600px" });
  io.observe(sentinel);
  load();
  return () => io.disconnect();
}

// ═════════════════════════════════════ VAULT ═════════════════════════════════
let vaultFilter = "";
export async function vaultView(root) {
  const [list, stats] = await Promise.all([api.list(), api.stats()]);
  const statuses = ctx.meta.statuses;
  const grid = h("div", { class: "cards" });
  const draw = () => {
    clear(grid);
    const items = list.filter((r) => !vaultFilter || r.status === vaultFilter);
    if (!items.length) grid.append(h("div", { class: "empty" }, h("b", {}, t("vault_empty")), t("vault_empty_sub")));
    for (const r of items) grid.append(personCard(r, `#/p/${r.id}`, [h("div", { class: "meta" }, h("span", { class: `pill s-${r.status}` }, t(`st_${r.status}`)), ` ${t("updated")} ${r.updated_at.slice(0, 10)}`)]));
  };
  const chips = h("div", { class: "toolbar", role: "group" }, ["", ...statuses].map((s) => {
    const b = h("button", { class: "chip-btn", "aria-pressed": String(vaultFilter === s), onclick: () => { vaultFilter = s; [...chips.children].forEach((c) => c.setAttribute("aria-pressed", String(c === b))); draw(); } }, s ? t(`st_${s}`) : t("all"));
    return b;
  }));
  root.append(section({
    id: "vault", num: "⬒", title: t("vault_title"), sub: t("vault_sub"),
    body: h("div", {},
      h("ul", { class: "stats" },
        h("li", {}, h("b", {}, stats.total), h("span", {}, t("total"))),
        statuses.map((s) => h("li", {}, h("b", {}, stats.by_status[s] || 0), h("span", {}, t(`st_${s}`)))),
        h("li", {}, h("b", { vars: { color: scoreColor(stats.avg_coherence) } }, stats.avg_coherence), h("span", {}, t("avg_coherence")))),
      chips, grid),
  }));
  draw();
}

// ═════════════════════════════════════ SAVED PERSONA ═════════════════════════
export async function personaView(root, id) {
  let rec = await api.get(id);
  let journal = await api.journal(id);
  const left = h("div");
  const side = h("aside", { class: "sticky" });

  const persist = async (persona) => {
    try {
      rec = await api.update(id, persona);
      journal = await api.journal(id);
      toast(t("updated_toast"));
      draw();
    } catch (e) { toast(e.message, true); }
  };

  const draw = () => {
    const p = rec.persona;
    clear(left).append(dossier(p, {
      savedId: id,
      archetypeLabel: archLabel,
      edit: async (path, value) => { try { await persist(applyEdit(rec.persona, path, value)); } catch (e) { toast(e.message, true); } },
      reroll: async (sec) => {
        const r = { ...p.recipe, rerolls: { ...p.recipe.rerolls, [sec]: (p.recipe.rerolls[sec] || 0) + 1 } };
        try { const np = await api.forge(r); await persist({ ...np, face: p.face }); } catch (e) { toast(e.message, true); }
      },
      setFace: (f) => persist({ ...rec.persona, face: { id: f.id, source: f.source } }),
      actions: commonActions(p, { exportMd: () => api.savedMd(id, lang()), quiz: (round) => api.savedQuiz(id, round) }),
    }));

    const kind = h("select", { class: "input" }, ["note", "action", "contact", "incident"].map((k) => opt(k, t(`k_${k}`))));
    const text = h("textarea", { class: "input", placeholder: t("journal_ph"), maxlength: 4000 });
    const lifecycle = h("div", { class: "lifecycle", role: "group", "aria-label": t("lifecycle") }, ctx.meta.statuses.map((s) => h("button", {
      class: s === "burned" ? "burn" : "", "aria-pressed": String(rec.status === s),
      onclick: async () => {
        if (s === rec.status) return;
        if (s === "burned" && !confirm(t("burn_confirm"))) return;
        try { rec = await api.status(id, s); journal = await api.journal(id); ctx.refreshNav(); draw(); } catch (e) { toast(e.message, true); }
      },
    }, t(`st_${s}`))));
    clear(side).append(
      h("a", { class: "btn btn-ghost btn-sm", href: "#/vault" }, t("back_vault")),
      h("div", { class: "panel" }, h("p", { class: "label" }, t("lifecycle")), lifecycle),
      h("div", { class: "panel" },
        h("p", { class: "label" }, t("journal")),
        h("form", { onsubmit: async (e) => {
          e.preventDefault();
          if (!text.value.trim()) return;
          try { await api.journalAdd(id, kind.value, text.value); text.value = ""; journal = await api.journal(id); draw(); } catch (err) { toast(err.message, true); }
        } }, h("div", { class: "field" }, kind), h("div", { class: "field" }, text), h("button", { class: "btn btn-sm" }, "+ " + t("add"))),
        h("ul", { class: "journal" }, journal.map((j) => h("li", { class: `k-${j.kind}` }, h("div", { class: "jh" }, h("b", {}, j.kind), h("span", {}, j.at.replace("T", " ").replace("Z", ""))), j.text)))),
      h("div", { class: "panel" },
        h("p", { class: "label" }, "Export"),
        h("div", { class: "actions" },
          h("button", { class: "btn btn-sm", onclick: async () => { download(`simulacre-${id}.json`, await api.savedJson(id), "application/json"); } }, "{ } JSON + journal"),
          h("button", { class: "btn btn-sm btn-danger", onclick: async () => {
            if (!confirm(t("confirm_delete"))) return;
            await api.remove(id);
            ctx.refreshNav();
            location.hash = "#/vault";
          } }, "✕ " + t("delete")))));
  };
  root.append(h("div", { class: "ops" }, left, side));
  draw();
}

// ═════════════════════════════════════ CONTAMINATION ═════════════════════════
export async function contaminationView(root) {
  const data = await api.contamination();
  const ids = Object.keys(data.names).filter((k) => data.names[k].status !== "burned");
  const body = h("div");
  root.append(section({ id: "contam", num: "⚠", title: t("contam_title"), sub: t("contam_sub"), body }));
  if (ids.length < 2) { body.append(h("div", { class: "empty" }, h("b", {}, t("contam_empty")))); return; }
  const risk = new Map();
  for (const pr of data.pairs) { risk.set(`${pr.a}|${pr.b}`, pr); risk.set(`${pr.b}|${pr.a}`, pr); }
  const label = (k) => `${data.names[k].name}`;
  const matrix = h("div", { class: "matrix", vars: { "grid-template-columns": `auto repeat(${ids.length}, 34px)` } });
  matrix.append(h("span"));
  ids.forEach((k) => matrix.append(h("span", { class: "mh", title: label(k) }, label(k))));
  const pairs = h("div", { class: "pairs" });
  const pairEls = new Map();
  ids.forEach((a) => {
    matrix.append(h("a", { class: "ml", href: `#/p/${a}`, title: L(data.names[a].codename) }, label(a)));
    ids.forEach((b) => {
      if (a === b) { matrix.append(h("span", { class: "mc self" })); return; }
      const pr = risk.get(`${a}|${b}`);
      const v = pr?.risk ?? 0;
      matrix.append(h("button", { class: `mc${v < 20 ? " low" : ""}`, vars: { "--v": v }, title: `${label(a)} × ${label(b)} — ${v}`, onclick: () => {
        const el = pairEls.get(pr);
        el?.scrollIntoView({ behavior: "smooth", block: "center" });
        el?.classList.remove("flash"); void el?.offsetWidth; el?.classList.add("flash");
      } }, v || ""));
    });
  });
  for (const pr of data.pairs) {
    const el = h("article", { class: "pair", vars: { "--v": pr.risk } },
      h("div", { class: "pair-head" }, h("b", {}, `${label(pr.a)} × ${label(pr.b)}`), h("span", { class: "risk", vars: { color: scoreColor(100 - pr.risk) } }, `${t("risk")} ${pr.risk}`)),
      pr.links.length ? h("ul", { class: "checks" }, pr.links.map((l) => h("li", { class: `s-${l.severity}` }, h("span", { class: "sev" }, t(`sev_${l.severity}`)), h("span", {}, L(l.message))))) : h("p", { class: "muted small" }, t("no_links")));
    pairEls.set(pr, el);
    pairs.append(el);
  }
  body.append(h("div", { class: "panel matrix-wrap" }, matrix), pairs);
}

// ═════════════════════════════════════ DOCTRINE ══════════════════════════════
const DOCTRINE = {
  fr: [
    ["À quoi sert SIMULACRE", ["Les enquêteurs OSINT, journalistes, chercheurs en sécurité et formateurs utilisent des « sock puppets » : des identités fictives qui leur permettent d'observer des communautés en ligne sans exposer leur vraie identité ni celle de leur organisation.", "Une identité fragile met l'enquête et l'enquêteur en danger. SIMULACRE produit des légendes cohérentes, mémorisables et suivies dans le temps, plutôt qu'un simple nom tiré au hasard."]],
    ["Ce que SIMULACRE refuse de faire", ["Pas de mineurs : 18 ans minimum, imposé par le moteur.", "Pas d'adresse postale : seulement la ville et le quartier, pour ne jamais pointer vers le domicile d'une vraie personne.", "Pas de numéro de sécurité sociale, de carte d'identité, d'IBAN ni de carte bancaire.", "Téléphones uniquement dans des plages réservées à la fiction (ARCEP, Ofcom, NANP 555-01xx). Ailleurs : aucun numéro.", "Aucune création automatique de compte, aucun contournement de captcha ou de vérification : la plateforme prépare, l'humain agit et assume.", "Visages générés par IA uniquement ; jamais la photo d'une personne réelle."]],
    ["Comment ça marche", ["Chaque légende est une fonction pure de sa recette (graine, pays, archétype, contraintes). La même graine redonne toujours la même personne : rien à stocker pour la retrouver, et 1,1 billion de graines possibles.", "Chaque section a son propre flux aléatoire : on peut retirer la voix ou l'apparence sans toucher au reste.", "Le moteur de cohérence rejoue plus de 30 contrôles à chaque modification : chronologie, âges, règles de pseudo par plateforme, dates d'inscription plausibles, rythme en ligne compatible avec l'emploi du temps…", "L'analyse de contamination compare toutes les légendes du coffre deux à deux, jusqu'à la corrélation de leurs heures de connexion."]],
    ["Règles d'engagement", null, ["Agir dans un cadre légal et avec un mandat clair (mission, rédaction, programme de recherche).", "Observer plutôt qu'interagir ; ne jamais inciter à commettre une infraction.", "Ne pas usurper une personne réelle ni une organisation existante.", "Respecter les CGU des plateformes et la vie privée des personnes observées.", "Tout consigner dans le journal : il protège l'enquêteur autant que l'enquête.", "Une légende grillée est retirée, jamais recyclée."]],
  ],
  en: [
    ["What SIMULACRE is for", ["OSINT investigators, journalists, security researchers and trainers use sock puppets: fictional identities that let them observe online communities without exposing themselves or their organisation.", "A flimsy identity endangers both the investigation and the investigator. SIMULACRE produces coherent, memorable, tracked legends rather than a random name."]],
    ["What SIMULACRE refuses to do", ["No minors: 18+ enforced by the engine.", "No street address: only city and neighbourhood, so it never points at a real person's home.", "No national ID, social security, bank or card numbers.", "Phone numbers only from ranges reserved for fiction (ARCEP, Ofcom, NANP 555-01xx). Elsewhere: none.", "No automated account creation, no captcha or verification bypass: the platform prepares, a human acts and is accountable.", "AI-generated faces only; never a real person's photo."]],
    ["How it works", ["A legend is a pure function of its recipe (seed, country, archetype, constraints). The same seed always yields the same person: nothing to store to find it again, and 1.1 trillion possible seeds.", "Each section has its own random stream: reroll the voice or the appearance without touching the rest.", "The coherence engine replays 30+ checks on every edit: timeline, ages, per-platform handle rules, plausible sign-up dates, online rhythm vs work schedule…", "The contamination analysis compares every pair of legends in the vault, down to the correlation of their online hours."]],
    ["Rules of engagement", null, ["Operate within the law and a clear mandate (engagement, newsroom, research programme).", "Observe rather than interact; never incite an offence.", "Do not impersonate a real person or an existing organisation.", "Respect platform terms and the privacy of the people observed.", "Log everything in the journal: it protects the investigator as much as the investigation.", "A burned legend is retired, never recycled."]],
  ],
};

export function doctrineView(root) {
  const blocks = DOCTRINE[lang()].map(([title, paras, rules], i) => h("div", { class: "panel" },
    h("h3", {}, title),
    paras ? (i === 1 ? h("ul", { class: "list-clean" }, paras.map((x) => h("li", {}, x))) : paras.map((x) => h("p", {}, x))) : null,
    rules ? h("ol", { class: "rule-list" }, rules.map((x) => h("li", {}, x))) : null));
  root.append(section({ id: "doctrine", num: "§", title: t("doctrine_title"), sub: t("doctrine_sub"), body: h("div", { class: "doctrine" }, blocks) }));
}

