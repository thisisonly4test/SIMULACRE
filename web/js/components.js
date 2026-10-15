import { api } from "./api.js";
import { L, lab, t, yrs } from "./i18n.js";
import { clear, h, s, scoreColor, toast } from "./util.js";

// ── Coherence gauge ─────────────────────────────────────────────────────────
export function gauge(value) {
  const r = 38, c = 2 * Math.PI * r;
  const val = s("circle", { class: "val", cx: 46, cy: 46, r, stroke: scoreColor(value), "stroke-dasharray": c, "stroke-dashoffset": c });
  const svg = s("svg", { class: "gauge", viewBox: "0 0 92 92", role: "img", "aria-label": `${t("coherence")} ${value}/100` },
    s("circle", { class: "track", cx: 46, cy: 46, r }), val, s("text", { x: 46, y: 47 }, String(value)));
  requestAnimationFrame(() => requestAnimationFrame(() => val.setAttribute("stroke-dashoffset", c * (1 - value / 100))));
  return svg;
}

// ── Big Five radar ──────────────────────────────────────────────────────────
export function radar(ocean) {
  const keys = ["o", "c", "e", "a", "n"];
  const cx = 110, cy = 110, R = 80;
  const pt = (i, v) => {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / 5;
    return [cx + Math.cos(a) * R * v, cy + Math.sin(a) * R * v];
  };
  const ring = (v) => keys.map((_, i) => pt(i, v).join(",")).join(" ");
  const svg = s("svg", { class: "radar", viewBox: "0 0 220 220", role: "img", "aria-label": "Big Five" });
  for (const v of [0.25, 0.5, 0.75, 1]) svg.append(s("polygon", { class: "web", points: ring(v) }));
  keys.forEach((_, i) => { const [x, y] = pt(i, 1); svg.append(s("line", { x1: cx, y1: cy, x2: x, y2: y })); });
  svg.append(s("polygon", { class: "shape", points: ocean.map((v, i) => pt(i, v / 100).join(",")).join(" ") }));
  keys.forEach((k, i) => {
    const [x, y] = pt(i, 1.2);
    svg.append(s("text", { x, y: y + 3 }, k.toUpperCase() + " " + ocean[i]));
  });
  return svg;
}

export function oceanBars(ocean) {
  return h("div", { class: "bars" }, ["o", "c", "e", "a", "n"].map((k, i) =>
    h("div", { class: "bar" }, h("span", {}, lab("ocean", k)), h("div", { class: "track" }, h("div", { class: "fill", vars: { "--v": ocean[i] } })), h("b", {}, ocean[i]))));
}

// ── Activity heatmap ────────────────────────────────────────────────────────
export function heatmap(matrix) {
  const grid = h("div", { class: "heat", role: "img", "aria-label": t("s_rhythm") });
  grid.append(h("span"));
  for (let hr = 0; hr < 24; hr++) grid.append(h("span", { class: "hh" }, hr % 3 === 0 ? String(hr).padStart(2, "0") : ""));
  matrix.forEach((row, d) => {
    grid.append(h("span", { class: "hl" }, t("days")[d]));
    row.forEach((v, hr) => grid.append(h("span", { class: `cell${v === 0 ? " zero" : ""}`, vars: { "--v": (v / 100).toFixed(2) }, title: `${t("days")[d]} ${String(hr).padStart(2, "0")}:00 — ${v}%` })));
  });
  return grid;
}

// ── Collapsible numbered section ──────────────────────────────────────────
const collapsedKey = "simulacre.collapsed";
const collapsed = new Set((() => { try { return JSON.parse(localStorage.getItem(collapsedKey) || "[]"); } catch { return []; } })());

