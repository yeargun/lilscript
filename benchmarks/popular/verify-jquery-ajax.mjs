import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { build as esbuild } from "esbuild";
import { mkdirSync } from "node:fs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const buildRoot = join(labRoot, "build");
const compiled = join(labRoot, "ports/jquery/jquery-lilscript.raw.js");
const outFile = join(buildRoot, "jquery-lilscript-ajax.js");

mkdirSync(buildRoot, { recursive: true });

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

run(compiler, [
  join(labRoot, "ports/jquery/entry.lil"),
  "--mode",
  "development",
  "--target",
  "js-module",
  "-o",
  compiled,
]);

await esbuild({
  absWorkingDir: join(labRoot, "ports/jquery"),
  entryPoints: [compiled],
  outfile: outFile,
  bundle: true,
  format: "esm",
  platform: "neutral",
  write: true,
});

class MockXHR {
  constructor() {
    this.readyState = 0;
    this.status = 0;
    this.statusText = "";
    this.response = "";
    this.responseText = "";
    this.withCredentials = false;
    this._reqHeaders = {};
    this._respHeaders = {};
  }
  open(method, url, async) {
    this.method = method;
    this.url = url;
    this.async = async === undefined ? true : async;
    this.readyState = 1;
  }
  setRequestHeader(name, value) {
    this._reqHeaders[name] = value;
  }
  overrideMimeType(type) {
    this._mime = type;
  }
  getAllResponseHeaders() {
    return Object.entries(this._respHeaders)
      .map(([k, v]) => `${k}: ${v}\r\n`)
      .join("");
  }
  getResponseHeader(name) {
    const key = Object.keys(this._respHeaders).find((k) => k.toLowerCase() === name.toLowerCase());
    return key ? this._respHeaders[key] : null;
  }
  send(body) {
    this._body = body;
    const mock = MockXHR.nextResponse || { status: 200, headers: {}, text: "" };
    const respond = () => {
      this.readyState = 4;
      this.status = mock.status;
      this.statusText = mock.statusText || "";
      this.responseText = mock.text === undefined ? "" : mock.text;
      this.response = this.responseText;
      this._respHeaders = mock.headers || {};
      if (mock.status >= 200 && mock.status < 400) {
        if (this.onload) this.onload();
      } else {
        if (this.onerror) this.onerror();
      }
      if (this.onreadystatechange) this.onreadystatechange();
    };
    if (mock.async === false) {
      respond();
    } else {
      setTimeout(respond, mock.delay || 0);
    }
  }
  abort() {
    this.readyState = 0;
    if (this.onabort) this.onabort();
  }
}

const require = createRequire(import.meta.url);
const upstreamFactory = require("jquery");
const { JSDOM } = await import("jsdom");
const dom = new JSDOM("<!doctype html><html><body></body></html>", { url: "http://localhost/" });
dom.window.XMLHttpRequest = MockXHR;
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const upstream = upstreamFactory(dom.window);
const lilModule = await import(outFile);
const lil = lilModule.jQuery;

function settle(ms = 30) {
  return new Promise((r) => setTimeout(r, ms));
}

function installMockTransport($) {
  let current = null;
  const state = { lastHeaders: null, lastBody: undefined };
  $.ajaxTransport("+*", function (options) {
    const mock = options.mockResponse || current;
    if (!mock) return undefined;
    return {
      send(headers, complete) {
        state.lastHeaders = headers;
        state.lastBody = options.data;
        const respond = () =>
          complete(
            mock.status === undefined ? 200 : mock.status,
            mock.statusText === undefined ? "OK" : mock.statusText,
            mock.responses || { text: mock.text === undefined ? "" : mock.text },
            mock.headers === undefined ? "" : mock.headers,
          );
        if (mock.async === false) {
          respond();
        } else {
          setTimeout(respond, mock.delay || 0);
        }
      },
      abort() {
        if (mock.onAbort) mock.onAbort();
      },
    };
  });
  return {
    set(v) {
      current = v;
    },
    get lastHeaders() {
      return state.lastHeaders;
    },
    get lastBody() {
      return state.lastBody;
    },
  };
}

const mockU = installMockTransport(upstream);
const mockL = installMockTransport(lil);

let scriptOutcome = { type: "load", callbackPayload: undefined };
function installScriptInterceptor(win) {
  const realCreateElement = win.document.createElement.bind(win.document);
  win.document.createElement = function (tag) {
    const el = realCreateElement(tag);
    if (String(tag).toLowerCase() === "script") {
      let srcValue = "";
      Object.defineProperty(el, "src", {
        get() {
          return srcValue;
        },
        set(v) {
          srcValue = v;
          queueMicrotask(() => {
            try {
              const url = new URL(v, "http://localhost/");
              const cb = url.searchParams.get("callback");
              if (cb && typeof win[cb] === "function" && scriptOutcome.type === "load") {
                win[cb](scriptOutcome.callbackPayload);
              }
            } catch {
              /* ignore malformed url in tests */
            }
            el.dispatchEvent(new win.Event(scriptOutcome.type));
          });
        },
      });
    }
    return el;
  };
}
installScriptInterceptor(dom.window);

