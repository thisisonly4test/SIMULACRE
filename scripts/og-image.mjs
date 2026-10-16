// Renders the link-preview image (web/assets/og.jpg, 1200×630) from the live
// landing page. Run from the repository root with the static test server up:
//   node tests/e2e/static-server.mjs &
//   CHROMIUM_PATH=… node scripts/og-image.mjs
import { chromium } from "@playwright/test";
const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH });
const p = await b.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1, bypassCSP: true });
await p.emulateMedia({ reducedMotion: "reduce" });
await p.goto("http://127.0.0.1:7990/"); await p.waitForSelector(".wordmark");
await p.addStyleTag({ content: `
  .topbar, .forge-row, .history, .foot, .skip, main > .view > :not(.landing) { display: none !important; }
  main { max-width: none; padding: 0 70px; }
  .landing { min-height: 630px; justify-content: center; padding: 0; }
  .hero { height: 560px; grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr); }
  .hero-copy { align-self: center; }
  .wordmark { font-size: 6.4rem; }
  .tagline { font-size: 2.15rem; max-width: 660px; }
  .eyebrow { font-size: .95rem; }
` });
await p.waitForTimeout(1500);
await p.screenshot({ path: "web/assets/og.jpg", type: "jpeg", quality: 86 });
await b.close();