export function section({ id, num, title, sub, tools = [], body }) {
  const sec = h("section", { class: `section${collapsed.has(id) ? " collapsed" : ""}`, id, "aria-labelledby": `${id}-t` });
  const btn = h("button", { class: "collapse-btn", "aria-expanded": String(!collapsed.has(id)), "aria-controls": `${id}-b`, title: title }, h("span", {}, "▾"));
  btn.addEventListener("click", () => {
    const on = sec.classList.toggle("collapsed");
    btn.setAttribute("aria-expanded", String(!on));
    on ? collapsed.add(id) : collapsed.delete(id);
    try { localStorage.setItem(collapsedKey, JSON.stringify([...collapsed])); } catch { /* ignore */ }
  });
  sec.append(
    h("h2", { class: "section-title", id: `${id}-t` }, h("span", { class: "num" }, num), title, btn, tools.length ? h("span", { class: "tools" }, tools) : null),
    h("div", { class: "section-body", id: `${id}-b` }, h("div", { class: "section-body-inner" }, sub ? h("p", { class: "section-sub" }, sub) : null, body)),
  );
  return sec;
}

// ── Inline editing ──────────────────────────────────────────────────────────
/** A value that turns into an input on click; `commit(newValue)` on Enter/blur. */
export function editable(value, commit, { type = "text", multiline = false } = {}) {
  const span = h("span", { class: "editable", tabindex: 0, role: "button", title: t("edit_hint") }, value || "—");
  const start = () => {
    const input = multiline ? h("textarea", { class: "edit-input input", rows: 3 }) : h("input", { class: "edit-input", type });
    input.value = value ?? "";
    span.replaceWith(input);
    input.focus();
    let done = false;
    const finish = (save) => {
      if (done) return;
      done = true;
      input.replaceWith(span);
      if (save && input.value !== (value ?? "")) commit(input.value);
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !(multiline && e.shiftKey)) { e.preventDefault(); finish(true); }
      if (e.key === "Escape") finish(false);
    });
    input.addEventListener("blur", () => finish(true));
  };
  span.addEventListener("click", start);
  span.addEventListener("keydown", (e) => { if (e.key === "Enter") start(); });
  return span;
}

// ── Modal ───────────────────────────────────────────────────────────────────
export function modal(title, body, { onClose } = {}) {
  const dlg = h("dialog", { "aria-label": title },
    h("div", { class: "modal-head" }, h("h3", {}, title), h("button", { class: "btn btn-ghost btn-icon x", "aria-label": "close", onclick: () => dlg.close() }, "✕")),
    h("div", { class: "modal-body" }, body));
  dlg.addEventListener("close", () => { dlg.remove(); onClose?.(); });
  dlg.addEventListener("click", (e) => { if (e.target === dlg) dlg.close(); });
  document.body.append(dlg);
  dlg.showModal();
  return dlg;
}

// ── Face picker ─────────────────────────────────────────────────────────────
export function facePicker(persona, onPick, ownId = null) {
  const id = persona.identity;
  const grid = h("div", { class: "faces" });
  const status = h("p", { class: "small muted" });
  const target = h("div", { class: "target-box" },
    h("b", {}, `${t("target")} :`),
    h("span", {}, `${id.gender === "female" ? t("female") : t("male")}, ${yrs(id.age)}`),
    h("span", { class: "muted" }, `${lab("hair_color", persona.appearance.hair_color)} · ${lab("eyes", persona.appearance.eyes)}${persona.appearance.glasses ? " · 👓" : ""}`));

  const addFace = async (f) => {
    const users = (await api.faceUsers(f.id).catch(() => [])).filter((u) => u !== ownId);
    const btn = h("button", { class: "face-opt", title: f.id, onclick: () => { onPick(f); dlg.close(); } }, h("img", { src: f.url, alt: "", loading: "lazy" }));
    if (users.length) btn.append(h("span", { class: "used" }, t("faces_used")));
    const firstLoaded = grid.querySelector(".face-opt:not(.loading)");
    grid.insertBefore(btn, firstLoaded);
  };
  // Faces stream in one by one (staggered: the upstream generator hands the
  // same picture to simultaneous requests); duplicates are silently retried.
  const seen = new Set();
  const draw = async () => {
    drawBtn.disabled = true;
    status.textContent = "";
    const placeholders = Array.from({ length: 6 }, () => h("div", { class: "face-opt loading" }, "…"));
    grid.prepend(...placeholders);
    let failures = 0;
    const one = async (slot, delay, retries = 2) => {
      await new Promise((r) => setTimeout(r, delay));
      try {
        const [f] = (await api.facesFetch(1)).faces;
        if (seen.has(f.id) && retries > 0) return one(slot, 600, retries - 1);
        slot.remove();
        if (!seen.has(f.id)) { seen.add(f.id); await addFace(f); }
      } catch (e) {
        slot.remove();
        if (++failures === placeholders.length) status.textContent = `${t("faces_fail")} (${e.message})`;
      }
    };
    await Promise.all(placeholders.map((slot, i) => one(slot, i * 450)));
    drawBtn.disabled = false;
  };
  const drawBtn = h("button", { class: "btn btn-primary", onclick: draw }, "⟳ " + t("faces_draw"));
  const file = h("input", { type: "file", accept: "image/jpeg,image/png,image/webp", hidden: true });
  file.addEventListener("change", async () => {
    if (!file.files[0]) return;
    try { await addFace(await api.faceUpload(file.files[0])); } catch (e) { toast(e.message, true); }
  });
  const body = h("div", {},
    h("p", { class: "muted small" }, t("faces_sub")),
    target,
    h("div", { class: "toolbar" }, drawBtn, h("button", { class: "btn", onclick: () => file.click() }, "⇪ " + t("faces_upload")), file),
    status, grid,
    h("p", { class: "small dim" }, "⚠ " + t("faces_note")));
  const dlg = modal(t("faces_title"), body);
  draw();
}

