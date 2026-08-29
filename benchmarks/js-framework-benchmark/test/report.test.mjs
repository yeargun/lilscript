import assert from "node:assert/strict";
import test from "node:test";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "../../browser/playwright-runtime.mjs";

const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const reportUrl = pathToFileURL(resolve(import.meta.dirname, "..", "report.html")).href;

test("standalone report handles the current size-only comparison", async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
  });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
  try {
    await page.goto(reportUrl);
    await page.locator("#resultsBody tr").first().waitFor();
    assert.equal(await page.title(), "SolidLil framework benchmark");
    assert.equal(await page.locator("#resultsBody tr").count(), 2);
    assert.match(await page.locator("#resultsBody tr.solidlil").innerText(), /3,414 B/);
    assert.equal(await page.locator("#selectCompact").count(), 1);

    await page.locator("#phase").selectOption("cpu");
    assert.equal(await page.locator("#resultsBody tr").count(), 1);
    assert.match(await page.locator("#tableNote").innerText(), /IN PROGRESS/);

    await page.locator("#selectCompact").click();
    await page.locator("#phase").selectOption("size");
    assert.equal(await page.locator("#resultsBody tr").count(), 2);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
