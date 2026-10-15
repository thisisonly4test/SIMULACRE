// The platform was first called LEGEND. Carry the browser-side settings
// (language, UV lamp, collapsed sections, token, forge history) over to the new
// key names. Imported first by app.js so it runs before any module reads them.

for (const get of [() => localStorage, () => sessionStorage]) {
  try {
    const st = get();
    for (const k of Object.keys(st)) {
      if (!k.startsWith("legend.")) continue;
      const renamed = `simulacre.${k.slice("legend.".length)}`;
      if (st.getItem(renamed) === null) st.setItem(renamed, st.getItem(k));
      st.removeItem(k);
    }
  } catch { /* storage unavailable: nothing to carry over */ }
}