// ── Interrogation quiz ──────────────────────────────────────────────────────
export function quizModal(load) {
  const body = h("div", {});
  let round = 0;
  const dlg = modal(t("quiz_title"), body);
  const run = async () => {
    clear(body).append(h("p", { class: "muted" }, "…"));
    const qs = await load(round++);
    let i = 0, right = 0, times = [];
    const show = () => {
      clear(body);
      if (i >= qs.length) {
        const avg = times.reduce((a, b) => a + b, 0) / Math.max(times.length, 1) / 1000;
        body.append(h("div", { class: "quiz-result" },
          h("div", { class: "big", vars: {} }, `${right}/${qs.length}`),
          h("p", {}, t("quiz_score")),
          h("p", { class: "mono" }, `${avg.toFixed(1)} s — ${t("quiz_time")}`),
          h("p", { class: avg > 4 || right < qs.length * 0.8 ? "dirty" : "" }, avg > 4 || right < qs.length * 0.8 ? t("quiz_slow") : t("quiz_good")),
          h("button", { class: "btn btn-primary", onclick: run }, "⟳ " + t("again"))));
        return;
      }
      const q = qs[i];
      const t0 = performance.now();
      const choices = h("div", { class: "quiz-choices" });
      q.choices.forEach((c, ci) => choices.append(h("button", {
        onclick: (e) => {
          times.push(performance.now() - t0);
          [...choices.children].forEach((b, bi) => { b.disabled = true; if (bi === q.answer) b.classList.add("right"); });
          if (ci === q.answer) right++; else e.currentTarget.classList.add("wrong");
          setTimeout(() => { i++; show(); }, ci === q.answer ? 450 : 1300);
        },
      }, c)));
      body.append(
        h("div", { class: "quiz-progress" }, h("div", { vars: { "--v": (i / qs.length) * 100 } })),
        h("p", { class: "label" }, `${i + 1} / ${qs.length} — ${t("quiz_sub")}`),
        h("p", { class: "quiz-q" }, L(q.prompt)),
        choices);
    };
    show();
  };
  run().catch((e) => { toast(e.message, true); dlg.close(); });
}

// ── Token prompt ────────────────────────────────────────────────────────────
export function tokenPrompt() {
  return new Promise((resolve) => {
    const input = h("input", { class: "input", type: "password", autocomplete: "current-password" });
    const form = h("form", { onsubmit: (e) => { e.preventDefault(); dlg.close(); } },
      h("p", { class: "muted" }, t("token_sub")), h("div", { class: "input-group" }, input, h("button", { class: "btn btn-primary" }, t("token_ok"))));
    const dlg = modal(t("token_title"), form, { onClose: () => resolve(input.value) });
    input.focus();
  });
}
