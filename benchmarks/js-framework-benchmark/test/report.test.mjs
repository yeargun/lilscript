import assert from "node:assert/strict";
import test from "node:test";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "../../browser/playwright-runtime.mjs";

const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const reportUrl = pathToFileURL(resolve(import.meta.dirname, "..", "report.html")).href;

test("standalone report filters completed real-browser evidence", async () => {
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
    assert.equal(await page.locator("#resultsBody tr").count(), 13);
    assert.match(await page.locator("#resultsBody tr.solidlil").innerText(), /4,711 B/);
    assert.equal(await page.locator("#selectCompact").count(), 1);

    await page.locator("#phase").selectOption("cpu");
    await page.locator("#workload").selectOption("05_swap1k");
    assert.equal(await page.locator("#resultsBody tr").count(), 13);
    assert.match(await page.locator("#tableNote").innerText(), /Complete 1,755 of 1,755/);
    assert.match(await page.locator("#resultsBody tr.solidlil").innerText(), /15/);

    await page.locator("#phase").selectOption("memory");
    assert.match(await page.locator("#tableNote").innerText(), /Complete 585 of 585/);
    assert.match(await page.locator("#resultsBody tr.solidlil").innerText(), /MiB/);

    await page.locator("#phase").selectOption("cold");
    assert.match(await page.locator("#tableNote").innerText(), /Complete 195 of 195/);
    assert.match(await page.locator("#resultsBody tr.solidlil").innerText(), /ms/);

    await page.locator("#selectCompact").click();
    assert.equal(await page.locator("#resultsBody tr").count(), 5);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
