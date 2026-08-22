import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "../apps/monaco/e2e-out");
mkdirSync(outDir, { recursive: true });

const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const base = process.env.MONACO_IDE_BASE || "http://127.0.0.1:8787/apps/monaco";

async function runSuite(page, name) {
  const failures = [];
  const note = (ok, msg) => {
    if (!ok) failures.push(msg);
    console.log((ok ? "  ok  " : "  FAIL") + " " + name + " " + msg);
  };

  await page.goto(base + "/" + name + "/", { waitUntil: "load", timeout: 30000 });
  await page.waitForSelector(".view-line", { timeout: 20000 });
  await page.waitForTimeout(800);
  await page.waitForFunction(() => {
    const markers = window.monaco?.editor?.getModelMarkers?.({}) || [];
    return markers.some((m) => /string/.test(m.message || ""));
  }, { timeout: 20000 });

  if (name === "lil") {
    const plugged = await page.evaluate(() => !!window.__lilEditor);
    note(plugged, "page is the LilScript monaco port (no monaco-editor JS)");
  }

  const painted = await page.locator(".view-line").count();
  note(painted >= 8, `paints source lines (${painted})`);

  await page.locator("#editor").click({ position: { x: 90, y: 18 } });
  await page.evaluate(() => {
    const ed = window.monaco.editor.getEditors()[0];
    ed.setPosition({ lineNumber: 1, column: 1 });
    ed.trigger("keyboard", "type", { text: "/*e2e*/" });
  });
  const typed = await page.evaluate(() => window.monaco.editor.getEditors()[0].getValue().includes("/*e2e*/"));
  note(typed, "editor.trigger type inserts text");
  await page.evaluate(() => window.monaco.editor.getEditors()[0].trigger("keyboard", "undo", null));

  await page.locator("#editor").click({ position: { x: 90, y: 18 } });
  await page.waitForTimeout(50);
  const beforeKeys = await page.evaluate(() => window.monaco.editor.getEditors()[0].getValue());
  await page.keyboard.type("HELLO_LIL");
  const keyed = await page.evaluate(() => window.monaco.editor.getEditors()[0].getValue().includes("HELLO_LIL"));
  note(keyed, "real keyboard typing inserts text");
  await page.evaluate((value) => window.monaco.editor.getEditors()[0].setValue(value), beforeKeys);

  if (name === "lil") {
    const editorBox = await page.locator("#editor").boundingBox();
    const clickX = editorBox.x + 180;
    const clickY = editorBox.y + 90;
    await page.mouse.click(clickX, clickY, { button: "right" });
    await page.waitForTimeout(150);
    const menuOk = await page.evaluate(({ clickX, clickY }) => {
      const el = document.querySelector(".context-view");
      if (!el) return false;
      const cs = getComputedStyle(el);
      if (cs.display === "none") return false;
      const r = el.getBoundingClientRect();
      return Math.abs(r.left - clickX) < 24 && Math.abs(r.top - clickY) < 24;
    }, { clickX, clickY });
    note(menuOk, "context menu opens at the click");
    const menuItems = await page.evaluate(() => (document.querySelector(".context-view")?.innerText || "").replace(/\s+/g, " "));
    note(/Go to Definition/.test(menuItems) && /Go to References/.test(menuItems) && /Command Palette/.test(menuItems), `context menu items match monaco (${menuItems.slice(0, 120)})`);
    note(!/Toggle Line Comment/.test(menuItems), "context menu is not the homemade cut/copy list");
    await page.keyboard.press("Escape");
  }

  await page.evaluate(() => {
    const ed = window.monaco.editor.getEditors()[0];
    ed.setPosition({ lineNumber: 1, column: 8 });
    ed.focus();
  });
  await page.keyboard.press("Control+Space");
  await page.waitForTimeout(900);
  const suggestText = await page.evaluate(() => {
    const nodes = [...document.querySelectorAll(".suggest-widget .monaco-list-row, .suggest-widget")];
    return nodes.map((n) => n.textContent || "").join("\n");
  });
  const suggestVisible = await page.evaluate(() => {
    const el = document.querySelector(".editor-widget.suggest-widget, .suggest-widget");
    if (!el) return false;
    const cs = getComputedStyle(el);
    return cs.display !== "none" && el.getBoundingClientRect().height > 8;
  });
  note(suggestVisible, `suggest widget visible (${suggestText.slice(0, 80).replace(/\s+/g, " ")})`);
  note(/export|function|greet|add|const/.test(suggestText), "suggest lists symbols/keywords");

  await page.keyboard.press("Escape");
  await page.evaluate(() => {
    const ed = window.monaco.editor.getEditors()[0];
    const action = ed.getAction?.("actions.find");
    if (action?.run) action.run();
    else ed.trigger("keyboard", "actions.find", null);
  });
  await page.waitForTimeout(500);
  const findOpen = await page.evaluate(() => {
    const widget = document.querySelector(".editor-widget.find-widget, .find-widget");
    if (!widget) return false;
    if (widget.classList.contains("visible")) return true;
    const r = widget.getBoundingClientRect();
    return r.height > 16 && r.width > 16;
  });
  note(findOpen, "find widget opens");

  await page.evaluate(() => {
    const ed = window.monaco.editor.getEditors()[0];
    ed.setPosition({ lineNumber: 1, column: 20 });
    ed.focus();
    const action = ed.getAction?.("editor.action.showHover");
    if (action?.run) action.run();
  });
  await page.waitForTimeout(1500);
  const hoverText = await page.evaluate(() => {
    const nodes = [...document.querySelectorAll(".monaco-hover, .monaco-resizable-hover")];
    for (const el of nodes) {
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      if (cs.display === "none" || r.height < 4) continue;
      const text = (el.textContent || "").trim();
      if (text) return text;
    }
    return "";
  });
  note(/greet|function|string/.test(hoverText), `hover shows info (${hoverText.slice(0, 80)})`);

  await page.evaluate(async () => {
    const ed = window.monaco.editor.getEditors()[0];
    const models = window.monaco.editor.getModels();
    const main = models.find((m) => String(m.uri?.toString?.() ?? m.uri ?? "").includes("main.ts"));
    if (main) ed.setModel(main);
    ed.setPosition({ lineNumber: 1, column: 12 });
    ed.focus();
    if (window.__ideGotoDef) await window.__ideGotoDef();
  });
  await page.waitForTimeout(400);
  const defInfo = await page.evaluate(() => {
    const ed = window.monaco.editor.getEditors()[0];
    const model = ed.getModel();
    const pos = ed.getPosition();
    return {
      uri: String(model?.uri?.toString?.() ?? ""),
      line: pos?.lineNumber ?? 0,
      column: pos?.column ?? 0,
      word: model?.getWordAtPosition?.(pos)?.word ?? "",
    };
  });
  note(/app\.ts/.test(defInfo.uri), `go to definition (${defInfo.uri}:${defInfo.line}:${defInfo.column} ${defInfo.word || ""})`);

  const markers = await page.evaluate(() =>
    (window.monaco.editor.getModelMarkers({}) || []).map((m) => m.message).join(" | "),
  );
  note(/string/.test(markers) && /number/.test(markers), "type error marker present");

  if (name === "lil") {
    const tsWorker = await page.evaluate(async () => {
      const tsApi = window.monaco?.typescript ?? window.monaco?.languages?.typescript;
      const get = tsApi?.getTypeScriptWorker;
      if (typeof get !== "function") return { ok: false, reason: "no getTypeScriptWorker" };
      const models = window.monaco.editor.getModels();
      const model = models.find((m) => String(m.uri?.toString?.() ?? m.uri ?? "").includes("app.ts"));
      if (!model) return { ok: false, reason: "no app.ts model" };
      const worker = await get();
      const client = await worker(model.uri);
      const diags = await client.getSemanticDiagnostics(String(model.uri.toString()));
      const text = (diags ?? []).map((d) => String(d.messageText?.messageText ?? d.messageText ?? "")).join(" | ");
      return { ok: /string/.test(text) && /number/.test(text), reason: text.slice(0, 160) };
    });
    note(tsWorker.ok, `official TypeScript worker diagnostics (${tsWorker.reason})`);

    const solidOk = await page.evaluate(async () => {
      const tsApi = window.monaco?.typescript ?? window.monaco?.languages?.typescript;
      const get = tsApi?.getTypeScriptWorker;
      const models = window.monaco.editor.getModels();
      const model = models.find((m) => String(m.uri?.toString?.() ?? m.uri ?? "").includes("App.tsx"));
      if (typeof get !== "function" || !model) return { ok: false, reason: "no App.tsx" };
      const worker = await get();
      const client = await worker(model.uri);
      const diags = await client.getSemanticDiagnostics(String(model.uri.toString()));
      const text = (diags ?? []).map((d) => String(d.messageText?.messageText ?? d.messageText ?? "")).join(" | ");
      return {
        ok: !/Cannot find module 'solid-js'/.test(text) && !/jsx-runtime/.test(text),
        reason: text.slice(0, 160) || "solid-js resolved",
      };
    });
    note(solidOk.ok, `Solid TSX uses the TypeScript worker (${solidOk.reason})`);
  }

  await page.keyboard.press("Escape");
  await page.keyboard.press("F1");
  await page.waitForTimeout(300);
  const palette = await page.evaluate(() => document.getElementById("quick-open")?.classList.contains("open"));
  note(palette, "command palette opens");
  await page.keyboard.press("Escape");

  await page.screenshot({ path: join(outDir, name + ".png"), fullPage: true });
  return failures;
}

const browser = await chromium.launch({
  executablePath: chrome,
  headless: true,
});
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
const lilFails = await runSuite(page, "lil");
const jsFails = await runSuite(page, "js");
await browser.close();

const report = { lilFails, jsFails };
writeFileSync(join(outDir, "e2e.json"), JSON.stringify(report, null, 2) + "\n");
console.log("\nsummary", report);
assert.equal(lilFails.length, 0, "Lil e2e failures:\n" + lilFails.join("\n"));
assert.equal(jsFails.length, 0, "JS e2e failures:\n" + jsFails.join("\n"));