async function runBoth(name, fn) {
  const outU = await fn(upstream, mockU, dom.window);
  const outL = await fn(lil, mockL, dom.window);
  assert.deepEqual(outL, outU, `${name}: lil !== upstream`);
  console.log(`jquery-ajax:${name}:ok`);
}

await runBoth("ajaxSetup-merge", async ($) => {
  const before = $.extend({}, $.ajaxSettings.accepts);
  const settings = $.ajaxSetup({ custom: "value", accepts: { mine: "text/mine" } });
  const out = [settings.custom, settings.accepts.mine, settings.accepts.text, $.ajaxSettings.custom];
  $.ajaxSetup({ custom: undefined, accepts: before });
  delete $.ajaxSettings.custom;
  return out;
});

await runBoth("ajax-success-json", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, statusText: "OK", text: '{"a":1,"b":[1,2,3]}', headers: { "Content-Type": "application/json" } });
  const { xhr: jqXHR } = await new Promise((resolve) => {
    $.ajax({
      url: "/api/data",
      dataType: "json",
      success(data, status, xhr) {
        out.push(["success", data, status]);
        resolve({ xhr });
      },
    });
  });
  out.push(["status", jqXHR.status, jqXHR.statusText]);
  out.push(["header", jqXHR.getResponseHeader("Content-Type")]);
  out.push(["responseJSON", jqXHR.responseJSON]);
  mock.set(null);
  return out;
});

await runBoth("ajax-error-404", async ($, mock) => {
  const out = [];
  mock.set({ status: 404, statusText: "Not Found", text: "oops" });
  await new Promise((resolve) => {
    $.ajax({
      url: "/api/missing",
      dataType: "text",
      error(xhr, status, err) {
        out.push(["error", xhr.status, status, err]);
        resolve();
      },
    });
  });
  mock.set(null);
  return out;
});

await runBoth("ajax-statusCode", async ($, mock) => {
  const out = [];
  mock.set({ status: 201, statusText: "Created", text: "" });
  await new Promise((resolve) => {
    $.ajax({
      url: "/api/create",
      statusCode: {
        201() {
          out.push("hit-201");
        },
        404() {
          out.push("hit-404");
        },
      },
      complete() {
        resolve();
      },
    });
  });
  mock.set(null);
  return out;
});

await runBoth("ajax-global-events", async ($, mock) => {
  const out = [];
  const handlers = {
    ajaxSend: () => out.push("send"),
    ajaxSuccess: () => out.push("success"),
    ajaxComplete: () => out.push("complete"),
    ajaxStart: () => out.push("start"),
    ajaxStop: () => out.push("stop"),
  };
  Object.entries(handlers).forEach(([type, fn]) => $(document).on(type, fn));
  mock.set({ status: 200, text: "ok" });
  await new Promise((resolve) => {
    $.ajax({ url: "/api/global", complete: resolve });
  });
  await settle(10);
  Object.entries(handlers).forEach(([type, fn]) => $(document).off(type, fn));
  mock.set(null);
  return out;
});

await runBoth("ajax-dataFilter-converters", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, text: "raw-payload" });
  const data = await new Promise((resolve) => {
    $.ajax({
      url: "/api/filtered",
      dataType: "text",
      dataFilter(response, type) {
        return response.toUpperCase();
      },
      success(d) {
        resolve(d);
      },
    });
  });
  out.push(data);
  mock.set(null);
  return out;
});

await runBoth("ajax-abort", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, text: "late", delay: 500 });
  const jqXHR = $.ajax({
    url: "/api/slow",
    error(xhr, status, err) {
      out.push(["error", status, err]);
    },
  });
  jqXHR.abort();
  await settle(20);
  out.push(["state", jqXHR.readyState]);
  mock.set(null);
  return out;
});

await runBoth("ajax-timeout", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, text: "never", delay: 500 });
  await new Promise((resolve) => {
    $.ajax({
      url: "/api/timeout",
      timeout: 20,
      error(xhr, status, err) {
        out.push(["error", status, err]);
        resolve();
      },
    });
  });
  mock.set(null);
  return out;
});

await runBoth("ajax-post-contentType", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, text: "{}" });
  await new Promise((resolve) => {
    $.ajax({
      url: "/api/post",
      type: "POST",
      data: { a: 1, b: "x" },
      dataType: "json",
      complete: resolve,
    });
  });
  out.push(["content-type", mock.lastHeaders["Content-Type"]]);
  out.push(["body", mock.lastBody]);
  mock.set(null);
  return out;
});

