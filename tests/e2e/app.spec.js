import { expect, test } from "@playwright/test";

const errors = [];
test.beforeEach(async ({ page }) => {
  errors.length = 0;
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  await page.addInitScript(() => { try { localStorage.setItem("simulacre.lang", "fr"); } catch {} });
});
test.afterEach(() => expect(errors, "no console errors / CSP violations").toEqual([]));

const forge = async (page, seed, locale = "fr_FR") => {
  await page.goto(`/#/?seed=${seed}&locale=${locale}`);
  await expect(page.locator("#log")).toContainText("légende prête");
  await expect(page.locator(".fullname")).toBeVisible();
};

test("forge renders a complete dossier", async ({ page }) => {
  await forge(page, "SIM-E2E-0001");
  await expect(page.locator(".section")).toHaveCount(9);
  await expect(page.locator(".gauge text")).toHaveText(/\d+/);
  await expect(page.locator(".timeline li").first()).toContainText("Naissance");
  await expect(page.locator(".heat .cell")).toHaveCount(7 * 24);
  await expect(page.locator(".acct").first()).toBeVisible();
  await expect(page.locator(".post").first()).toBeVisible();
  await expect(page.locator(".checks li").first()).toBeVisible();
  await expect(page.locator(".stamp")).toHaveText("FICTIF");
});

test("same seed, same person", async ({ page }) => {
  await forge(page, "SIM-E2E-SAME", "en_GB");
  const a = await page.locator(".fullname").textContent();
  await page.goto("/#/pool");
  await forge(page, "SIM-E2E-SAME", "en_GB");
  expect(await page.locator(".fullname").textContent()).toBe(a);
});

test("language switch re-renders everything", async ({ page }) => {
  await forge(page, "SIM-E2E-LANG");
  await page.getByRole("button", { name: "EN", exact: true }).click();
  await expect(page.locator(".nav")).toContainText("Pool");
  await expect(page.locator(".tagline")).toContainText("never existed");
  await expect(page.locator(".stamp")).toHaveText("FICTIONAL");
  await expect(page.locator(".section-title").nth(1)).toContainText("Life path");
});

test("editing re-runs the coherence engine", async ({ page }) => {
  await forge(page, "SIM-E2E-EDIT");
  const before = Number(await page.locator(".gauge text").textContent());
  await page.locator(".idcard .editable").nth(2).click(); // birth date
  const input = page.locator(".idcard input.edit-input");
  await input.fill("2015-01-01");
  await input.press("Enter");
  await expect(page.locator(".checks li.s-error").first()).toBeVisible();
  await expect(page.locator(".dirty")).toBeVisible();
  const after = Number(await page.locator(".gauge text").textContent());
  expect(after).toBeLessThan(before);
});

test("reroll changes one section only", async ({ page }) => {
  await forge(page, "SIM-E2E-REROLL");
  const name = await page.locator(".fullname").textContent();
  const posts = await page.locator(".post p").allInnerTexts();
  await page.locator("#s-voice .tools button").click();
  await expect.poll(async () => (await page.locator(".post p").allInnerTexts()).join("|")).not.toBe(posts.join("|"));
  expect(await page.locator(".fullname").textContent()).toBe(name);
});

