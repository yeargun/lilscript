const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function firstExisting(paths) {
  return paths.find((candidate) => candidate && fs.existsSync(candidate));
}

function serverCommand(context) {
  const configured = vscode.workspace.getConfiguration("lilscript").get("server.path", "").trim();
  if (configured) return configured;

  const repositoryRoot = path.resolve(context.extensionPath, "..");
  const workspaceCandidates = (vscode.workspace.workspaceFolders ?? []).flatMap((folder) => [
    path.join(folder.uri.fsPath, "target", "release", "lilscript-lsp"),
    path.join(folder.uri.fsPath, "target", "debug", "lilscript-lsp"),
  ]);
  const local = firstExisting([
    path.join(repositoryRoot, "target", "release", "lilscript-lsp"),
    path.join(repositoryRoot, "target", "debug", "lilscript-lsp"),
    ...workspaceCandidates,
  ]);
  return local ?? "lilscript-lsp";
}

async function activate(context) {
  const command = serverCommand(context);
  const serverOptions = {
    run: { command, args: [], transport: TransportKind.stdio },
    debug: { command, args: [], transport: TransportKind.stdio },
  };
  const clientOptions = {
    documentSelector: [
      { scheme: "file", language: "lilscript" },
      { scheme: "untitled", language: "lilscript" },
    ],
    synchronize: {
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.lil"),
        vscode.workspace.createFileSystemWatcher("**/lilscript.toml"),
      ],
    },
    outputChannelName: "LilScript Language Server",
  };

  client = new LanguageClient(
    "lilscript",
    "LilScript Language Server",
    serverOptions,
    clientOptions,
  );
  await client.start();
}

async function deactivate() {
  if (client) await client.stop();
}

module.exports = { activate, deactivate };
