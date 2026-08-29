import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import test from "node:test";
import { chromium } from "../../browser/playwright-runtime.mjs";

const upstreamRoot = resolve(import.meta.dirname, "../upstream");
const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

async function serve(request, response) {
  const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  let path = resolve(upstreamRoot, `.${pathname}`);
  if (path !== upstreamRoot && !path.startsWith(`${upstreamRoot}${sep}`)) {
    response.writeHead(403).end();
    return;
  }
  try {
    if ((await stat(path)).isDirectory()) path = resolve(path, "index.html");
    const content = await readFile(path);
    const contentType =
      extname(path) === ".js"
        ? "text/javascript"
        : extname(path) === ".css"
          ? "text/css"
          : "text/html";
    response.writeHead(200, { "content-type": contentType }).end(content);
  } catch {
    response.writeHead(404).end();
  }
}

test("source-aligned SolidLil v2 passes every keyed browser operation", async () => {
  const server = createServer(serve);
  await new Promise((resolvePromise) => server.listen(0, "127.0.0.1", resolvePromise));
  const address = server.address();
  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
    args: ["--headless=new"],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
    await page.goto(
      `http://127.0.0.1:${address.port}/frameworks/keyed/solidlil-v2/index.html`,
    );
    const rows = page.locator("tbody > tr");

    await page.locator("#run").click();
    await assert.doesNotReject(() => rows.nth(999).waitFor());
    assert.equal(await rows.count(), 1000);

    const originalLabel = await rows.nth(0).locator("td").nth(1).innerText();
    await page.locator("#update").click();
    assert.equal(
      await rows.nth(0).locator("td").nth(1).innerText(),
      `${originalLabel} !!!`,
    );

    await rows.nth(1).locator("td").nth(1).locator("a").click();
    assert.match((await rows.nth(1).getAttribute("class")) ?? "", /\bdanger\b/);
    await rows.nth(2).locator("td").nth(1).locator("a").click();
    assert.doesNotMatch((await rows.nth(1).getAttribute("class")) ?? "", /\bdanger\b/);
    assert.match((await rows.nth(2).getAttribute("class")) ?? "", /\bdanger\b/);

    const secondId = await rows.nth(1).locator("td").nth(0).innerText();
    const lateId = await rows.nth(998).locator("td").nth(0).innerText();
    await page.locator("#swaprows").click();
    assert.equal(await rows.nth(1).locator("td").nth(0).innerText(), lateId);
    assert.equal(await rows.nth(998).locator("td").nth(0).innerText(), secondId);

    const removedId = await rows.nth(4).locator("td").nth(0).innerText();
    await rows.nth(4).locator("td").nth(2).locator("a").click();
    assert.equal(await rows.count(), 999);
    assert.equal(
      await page
        .locator("tbody > tr > td:first-child")
        .filter({ hasText: new RegExp(`^${removedId}$`) })
        .count(),
      0,
    );

    await page.locator("#add").click();
    assert.equal(await rows.count(), 1999);
    await page.locator("#clear").click();
    assert.equal(await rows.count(), 0);
    await page.locator("#runlots").click();
    await assert.doesNotReject(() => rows.nth(9999).waitFor());
    assert.equal(await rows.count(), 10000);
    await page.locator("#run").click();
    assert.equal(await rows.count(), 1000);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    await new Promise((resolvePromise) => server.close(resolvePromise));
  }
});