test("face, vault, lifecycle, journal", async ({ page }) => {
  await forge(page, "SIM-E2E-VAULT");
  await page.locator(".mug-btn").click();
  await page.locator(".face-opt:not(.loading)").first().click();
  // Server: /faces/<sha>.jpg. Browser build: an object URL from IndexedDB.
  await expect(page.locator(".mug-wrap img")).toHaveAttribute("src", /\/faces\/[0-9a-f]{32}\.jpg|^blob:/);
  await expect.poll(() => page.locator(".mug-wrap img").evaluate((i) => i.complete && i.naturalWidth)).toBe(512);
  await page.getByRole("button", { name: /Enregistrer au coffre/ }).click();
  await expect(page).toHaveURL(/#\/p\/\w+/);
  await page.locator(".lifecycle button", { hasText: "Active" }).click();
  await expect(page.locator(".lifecycle button[aria-pressed=true]")).toHaveText("Active");
  await page.locator(".sticky textarea").fill("Compte Instagram ouvert.");
  await page.locator(".sticky form button").click();
  await expect(page.locator(".journal")).toContainText("Compte Instagram ouvert.");
  await expect(page.locator(".journal")).toContainText("draft → active");
  await page.goto("/#/vault");
  await expect(page.locator(".pcard .pill.s-active").first()).toBeVisible();
  await expect(page.locator("#vault-count")).not.toHaveText("");
});

test("contamination catches a cloned legend", async ({ page }) => {
  for (let i = 0; i < 2; i++) {
    await forge(page, "SIM-E2E-CLONE");
    await page.getByRole("button", { name: /Enregistrer au coffre/ }).click();
    await expect(page).toHaveURL(/#\/p\//);
  }
  await page.goto("/#/contamination");
  await expect(page.locator(".pair .risk").first()).toHaveText(/100/);
  await expect(page.locator(".pair").first()).toContainText("Même nom complet");
});

test("pool scrolls forever", async ({ page }) => {
  await page.goto("/#/pool");
  await expect(page.locator(".pcard")).toHaveCount(24);
  await page.locator(".sentinel").scrollIntoViewIfNeeded();
  await expect.poll(() => page.locator(".pcard").count()).toBeGreaterThan(24);
  await page.locator(".pcard").first().click();
  await expect(page.locator("#log")).toContainText("légende prête");
});

test("interrogation runs to a score", async ({ page }) => {
  await forge(page, "SIM-E2E-QUIZ");
  await page.getByRole("button", { name: /Interrogatoire/ }).click();
  for (let i = 0; i < 10; i++) {
    const q = page.locator(".quiz-choices button").first();
    if (!(await q.isVisible().catch(() => false))) break;
    await q.click();
    await page.waitForTimeout(1400);
  }
  await expect(page.locator(".quiz-result .big")).toHaveText(/\d+\/\d+/);
});

test("doctrine states the guard-rails", async ({ page }) => {
  await page.goto("/#/doctrine");
  await expect(page.locator(".doctrine")).toContainText("18 ans minimum");
  await expect(page.locator(".doctrine")).toContainText("Pas d'adresse postale");
});

test("the marionette's strings move, and the puppet with them", async ({ page }) => {
  await page.goto("/#/");
  const pose = () => page.locator(".hero-puppet").evaluate((svg) =>
    [...svg.querySelectorAll(".mn-string, .mn-limb")].map((e) => e.getAttribute("y1") ?? e.getAttribute("d") ?? e.getAttribute("points")).join("|"));
  await expect(page.locator(".hero-puppet .mn-body")).toBeVisible();
  // The strings come down from above the page, not from inside the hero.
  const top = await page.locator(".hero-puppet .mn-string").first().evaluate((l) => l.getBoundingClientRect().top);
  expect(top).toBeLessThanOrEqual(0);
  const a = await pose();
  await page.waitForTimeout(400);
  expect(await pose()).not.toEqual(a);
  await expect(page.locator(".brand .brand-mark .pm-shade >> nth=0")).toBeVisible();
});

test("the vault survives a reload", async ({ page }) => {
  await forge(page, "SIM-E2E-PERSIST");
  await page.getByRole("button", { name: /Enregistrer au coffre/ }).click();
  await expect(page).toHaveURL(/#\/p\//);
  const url = page.url();
  await page.reload();
  await expect(page.locator(".fullname")).toBeVisible();
  expect(page.url()).toBe(url);
  await page.goto("/#/vault");
  await expect(page.locator(".pcard").first()).toBeVisible();
});

test("a vault backup restores into an empty browser", async ({ page, browser }, info) => {
  const browserBuild = info.project.name.startsWith("browser");
  await forge(page, "SIM-E2E-BACKUP");
  const name = `${await page.locator(".pp-given").textContent()} ${await page.locator(".pp-surname").textContent()}`;
  await page.getByRole("button", { name: /Enregistrer au coffre/ }).click();
  await expect(page).toHaveURL(/#\/p\//);
  await page.goto("/#/vault");
  // Never backed up: the browser build warns, the server does not need to.
  await expect(page.locator(".backup-bar")).toHaveClass(browserBuild ? /stale/ : /^backup-bar$/);
  const [dl] = await Promise.all([page.waitForEvent("download"), page.locator("#backup-btn").click()]);
  expect(dl.suggestedFilename()).toMatch(/^simulacre-vault-\d{4}-\d{2}-\d{2}\.json$/);
  const file = await dl.path();
  await expect(page.locator(".backup-bar")).not.toHaveClass(/stale/);
  await expect(page.locator(".backup-when b")).toHaveText("aujourd'hui");

  // A fresh browser profile: empty IndexedDB in the browser build.
  const ctx = await browser.newContext({ baseURL: info.project.use.baseURL });
  const fresh = await ctx.newPage();
  const errs = [];
  fresh.on("pageerror", (e) => errs.push(String(e)));
  await fresh.addInitScript(() => { try { localStorage.setItem("simulacre.lang", "fr"); } catch {} });
  await fresh.goto("/#/vault");
  await expect(fresh.locator(".backup-bar")).toBeVisible();
  await fresh.locator(".backup-bar input[type=file]").setInputFiles(file);
  await expect(fresh.locator(".toast")).toContainText(browserBuild ? "1 ajoutée(s)" : "déjà présente(s)");
  await expect(fresh.locator(".pcard", { hasText: name }).first()).toBeVisible();
  await fresh.locator(".pcard", { hasText: name }).first().click();
  // Skipped on the server: the legend is already in its (shared) vault.
  if (browserBuild) await expect(fresh.locator(".journal")).toContainText("imported from backup");

  // Garbage is refused with a message, and changes nothing.
  await fresh.goto("/#/vault");
  await fresh.locator(".backup-bar input[type=file]").setInputFiles({ name: "x.json", mimeType: "application/json", buffer: Buffer.from('{"format":"other"}') });
  await expect(fresh.locator(".toast.err")).toContainText("Restauration refusée");
  expect(errs).toEqual([]);
  await ctx.close();
});

test("paper is the default theme even on a dark OS", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/#/doctrine");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "paper");
  await page.locator(".uv-toggle").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "uv");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "uv", { timeout: 5000 });
  await page.locator(".uv-toggle").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "paper");
});

