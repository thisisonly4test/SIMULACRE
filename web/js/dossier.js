// The dossier: one persona, rendered as an operator's case file. Shared by the
// forge (unsaved) and the vault (saved) views.

import { faceUrl } from "./api.js";
import { editable, facePicker, gauge, heatmap, oceanBars, quizModal, radar, section } from "./components.js";
import { L, lab, t, yrs } from "./i18n.js";
import { mrz } from "./security.js";
import { clone, download, flag, h, initials, pbadge, PLATFORM_COLORS, scoreColor, setPath, silhouette, toast } from "./util.js";

/**
 * @param {object} p persona
 * @param {object} o { edit(path, value) → Promise, reroll(section), setFace(face), actions: Node[], dirty, savedId, quiz(round) }
 */
export function dossier(p, o) {
  const id = p.identity;
  const wrap = h("div", { class: "dossier-root" });
  const year = Number(p.recipe.as_of.slice(0, 4));
  const ed = (path, value, opts) => (o.edit ? editable(value, (v) => o.edit(path, v), opts) : h("span", {}, value || "—"));
  const rerollBtn = (sec) => (o.reroll ? h("button", { class: "btn btn-ghost btn-icon", title: t("reroll"), "aria-label": t("reroll"), onclick: () => o.reroll(sec) }, "⟳") : null);

  // ── Head: passport data page (SPECIMEN) + verdict ─────────────────────────
  const initialsTxt = initials(`${id.first_name} ${id.last_name}`);
  const portrait = () => (p.face ? h("img", { src: faceUrl(p.face.id), alt: id.first_name }) : silhouette(p.recipe.seed, initialsTxt));
  const mug = h("div", { class: "mug-wrap" },
    portrait(),
    h("span", { class: "mug-target" }, `${id.gender === "female" ? "♀" : "♂"} ${id.age}`),
    o.setFace ? h("button", { class: "btn btn-sm mug-btn", onclick: () => facePicker(p, o.setFace, o.savedId) }, "◉ " + (p.face ? t("change_face") : t("pick_face"))) : null);

  const phone = p.contact.phone
    ? h("span", {}, h("span", { class: "mono" }, p.contact.phone), h("span", { class: "fine" }, t("phone_fiction")))
    : h("span", { class: "fine" }, t("phone_none"));
  const fld = (fr, en, ...val) => h("div", { class: "pp-f" }, h("span", { class: "pp-l" }, `${fr} / ${en}`), h("span", { class: "pp-v" }, ...val));
  const [m1, m2] = mrz(p);
  const docNo = m2.slice(0, 9).replace(/</g, "");

  const card = h("article", { class: "idcard passport", "aria-label": `${t("file")} ${p.recipe.seed}` },
    h("header", { class: "pp-band" },
      h("span", { class: "pp-issuer" }, "UTOPIA · SIMULACRE"),
      h("span", { class: "pp-kind" }, "PASSEPORT · PASSPORT · REISEPASS"),
      h("span", { class: "flag", title: L(p.locale.country) }, flag(p.locale.country_code))),
    h("span", { class: "stamp", "aria-hidden": "true" }, t("classified")),
    h("div", { class: "pp-body" },
      h("div", { class: "pp-photo" }, mug, h("p", { class: "codename" }, L(p.codename))),
      h("div", { class: "pp-fields" },
        h("div", { class: "pp-row3" }, fld("Type", "Type", "P"), fld("Code", "Code", "UTO"), fld("N°", "No.", h("span", { class: "mono" }, docNo))),
        h("div", { class: "pp-names" },
          h("span", { class: "pp-l" }, "Nom / Surname · Prénoms / Given names"),
          h("h2", { class: "fullname" }, h("span", { class: "pp-surname" }, ed("identity.last_name", id.last_name)), h("span", { class: "pp-given" }, ed("identity.first_name", id.first_name)))),
        h("div", { class: "pp-grid" },
          fld("Date de naissance", "Date of birth", ed("identity.birth_date", id.birth_date, { type: "date" })),
          fld("Sexe", "Sex", id.gender === "female" ? "F" : "M"),
          fld("Lieu de naissance", "Place of birth", id.birthplace.city),
          fld("Âge", "Age", yrs(id.age)),
          fld("Domicile", "Residence", `${p.residence.place.city}, `, ed("residence.district", p.residence.district)),
          fld("Statut", "Status", lab("status", p.occupation.status)),
          p.occupation.title ? fld("Profession", "Occupation", ed("occupation.title", p.occupation.title), p.occupation.employer ? ` — ${p.occupation.employer}` : "") : null,
          fld("Téléphone", "Phone", phone)),
        h("div", { class: "pp-sign" }, h("span", { class: "pp-l" }, "Signature"), h("span", { class: "signature" }, `${id.first_name} ${id.last_name}`))),
      h("div", { class: "pp-ghost", "aria-hidden": "true" }, portrait())),
    h("div", { class: "mrz", "aria-label": "MRZ" }, h("span", {}, m1), h("span", {}, m2)),
    h("footer", { class: "seedline" }, h("span", {}, `${t("seed")} `, h("b", {}, p.recipe.seed)), h("span", {}, p.locale.code), h("span", {}, L(o.archetypeLabel?.(p.archetype)) || p.archetype)));

  const coh = p.coherence;
  const counts = { error: 0, warn: 0, info: 0 };
  coh.checks.forEach((c) => { if (c.severity in counts) counts[c.severity]++; });
  const report = h("aside", { class: "report" },
    h("div", { class: "report-head" },
      gauge(coh.score),
      h("div", { class: "metrics" },
        h("div", { class: "metric" }, h("span", {}, t("coherence")), h("b", { vars: { color: scoreColor(coh.score) } }, `${coh.score}/100`), h("span", {}, `${counts.error} ✕ · ${counts.warn} ! · ${counts.info} i`)),
        h("div", { class: "metric", title: t("linkability_hint") }, h("span", {}, t("linkability")), h("b", {}, `${coh.linkability}%`), h("span", {}, t("linkability_hint"))))),
    h("div", { class: "actions" }, o.actions || []),
    o.dirty ? h("p", { class: "dirty" }, "● " + t("unsaved")) : null,
    h("p", { class: "pitch" }, L(p.legend.pitch)),
    o.edit ? h("p", { class: "fine" }, "✎ " + t("edit_hint")) : null);
  wrap.append(
    h("div", { class: "dossier" }, card, report),
    h("div", { class: "narrative sheet" }, h("p", { class: "label" }, t("narrative")), L(p.narrative).split("\n\n").map((para) => h("p", {}, para))));

  // ── 01 Identity & appearance ──────────────────────────────────────────────
  const ap = p.appearance;
  wrap.append(section({
    id: "s-identity", num: "01", title: t("s_identity"), sub: t("s_identity_sub"), tools: [rerollBtn("identity"), rerollBtn("appearance")].filter(Boolean),
    body: h("div", { class: "grid-2" },
      h("div", { class: "panel" }, h("dl", { class: "kv" },
        h("dt", {}, t("nickname")), h("dd", {}, ed("identity.nickname", id.nickname || "")),
        h("dt", {}, t("birthplace")), h("dd", {}, `${id.birthplace.city} (${id.birthplace.region})`),
        h("dt", {}, t("zodiac")), h("dd", {}, lab("zodiac", id.zodiac)),
        h("dt", {}, t("languages")), h("dd", {}, id.languages.map((l) => `${lab("language", l.code)} (${lab("lang_level", l.level)})`).join(", ")),
        h("dt", {}, t("housing")), h("dd", {}, `${p.residence.housing_label} · ${lab("tenure", p.residence.tenure)} · ${t("since")} ${p.residence.since_year}`),
        h("dt", {}, "E-mail"), h("dd", { class: "mono small" }, p.contact.email_localparts.map((e) => `${e}@…`).join("  ·  ")))),
      h("div", { class: "panel" }, h("dl", { class: "kv" },
        h("dt", {}, t("height")), h("dd", {}, `${ap.height_cm} cm`),
        h("dt", {}, t("build")), h("dd", {}, lab("build", ap.build)),
        h("dt", {}, t("eyes")), h("dd", {}, lab("eyes", ap.eyes)),
        h("dt", {}, t("hair")), h("dd", {}, `${lab("hair_color", ap.hair_color)}, ${lab("hair_style", ap.hair_style)}`),
        h("dt", {}, t("glasses")), h("dd", {}, ap.glasses ? t("yes") : t("no")),
        h("dt", {}, t("marks")), h("dd", {}, ap.marks.length ? ap.marks.map((m) => lab("marks", m)).join(", ") : t("none"))))),
  }));

  // ── 02 Life path ──────────────────────────────────────────────────────────
  wrap.append(section({
    id: "s-life", num: "02", title: t("s_life"), sub: t("s_life_sub"), tools: [rerollBtn("life")].filter(Boolean),
    body: h("div", { class: "panel" }, h("ol", { class: "timeline" },
      p.timeline.map((e) => h("li", { class: `k-${e.kind}` }, h("span", { class: "yr" }, e.year), h("span", {}, L(e.text)))))),
  }));

  // ── 03 Circle ─────────────────────────────────────────────────────────────
  const f = p.family;
  const person = (role, name, sub, gone) => h("div", { class: `person${gone ? " gone" : ""}` }, h("span", { class: "role" }, role), h("b", {}, name), sub ? h("span", { class: "small muted" }, sub) : null);
  const people = [];
  if (f.partner) people.push(person(t("partner"), f.partner.first_name, `${yrs(year - f.partner.birth_year)} · ${lab("relationship", f.relationship)}`, !f.partner.alive));
  f.children.forEach((c) => people.push(person(t("child"), c.first_name, `${yrs(year - c.birth_year)}`)));
  f.siblings.forEach((c) => people.push(person(lab("sibling", c.gender === "female" ? "sister" : "brother"), c.first_name, `${yrs(year - c.birth_year)}`, !c.alive)));
  f.parents.forEach((c) => people.push(person(c.gender === "female" ? t("mother") : t("father"), c.first_name, `${yrs(year - c.birth_year)}`, !c.alive)));
  f.pets.forEach((pt) => people.push(person(`${t("pet")} · ${lab("species", pt.species)}`, pt.name, `${t("since")} ${pt.since_year}`)));
  wrap.append(section({
    id: "s-family", num: "03", title: t("s_family"), sub: t("s_family_sub"), tools: [rerollBtn("family")].filter(Boolean),
    body: h("div", {},
      h("p", { class: "chips" }, h("span", { class: "chip amber" }, `${t("relationship")} : ${lab("relationship", f.relationship)}`), f.grandchildren ? h("span", { class: "chip" }, `${f.grandchildren} ${t("grandchildren")}`) : null),
      h("div", { class: "people" }, people)),
  }));

  // ── 04 Psychology ─────────────────────────────────────────────────────────
  const per = p.personality;
  wrap.append(section({
    id: "s-mind", num: "04", title: t("s_mind"), sub: t("s_mind_sub"), tools: [rerollBtn("personality"), rerollBtn("interests")].filter(Boolean),
    body: h("div", { class: "grid-2" },
      h("div", { class: "panel" }, h("p", { class: "label" }, `${t("ptype")} — ${lab("ptype", per.type_key)}`), h("div", { class: "ocean" }, radar(per.ocean), oceanBars(per.ocean))),
      h("div", { class: "panel" }, h("dl", { class: "kv" },
        h("dt", {}, t("values")), h("dd", {}, per.values.map(L).join(", ")),
        h("dt", {}, t("quirks")), h("dd", {}, h("ul", { class: "list-clean" }, per.quirks.map((q) => h("li", {}, L(q))))),
        h("dt", {}, t("peeves")), h("dd", {}, per.peeves.map(L).join(", ")),
        h("dt", {}, t("humor")), h("dd", {}, L(per.humor)),
        h("dt", {}, t("interests")), h("dd", {}, h("div", { class: "chips" }, p.interests.map((i) =>
          h("span", { class: `chip${i.depth === "devoted" ? " amber" : ""}`, title: `${lab("depth", i.depth)} · ${t("since")} ${i.since_year}` }, `${L(i.label)} · ${i.since_year}`))))))),
  }));

  // ── 05 Rhythm ─────────────────────────────────────────────────────────────
  const b = p.behavior;
  wrap.append(section({
    id: "s-rhythm", num: "05", title: t("s_rhythm"), sub: t("s_rhythm_sub"), tools: [rerollBtn("behavior")].filter(Boolean),
    body: h("div", { class: "panel" }, heatmap(b.heatmap), h("div", { class: "rhythm-meta" },
      h("span", {}, `${t("chronotype")} `, h("b", {}, lab("chronotype", b.chronotype))),
      h("span", {}, `${t("wake")} `, h("b", {}, b.wake)), h("span", {}, `${t("sleep")} `, h("b", {}, b.sleep)),
      h("span", {}, `${t("tz")} `, h("b", {}, b.timezone)),
      h("span", {}, `${t("device")} `, h("b", {}, b.phone)), b.computer ? h("span", {}, `${t("computer")} `, h("b", {}, b.computer)) : null,
      h("span", {}, h("b", {}, b.posts_per_week.toFixed(1)), ` ${t("ppw")}`))),
  }));

  // ── 06 Digital footprint ──────────────────────────────────────────────────
  wrap.append(section({
    id: "s-digital", num: "06", title: t("s_digital"), sub: t("s_digital_sub"), tools: [rerollBtn("digital")].filter(Boolean),
    body: h("div", {},
      h("div", { class: "link-meter", title: t("linkability_hint") }, h("span", {}, `${t("linkability")} ${coh.linkability}%`), h("div", { class: "track" }, h("span", { class: "needle", vars: { "--v": coh.linkability } }))),
      h("div", { class: "accounts" }, p.accounts.map((a, i) => h("article", { class: "acct", vars: { "--pc": PLATFORM_COLORS[a.platform] || "#8b919a" } },
        h("div", { class: "acct-head" }, pbadge(a.platform), h("div", { class: "who" }, h("b", {}, a.platform_name), a.handle ? h("span", { class: "handle" }, "@", ed(`accounts.${i}.handle`, a.handle)) : h("span", { class: "handle" }, a.display_name))),
        h("div", { class: "small" }, ed(`accounts.${i}.display_name`, a.display_name)),
        a.bio || o.edit ? h("p", { class: "bio" }, ed(`accounts.${i}.bio`, a.bio, { multiline: true })) : null,
        h("div", { class: "acct-meta" },
          h("span", { class: `chip${a.activity === "active" ? " green" : ""}` }, lab("activity", a.activity)),
          a.posts_per_week ? h("span", { class: "chip" }, `${a.posts_per_week}${t("posts_week")}`) : null,
          h("span", { class: "chip" }, `${t("since_claimed")} ${a.claimed_since}`),
          a.real_name ? h("span", { class: "chip amber" }, t("real_name")) : null),
        a.join_date_public && a.claimed_since < year - 1 ? h("p", { class: "warn" }, "⚠ " + t("join_public")) : null)))),
  }));

  // ── 07 Voice ──────────────────────────────────────────────────────────────
  const w = b.writing;
  const avatar = p.face ? h("span", { class: "avatar" }, h("img", { src: faceUrl(p.face.id), alt: "" })) : h("span", { class: "avatar" }, initials(`${id.first_name} ${id.last_name}`));
  wrap.append(section({
    id: "s-voice", num: "07", title: t("s_voice"), sub: t("s_voice_sub"), tools: [rerollBtn("voice")].filter(Boolean),
    body: h("div", {},
      h("div", { class: "style-grid" },
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("register")), h("b", {}, lab("register", w.register))),
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("casing")), h("b", {}, lab("casing", w.casing))),
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("emoji")), h("b", {}, `${Math.round(w.emoji_rate * 100)}% ${w.emojis.join(" ")}`)),
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("typos")), h("b", {}, `${(w.typo_rate * 100).toFixed(1)}%`)),
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("habits")), h("b", {}, w.habits.map((x) => lab("habit", x)).join(", "))),
        h("div", { class: "cellx" }, h("span", { class: "label" }, t("slang")), h("b", {}, w.slang.join(", ") || "—"))),
      h("div", { class: "posts" }, p.voice.map((post) => {
        const acct = p.accounts.find((a) => a.platform === post.platform);
        return h("article", { class: "post" },
          h("div", { class: "post-head" }, avatar.cloneNode(true),
            h("div", { class: "who" }, h("b", {}, acct?.display_name || id.first_name), h("span", {}, `${acct?.handle ? "@" + acct.handle + " · " : ""}${t("days")[post.weekday]} ${String(post.hour).padStart(2, "0")}:${String(post.minute).padStart(2, "0")}`)),
            pbadge(post.platform)),
          h("p", {}, post.text));
      }))),
  }));

  // ── 08 Legend ─────────────────────────────────────────────────────────────
  const lg = p.legend;
  wrap.append(section({
    id: "s-legend", num: "08", title: t("s_legend"), sub: t("s_legend_sub"),
    body: h("div", {},
      h("p", { class: "label" }, t("pitch")), h("p", { class: "pitch" }, L(lg.pitch)),
      h("div", { class: "legend-cols" },
        h("div", { class: "panel tp" }, h("h4", {}, t("talking")), h("ul", { class: "list-clean" }, lg.talking_points.map((x) => h("li", {}, L(x))))),
        h("div", { class: "panel ws" }, h("h4", {}, t("weak")), h("ul", { class: "list-clean" }, lg.weak_spots.map((x) => h("li", {}, L(x))))),
        h("div", { class: "panel op" }, h("h4", {}, t("opsec")), h("ul", { class: "list-clean" }, lg.opsec.map((x) => h("li", {}, L(x))))))),
  }));

  // ── 09 Coherence ──────────────────────────────────────────────────────────
  wrap.append(section({
    id: "s-checks", num: "09", title: t("s_checks"), sub: t("s_checks_sub"),
    body: h("ul", { class: "checks" }, coh.checks.map((c) => h("li", { class: `s-${c.severity}` }, h("span", { class: "sev" }, t(`sev_${c.severity}`)), h("span", {}, L(c.message))))),
  }));

  return wrap;
}

/** Buttons shared by the forge and the vault. */
export function commonActions(p, { exportMd, quiz }) {
  return [
    h("button", { class: "btn btn-sm", onclick: () => quizModal(quiz) }, "?! " + t("quiz")),
    h("button", { class: "btn btn-sm", onclick: async () => { try { download(`simulacre-${p.recipe.seed}.md`, await exportMd(), "text/markdown"); } catch (e) { toast(e.message, true); } } }, "↓ " + t("export_md")),
    h("button", { class: "btn btn-sm", onclick: () => download(`simulacre-${p.recipe.seed}.json`, JSON.stringify(p, null, 2), "application/json") }, "{ } " + t("export_json")),
    h("button", { class: "btn btn-sm btn-ghost", onclick: () => window.print() }, "⎙ " + t("print")),
  ];
}

/** Applies an inline edit to a copy of the persona. Dates and empty nicknames need care. */
export function applyEdit(p, path, value) {
  const next = clone(p);
  let v = value.trim();
  if (path === "identity.nickname" && !v) v = null;
  if (path === "identity.birth_date" && !/^\d{4}-\d{2}-\d{2}$/.test(v)) throw new Error("YYYY-MM-DD");
  setPath(next, path, v);
  return next;
}
