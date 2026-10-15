// The Simulacre mark: a marionette — a fabricated person, held by strings.
// The hero version is a doll in silhouette, a faceless shadow, whose
// strings come down from the very top of the page. An unseen hand sways the
// control, the strings pull, and the doll follows through a small
// spring-and-constraint simulation.

import { s } from "./util.js";

// ── Small static mark (top bar). Same drawing as assets/favicon.svg. ────────
export function puppetMark() {
  const L = (x1, y1, x2, y2, cls) => s("line", { x1, y1, x2, y2, class: cls });
  return s("svg", { viewBox: "0 0 64 64", "aria-hidden": "true", class: "brand-mark" },
    s("g", { class: "pm-sway" },
      L(14, -6, 15, 41, "pm-string"), L(50, -6, 49, 41, "pm-string"), L(32, -6, 32, 10, "pm-string"),
      L(25, -6, 27, 52, "pm-string"), L(39, -6, 37, 52, "pm-string"),
      s("polyline", { points: "28,46 27.5,58", class: "pm-limb" }), s("polyline", { points: "36,46 36.5,58", class: "pm-limb" }),
      s("ellipse", { cx: 26.6, cy: 59.4, rx: 3.2, ry: 2, class: "pm-shade" }), s("ellipse", { cx: 37.4, cy: 59.4, rx: 3.2, ry: 2, class: "pm-shade" }),
      s("polyline", { points: "28,33 21,38 15,41", class: "pm-limb" }), s("polyline", { points: "36,33 43,38 49,41", class: "pm-limb" }),
      s("path", { d: "M28 30 L36 30 L37 36 L45 50 Q32 53 19 50 L27 36 Z", class: "pm-shade" }),
      s("g", { transform: "rotate(10 32 20)" },
        s("polyline", { points: "22,17 18,30", class: "pm-limb pm-tail" }), s("polyline", { points: "42,17 46,30", class: "pm-limb pm-tail" }),
        s("circle", { cx: 32, cy: 20, r: 10.5, class: "pm-shade" }),
        s("path", { d: "M37 10.5 l5 -4 l0.6 7 Z M37 10.5 l-5.6 -2.6 l1.6 6.4 Z", class: "pm-shade" }))));
}

// ── Living marionette (hero) ────────────────────────────────────────────────
const sub = (a, b) => [a[0] - b[0], a[1] - b[1]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1]];
const mul = (a, k) => [a[0] * k, a[1] * k];
const len = (a) => Math.hypot(a[0], a[1]) || 1e-6;
const unit = (a) => mul(a, 1 / len(a));
const rot = ([x, y], t) => [x * Math.cos(t) - y * Math.sin(t), x * Math.sin(t) + y * Math.cos(t)];
const f = (n) => n.toFixed(1);
const pts = (...ps) => ps.map((p) => `${f(p[0])},${f(p[1])}`).join(" ");
/** Keep `p` exactly `d` away from `from` (a rigid bone). */
const bone = (from, p, d) => add(from, mul(sub(p, from), d / len(sub(p, from))));
const setXY = (el, [x1, y1], [x2, y2]) => { el.setAttribute("x1", f(x1)); el.setAttribute("y1", f(y1)); el.setAttribute("x2", f(x2)); el.setAttribute("y2", f(y2)); };
const setC = (el, [x, y]) => { el.setAttribute("cx", f(x)); el.setAttribute("cy", f(y)); };

// The drawing space. The view box is zoomed onto the doll, which sits a little
// right of centre; the strings run above it up to the top of the page.
const VB = { x: 94, y: 46, w: 240, h: 284 };   // keep w/h in sync with .hero-puppet aspect-ratio
const CX = 228;
const ANCHOR = { handL: -66, handR: 66, kneeL: -26, kneeR: 26, headL: -16, headR: 16 };   // x on the unseen control
const REST = { head: [CX, 104], waist: [CX, 194], handL: [CX - 44, 196], handR: [CX + 44, 196], kneeL: [CX - 12, 268], kneeR: [CX + 12, 268] };
let uid = 0;
const HEAD_R = 44, BODICE = 36, ARM = 30, THIGH = 32, SHIN = 32, TAIL = 40;