test("forge again = new legend; history goes back and forward", async ({ page }) => {
  await forge(page, "SIM-E2E-HIST");
  const first = await page.locator(".fullname").textContent();
  await expect(page.locator(".history")).toBeHidden();

  await page.locator("#forge-btn").click();
  await expect(page.locator("#seed")).not.toHaveValue("SIM-E2E-HIST");
  await expect(page.locator(".hist-pos")).toHaveText("2 / 2", { timeout: 10000 });
  const secondSeed = await page.locator("#seed").inputValue();
  expect(await page.locator(".fullname").textContent()).not.toBe(first);

  // Edit legend #2, then go back to #1 and forward again: the edit survives.
  await page.locator(".idcard .editable").first().click();
  await page.locator(".idcard input.edit-input").fill("Zzyzx");
  await page.locator(".idcard input.edit-input").press("Enter");
  await expect(page.locator(".pp-surname")).toContainText("Zzyzx");

  await page.locator("#hist-prev").click();
  await expect(page.locator(".hist-pos")).toHaveText("1 / 2");
  await expect(page.locator("#seed")).toHaveValue("SIM-E2E-HIST");
  expect(await page.locator(".fullname").textContent()).toBe(first);

  await page.locator("#hist-next").click();
  await expect(page.locator("#seed")).toHaveValue(secondSeed);
  await expect(page.locator(".pp-surname")).toContainText("Zzyzx");

  // Going back then forging drops the forward branch, like a browser.
  await page.locator("#hist-prev").click();
  await page.locator("#forge-btn").click();
  await expect(page.locator(".hist-pos")).toHaveText("2 / 2", { timeout: 10000 });
  await expect(page.locator(".hist-chip")).toHaveCount(2);
  await expect(page.locator(".hist-chip").first()).toHaveAttribute("title", /SIM-E2E-HIST/);
  await expect(page.locator(".hist-chip").nth(1)).not.toHaveAttribute("title", new RegExp(secondSeed));
});

test("a typed seed is honoured and one click forges exactly once", async ({ page }) => {
  await forge(page, "SIM-E2E-ONE");
  await page.locator("#seed").fill("SIM-E2E-TWO");
  await page.locator("#forge-btn").click();
  await expect(page.locator(".hist-pos")).toHaveText("2 / 2", { timeout: 10000 });
  await expect(page.locator("#seed")).toHaveValue("SIM-E2E-TWO");
  await page.waitForTimeout(800);
  await expect(page.locator(".hist-chip")).toHaveCount(2);
});