await runBoth("getJSON-shortcut", async ($, mock) => {
  mock.set({ status: 200, text: '{"ok":true}' });
  const data = await new Promise((resolve) => {
    $.getJSON("/api/json", (d) => resolve(d));
  });
  mock.set(null);
  return data;
});

await runBoth("get-post-shortcuts", async ($, mock) => {
  const out = [];
  mock.set({ status: 200, text: "get-body" });
  await new Promise((resolve) => $.get("/api/get", (d) => { out.push(["get", d]); resolve(); }));
  mock.set({ status: 200, text: "post-body" });
  await new Promise((resolve) => $.post("/api/post2", { x: 1 }, (d) => { out.push(["post", d]); resolve(); }));
  mock.set(null);
  return out;
});

await runBoth("ajax-deprecated-alias", async ($, mock) => {
  const out = [];
  const handler = () => out.push("aliased-success");
  $(document).ajaxSuccess(handler);
  mock.set({ status: 200, text: "ok" });
  await new Promise((resolve) => $.ajax({ url: "/api/alias", complete: resolve }));
  await settle(10);
  $(document).off("ajaxSuccess", handler);
  mock.set(null);
  return out;
});

await runBoth("script-transport-success", async ($, mock, win) => {
  scriptOutcome = { type: "load", callbackPayload: undefined };
  const out = [];
  const data = await new Promise((resolve) => {
    $.ajax({
      url: "/api/some.js",
      dataType: "script",
      crossDomain: true,
      success(d, status, xhr) {
        resolve([d, status, xhr.status]);
      },
    });
  });
  out.push(data);
  return out;
});

await runBoth("script-transport-error", async ($) => {
  scriptOutcome = { type: "error", callbackPayload: undefined };
  const out = [];
  await new Promise((resolve) => {
    $.ajax({
      url: "/api/missing.js",
      dataType: "script",
      crossDomain: true,
      error(xhr, status, err) {
        out.push([xhr.status, status]);
        resolve();
      },
    });
  });
  scriptOutcome = { type: "load", callbackPayload: undefined };
  return out;
});

await runBoth("getScript-shortcut", async ($) => {
  scriptOutcome = { type: "load", callbackPayload: undefined };
  const out = [];
  await new Promise((resolve) => {
    $.getScript("http://example.com/lib.js", (d, status, xhr) => {
      out.push([d, status, xhr.status]);
      resolve();
    });
  });
  return out;
});

await runBoth("jsonp-roundtrip", async ($) => {
  scriptOutcome = { type: "load", callbackPayload: [{ hello: "world" }] };
  const out = [];
  const data = await new Promise((resolve) => {
    $.ajax({
      url: "/api/jsonp?callback=?",
      dataType: "jsonp",
      crossDomain: true,
      success(d) {
        resolve(d);
      },
    });
  });
  out.push(data);
  scriptOutcome = { type: "load", callbackPayload: undefined };
  return out;
});

await runBoth("load-html-with-selector", async ($, mock, win) => {
  mock.set({ status: 200, text: '<div id="wrap"><p class="target">A</p><p>B</p></div>' });
  const el = win.document.createElement("div");
  const col = $.merge($(), [el]);
  const out = await new Promise((resolve) => {
    col.load("/api/page.html .target", function (response, status, xhr) {
      resolve([this.innerHTML, status, xhr.status]);
    });
  });
  mock.set(null);
  return out;
});

await runBoth("load-html-no-selector", async ($, mock, win) => {
  mock.set({ status: 200, text: "<span>whole page</span>" });
  const el = win.document.createElement("div");
  const col = $.merge($(), [el]);
  const out = await new Promise((resolve) => {
    col.load("/api/whole.html", function () {
      resolve(this.innerHTML);
    });
  });
  mock.set(null);
  return out;
});

await runBoth("xhr-transport-real", async ($, mock, win) => {
  const out = [];
  MockXHR.nextResponse = { status: 200, statusText: "OK", text: "raw-xhr-text", headers: { "X-Test": "yes" } };
  const { xhr: jqXHR } = await new Promise((resolve) => {
    $.ajax({
      url: "/api/rawxhr",
      dataType: "text",
      complete(xhr) {
        resolve({ xhr });
      },
    });
  });
  out.push(["status", jqXHR.status, jqXHR.statusText]);
  out.push(["header", jqXHR.getResponseHeader("X-Test")]);
  out.push(["allHeaders", jqXHR.getAllResponseHeaders().trim()]);
  MockXHR.nextResponse = null;
  return out;
});

console.log("jquery-ajax:all:ok");
