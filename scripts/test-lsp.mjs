import { spawn } from "node:child_process";
import path from "node:path";

const server = path.resolve(process.argv[2] ?? "target/release/lilscript-lsp");
const child = spawn(server, [], { stdio: ["pipe", "pipe", "pipe"] });
const childExit = new Promise((resolve, reject) => {
  child.once("exit", (code) => {
    if (code === 0) resolve();
    else reject(new Error(`Language server exited with ${code}.`));
  });
});
let buffer = Buffer.alloc(0);
let stderr = "";
const queued = [];
const waiters = [];

child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => {
  stderr += chunk;
});

function dispatch(message) {
  const index = waiters.findIndex((waiter) => waiter.predicate(message));
  if (index === -1) {
    queued.push(message);
    return;
  }
  const [waiter] = waiters.splice(index, 1);
  clearTimeout(waiter.timer);
  waiter.resolve(message);
}

child.stdout.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) return;
    const header = buffer.subarray(0, headerEnd).toString("ascii");
    const match = /Content-Length:\s*(\d+)/i.exec(header);
    if (!match) throw new Error(`LSP response has no Content-Length: ${header}`);
    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    if (buffer.length < bodyStart + length) return;
    const body = buffer.subarray(bodyStart, bodyStart + length).toString("utf8");
    buffer = buffer.subarray(bodyStart + length);
    dispatch(JSON.parse(body));
  }
});

function send(message) {
  const body = JSON.stringify(message);
  child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function receive(predicate, label) {
  const queuedIndex = queued.findIndex(predicate);
  if (queuedIndex !== -1) return Promise.resolve(queued.splice(queuedIndex, 1)[0]);
  return new Promise((resolve, reject) => {
    const waiter = { predicate, resolve, timer: undefined };
    waiter.timer = setTimeout(() => {
      const index = waiters.indexOf(waiter);
      if (index !== -1) waiters.splice(index, 1);
      reject(new Error(`Timed out waiting for ${label}. Server stderr:\n${stderr}`));
    }, 5000);
    waiters.push(waiter);
  });
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const uri = "file:///tmp/lilscript-lsp-smoke.lil";
send({
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: { processId: process.pid, capabilities: {}, rootUri: null },
});
const initialized = await receive((message) => message.id === 1, "initialize response");
assert(initialized.result.capabilities.hoverProvider === true, "hover capability is missing");
send({ jsonrpc: "2.0", method: "initialized", params: {} });

send({
  jsonrpc: "2.0",
  method: "textDocument/didOpen",
  params: {
    textDocument: {
      uri,
      languageId: "lilscript",
      version: 1,
      text: 'int broken="wrong";',
    },
  },
});
const invalidDiagnostics = await receive(
  (message) => message.method === "textDocument/publishDiagnostics",
  "invalid diagnostics",
);
assert(invalidDiagnostics.params.diagnostics.length === 1, "invalid source produced no diagnostic");

const validSource = [
  "struct Point { int x; int y; }",
  "int[] values = [1, 2, 3];",
  "auto mapped = values.map((int value) => value * 2);",
  "print(mapped.length);",
].join("\n");
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: 2 },
    contentChanges: [{ text: validSource }],
  },
});
const validDiagnostics = await receive(
  (message) => message.method === "textDocument/publishDiagnostics",
  "cleared diagnostics",
);
assert(validDiagnostics.params.diagnostics.length === 0, "valid source retained diagnostics");

send({
  jsonrpc: "2.0",
  id: 2,
  method: "textDocument/completion",
  params: { textDocument: { uri }, position: { line: 2, character: 20 } },
});
const completion = await receive((message) => message.id === 2, "completion response");
assert(completion.result.items.some((item) => item.label === "mapped"), "document completion is missing");

const mapCharacter = validSource.split("\n")[2].lastIndexOf("map") + 1;
send({
  jsonrpc: "2.0",
  id: 3,
  method: "textDocument/hover",
  params: { textDocument: { uri }, position: { line: 2, character: mapCharacter } },
});
const hover = await receive((message) => message.id === 3, "hover response");
assert(hover.result.contents.value.includes("Transforms every array element"), "map hover is missing");

send({
  jsonrpc: "2.0",
  id: 4,
  method: "textDocument/documentSymbol",
  params: { textDocument: { uri } },
});
const symbols = await receive((message) => message.id === 4, "document symbols");
assert(symbols.result.some((symbol) => symbol.name === "Point"), "struct symbol is missing");
assert(symbols.result.some((symbol) => symbol.name === "mapped"), "binding symbol is missing");

send({ jsonrpc: "2.0", id: 5, method: "shutdown", params: null });
await receive((message) => message.id === 5, "shutdown response");
send({ jsonrpc: "2.0", method: "exit", params: null });
child.stdin.end();

await childExit.catch((error) => {
  throw new Error(`${error.message} Server stderr:\n${stderr}`);
});

console.log("LilScript language-server protocol passed.");