/** Animated marionette (strings + doll layers). Stops by itself once removed from the DOM. */
export function marionette() {
  // Two layers: the strings stay under the top bar and the UV ink, while the
  // doll floats above both, so nothing is printed over the shadow.
  const layer = (cls) => s("svg", { class: `marionette ${cls}`, viewBox: `${VB.x} ${VB.y} ${VB.w} ${VB.h}`, "aria-hidden": "true" });
  const svg = layer("mn-doll"), wires = layer("mn-wires");
  const el = (tag, cls, attrs = {}, ...kids) => s(tag, { class: cls, ...attrs }, ...kids);

  const legs = [el("polyline", "mn-limb"), el("polyline", "mn-limb")];
  const arms = [el("polyline", "mn-limb"), el("polyline", "mn-limb")];
  const shoes = [el("ellipse", "mn-shade", { rx: 12, ry: 7 }), el("ellipse", "mn-shade", { rx: 12, ry: 7 })];
  const hands = [el("circle", "mn-shade", { r: 9 }), el("circle", "mn-shade", { r: 9 })];
  const tails = [el("polyline", "mn-limb mn-tail"), el("polyline", "mn-limb mn-tail")];
  const dress = el("path", "mn-shade");
  // A round doll's head with a bow. No face: it is only a shadow.
  const head = el("g", "", {},
    el("circle", "mn-shade", { r: HEAD_R }),
    el("path", "mn-shade", { d: "M16 -40 l22 -16 l3 27 Z M16 -40 l-24 -10 l7 25 Z" }),
    el("circle", "mn-shade", { cx: 16, cy: -38, r: 6 }));
  const body = el("g", "mn-body", {}, ...tails, ...legs, ...shoes, dress, ...arms, ...hands, head);
  const strings = Object.keys(ANCHOR).map(() => el("line", "mn-string"));
  svg.append(body);
  // The shadow is translucent: mask the strings with its silhouette so they
  // vanish behind it instead of showing through.
  const id = `mn-${++uid}`;
  body.id = `${id}-body`;
  const big = { x: -1e4, y: -1e5, width: 2e4, height: 2e5 };
  wires.append(
    s("defs", {},
      // Silhouette to solid black, whatever the theme's colours and opacity.
      s("filter", { id: `${id}-solid` }, s("feColorMatrix", { values: "0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 6 0" })),
      s("mask", { id: `${id}-mask`, maskUnits: "userSpaceOnUse", ...big },
        s("rect", { ...big, fill: "#fff" }), s("use", { href: `#${id}-body`, filter: `url(#${id}-solid)` }))),
    s("g", { mask: `url(#${id}-mask)` }, ...strings));

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  // Where the top of the page is, in view-box units: strings start above it.
  let top = -400;
  const measure = () => {
    const r = svg.getBoundingClientRect();
    if (r.width) top = VB.y - ((r.top + scrollY) * VB.w) / r.width - 30;
  };
  const ro = new ResizeObserver(() => { measure(); if (reduced) step(0, 0); });
  ro.observe(document.documentElement);

  // Spring-driven points: each follows a target pulled by its string.
  const P = {};
  for (const [k, v] of Object.entries(REST)) P[k] = { p: [...v], v: [0, 0] };
  P.ankleL = { p: [CX - 12, 300], v: [0, 0] };
  P.ankleR = { p: [CX + 12, 300], v: [0, 0] };
  P.tailL = { p: [CX - 50, 130], v: [0, 0] };
  P.tailR = { p: [CX + 50, 130], v: [0, 0] };
  P.hem = { p: [0, 0], v: [0, 0] };                  // the skirt's lag behind the body
  const spring = (pt, target, k, damp, dt) => {
    pt.v = mul(add(pt.v, mul(sub(target, pt.p), k * dt)), Math.pow(1 - damp, dt));
    pt.p = add(pt.p, mul(pt.v, dt));
  };

  let ptr = 0, ptrTarget = 0, tilt = 0.22;
  svg.addEventListener("pointermove", (e) => {
    const r = svg.getBoundingClientRect();
    ptrTarget = Math.max(-1, Math.min(1, ((e.clientX - r.left) / r.width) * 2 - 1)) * 30;
  });
  svg.addEventListener("pointerleave", () => { ptrTarget = 0; });

  const step = (t, dt) => {
    ptr += (ptrTarget - ptr) * Math.min(1, 0.04 * dt);
    // The puppeteer's hand, somewhere above the page: a slow, uneven sway.
    const th = 0.11 * Math.sin(0.8 * t) + 0.05 * Math.sin(2.3 * t + 1);
    const dx = 12 * Math.sin(0.5 * t) + ptr;
    const A = (k) => [CX + dx + ANCHOR[k] * Math.cos(th), top + ANCHOR[k] * Math.sin(th)];
    const disp = (k) => [dx + ANCHOR[k] * (Math.cos(th) - 1), ANCHOR[k] * Math.sin(th)];
    const pos = (x) => Math.max(0, x) ** 3;
    const lift = {
      handL: 50 * pos(Math.sin(0.7 * t)), handR: 50 * pos(Math.sin(0.7 * t + 2.2)),
      kneeL: 22 * pos(Math.sin(1.05 * t + 1)), kneeR: 22 * pos(Math.sin(1.05 * t + 1 + Math.PI)),
    };
    const pull = (k) => add(add(REST[k], disp(k)), [0, -(lift[k] || 0)]);

    const hd = mul(add(disp("headL"), disp("headR")), 0.5);
    spring(P.head, add(REST.head, [hd[0], hd[1] - 5 * Math.sin(0.6 * t)]), 0.012, 0.08, dt);
    const H = P.head.p;
    // Every few seconds the head snaps to the other shoulder, then sags back.
    const ph = t % 8.7;
    const twitch = ph < 0.6 ? -0.55 * Math.sin((ph / 0.6) * Math.PI) ** 0.6 : 0;
    tilt += (0.22 + th * 0.5 + (H[0] - REST.head[0]) * 0.008 - tilt) * Math.min(1, 0.05 * dt);
    const ht = tilt + twitch;

    const N = add(H, rot([0, HEAD_R - 2], ht * 0.4));
    spring(P.waist, add(N, [0, BODICE]), 0.01, 0.07, dt);
    P.waist.p = bone(N, P.waist.p, BODICE);
    const W = P.waist.p;
    const u = mul(sub(W, N), 1 / BODICE), side = [-u[1], u[0]];   // side: screen left when hanging straight
    const sh = [add(add(N, mul(u, 7)), mul(side, 17)), add(add(N, mul(u, 7)), mul(side, -17))];
    const hp = [add(add(W, mul(u, 34)), mul(side, 13)), add(add(W, mul(u, 34)), mul(side, -13))];

    const out = { arms: [], legs: [] };
    ["L", "R"].forEach((sd, i) => {
      const hand = P[`hand${sd}`], knee = P[`knee${sd}`], ankle = P[`ankle${sd}`];
      spring(hand, pull(`hand${sd}`), 0.016, 0.09, dt);
      if (len(sub(hand.p, sh[i])) > 2 * ARM - 1) hand.p = bone(sh[i], hand.p, 2 * ARM - 1);
      // Two-bone IK: the elbow bends outward.
      const d = len(sub(hand.p, sh[i])), mid = mul(add(hand.p, sh[i]), 0.5);
      const n = mul([-(hand.p[1] - sh[i][1]), hand.p[0] - sh[i][0]], 1 / d);
      const hgt = Math.sqrt(Math.max(0, ARM * ARM - (d / 2) ** 2));
      const outward = mul(side, i ? -1 : 1);
      const elbow = add(mid, mul(n, hgt * Math.sign(n[0] * outward[0] + n[1] * outward[1] || 1)));
      out.arms.push([sh[i], elbow, hand.p]);

      spring(knee, pull(`knee${sd}`), 0.014, 0.09, dt);
      knee.p = bone(hp[i], knee.p, THIGH);
      spring(ankle, add(knee.p, [0, SHIN]), 0.02, 0.12, dt);
      ankle.p = bone(knee.p, ankle.p, SHIN);
      out.legs.push([hp[i], knee.p, ankle.p]);
    });

    // Pigtails swing from the sides of the head.
    const roots = [add(H, rot([-HEAD_R + 6, -6], ht)), add(H, rot([HEAD_R - 6, -6], ht))];
    [P.tailL, P.tailR].forEach((tp, i) => {
      spring(tp, add(roots[i], [i ? 12 : -12, TAIL]), 0.02, 0.05, dt);
      tp.p = bone(roots[i], tp.p, TAIL);
    });
    // The skirt trails the body's motion a little.
    spring(P.hem, mul(P.waist.v, -6), 0.05, 0.15, dt);

    // Paint.
    legs.forEach((l, i) => l.setAttribute("points", pts(...out.legs[i])));
    arms.forEach((l, i) => l.setAttribute("points", pts(...out.arms[i])));
    hands.forEach((hd2, i) => setC(hd2, out.arms[i][2]));
    shoes.forEach((sh2, i) => {
      const [, k, a] = out.legs[i], dir = unit(sub(a, k));
      const c = add(add(a, mul(dir, 3)), [i ? 4 : -4, 0]);
      sh2.setAttribute("transform", `translate(${f(c[0])} ${f(c[1])}) rotate(${f((Math.atan2(dir[1], dir[0]) * 180) / Math.PI - 90)})`);
    });
    tails.forEach((tl, i) => tl.setAttribute("points", pts(roots[i], [P.tailL, P.tailR][i].p)));
    const hemC = add(add(W, mul(u, 56)), P.hem.p);
    const hemL = add(hemC, mul(side, 54)), hemR = add(hemC, mul(side, -54));
    dress.setAttribute("d",
      `M${pts(add(N, mul(side, 12)))} L${pts(add(sh[0], mul(side, 3)))} L${pts(add(W, mul(side, 15)))} L${pts(hemL)}` +
      ` Q${pts(add(hemC, mul(u, 12)))} ${pts(hemR)} L${pts(add(W, mul(side, -15)))} L${pts(add(sh[1], mul(side, -3)))} L${pts(add(N, mul(side, -12)))} Z`);
    head.setAttribute("transform", `translate(${f(H[0])} ${f(H[1])}) rotate(${f((ht * 180) / Math.PI)})`);

    const ends = {
      handL: out.arms[0][2], handR: out.arms[1][2], kneeL: out.legs[0][1], kneeR: out.legs[1][1],
      headL: add(H, rot([-14, -HEAD_R + 2], ht)), headR: add(H, rot([14, -HEAD_R + 2], ht)),
    };
    Object.keys(ANCHOR).forEach((k, i) => setXY(strings[i], A(k), ends[k]));
  };

  // Settle once so the first painted frame is already a natural pose.
  for (let i = 0; i < 240; i++) step(0, 1);

  let raf = 0, last = 0, t = 0, visible = true;
  const tick = (now) => {
    raf = 0;
    if (!svg.isConnected) { io.disconnect(); ro.disconnect(); return; }
    if (reduced) { step(0, 0); return; }   // static pose, but strings still reach the top
    const dt = Math.min(3, last ? (now - last) / 16.67 : 1);   // in 60 fps frames, clamped after tab switches
    last = now;
    t += dt / 60;
    step(t, dt);
    if (visible) raf = requestAnimationFrame(tick);
  };
  const io = new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    if (visible && !raf) { measure(); last = 0; raf = requestAnimationFrame(tick); }
  });
  io.observe(svg);
  const frag = document.createDocumentFragment();
  frag.append(wires, svg);
  return frag;
}
