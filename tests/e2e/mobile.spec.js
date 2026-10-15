import { expect, test } from "@playwright/test";

for (const route of ["/#/?seed=SIM-E2E-MOB", "/#/pool", "/#/vault", "/#/contamination", "/#/doctrine"]) {
  test(`no horizontal overflow on ${route}`, async ({ page }) => {
    await page.goto(route);
    await page.waitForTimeout(1500);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    expect(overflow).toBeLessThanOrEqual(0);
  });
}
