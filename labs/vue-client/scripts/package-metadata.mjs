import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {mkdirSync,readFileSync,writeFileSync,statSync} from "node:fs";
import {dirname,relative,resolve} from "node:path";
import {projectRoot} from "../tooling/compiler-path.mjs";
const inventory=readJson(resolve(projectRoot,"compatibility/inventory.json"));
const formatManifest=readJson(resolve(projectRoot,"compatibility/package-formats.json"));
const packageRows=new Map(formatManifest.packages.map(entry=>[entry.name,entry]));
const upstreamRoot=resolve(projectRoot,"upstream/vue");
const declarationRows=[];
function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function relativePath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function artifact(path, extra = {}) {
  const bytes = readFileSync(path);
  return {
    path: relativePath(path),
    bytes: bytes.byteLength,
    sha256: sha256(bytes),
    ...extra,
  };
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function replaceWorkspaceVersions(value) {
  if (Array.isArray(value)) return value.map(replaceWorkspaceVersions);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, replaceWorkspaceVersions(entry)]),
    );
  }
  return typeof value === "string" && value.startsWith("workspace:")
    ? formatManifest.upstreamVersion
    : value;
}

function packageManifest(entry, audited) {
  const upstream = replaceWorkspaceVersions(
    readJson(resolve(upstreamRoot, audited.directory, "package.json")),
  );
  upstream.version = entry.name === "@vue/runtime-test"
    ? upstream.version
    : `${formatManifest.upstreamVersion}-vuelil`;
  upstream.private = true;
  if (entry.name === "vue") {
    upstream.type = "module";
    upstream.main = "./index.cjs";
    upstream.exports["."].require.node.production = "./dist/vue.cjs.prod.js";
    upstream.exports["."].require.node.development = "./dist/vue.cjs.js";
    upstream.exports["."].require.node.default = "./index.cjs";
    upstream.exports["."].require.default = "./index.cjs";
  }
  return upstream;
}

function declarationTarget(entry) {
  return resolve(entry.root, "dist", `${entry.filename}.d.ts`);
}

function copyDeclaration(source, target, packageName) {
  const sourceBytes = readFileSync(source);
  write(target, sourceBytes);
  const outputBytes = readFileSync(target);
  assert.equal(sha256(outputBytes), sha256(sourceBytes), `${packageName} declaration changed`);
  declarationRows.push({
    package: packageName,
    path: relativePath(target),
    source: relativePath(source),
    sourceSha256: sha256(sourceBytes),
    sha256: sha256(outputBytes),
    bytes: outputBytes.byteLength,
    exact: true,
  });
}

function runtimeTestDeclaration() {
  return `import type { CreateAppFunction, RootRenderFunction, VNode } from "@vue/runtime-core";
export * from "@vue/runtime-core";
export declare enum TestNodeTypes { TEXT = "text", ELEMENT = "element", COMMENT = "comment" }
export declare enum NodeOpTypes { CREATE = "create", INSERT = "insert", REMOVE = "remove", SET_TEXT = "setText", SET_ELEMENT_TEXT = "setElementText", PATCH = "patch" }
export interface TestElement { id: number; type: TestNodeTypes.ELEMENT; parentNode: TestElement | null; tag: string; children: TestNode[]; props: Record<string, any>; eventListeners: Record<string, Function | Function[]> | null; }
export interface TestText { id: number; type: TestNodeTypes.TEXT; parentNode: TestElement | null; text: string; }
export interface TestComment { id: number; type: TestNodeTypes.COMMENT; parentNode: TestElement | null; text: string; }
export type TestNode = TestElement | TestText | TestComment;
export interface NodeOp { type: NodeOpTypes; nodeType?: TestNodeTypes; tag?: string; text?: string; targetNode?: TestNode; parentNode?: TestElement; refNode?: TestNode | null; propKey?: string; propPrevValue?: any; propNextValue?: any; }
export declare function logNodeOp(op: NodeOp): void;
export declare function resetOps(): void;
export declare function dumpOps(): NodeOp[];
export declare const nodeOps: Record<string, Function>;
export declare function serialize(node: TestNode, indent?: number, depth?: number): string;
export declare function serializeInner(node: TestElement, indent?: number, depth?: number): string;
export declare function triggerEvent(el: TestElement, event: string, payload?: any[]): void;
export declare const render: RootRenderFunction<TestElement>;
export declare const createApp: CreateAppFunction<TestElement>;
export declare function renderToString(vnode: VNode): string;
`;
}

function compatDeclaration() {
  return `import type { CompatVue } from "@vue/runtime-core";
declare const Vue: CompatVue;
export default Vue;
`;
}

function copyDeclarations() {
  for (const audited of inventory.packages) {
    const entry = packageRows.get(audited.name);
    assert(entry, `missing package format manifest for ${audited.name}`);
    entry.root = resolve(projectRoot, entry.root);
    entry.source = resolve(projectRoot, entry.source);
    if (entry.runtimeSource) entry.runtimeSource = resolve(projectRoot, entry.runtimeSource);
    if (entry.declarationSource) {
      entry.declarationSource = resolve(projectRoot, entry.declarationSource);
      const sourceManifest = readJson(resolve(dirname(dirname(entry.declarationSource)), "package.json"));
      assert.equal(sourceManifest.version, formatManifest.upstreamVersion);
      copyDeclaration(entry.declarationSource, declarationTarget(entry), entry.name);
    } else {
      const contents = entry.name === "@vue/compat"
        ? compatDeclaration()
        : runtimeTestDeclaration();
      const target = declarationTarget(entry);
      write(target, contents);
      declarationRows.push({
        package: entry.name,
        path: relativePath(target),
        source: "generated from pinned public source declarations",
        sourceSha256: sha256(Buffer.from(contents)),
        sha256: sha256(readFileSync(target)),
        bytes: statSync(target).size,
        exact: true,
      });
    }
  }

  const vueRoot = packageRows.get("vue").root;
  for (const [source, target] of [
    ["node_modules/vue/dist/vue.d.mts", "dist/vue.d.mts"],
    ["node_modules/vue/jsx.d.ts", "jsx.d.ts"],
    ["node_modules/vue/jsx-runtime/index.d.ts", "jsx-runtime/index.d.ts"],
    ["node_modules/vue/compiler-sfc/index.d.ts", "compiler-sfc/index.d.ts"],
    ["node_modules/vue/compiler-sfc/index.d.mts", "compiler-sfc/index.d.mts"],
    ["node_modules/vue/server-renderer/index.d.ts", "server-renderer/index.d.ts"],
    ["node_modules/vue/server-renderer/index.d.mts", "server-renderer/index.d.mts"],
  ]) {
    copyDeclaration(resolve(projectRoot, source), resolve(vueRoot, target), "vue");
  }
}

export function writePackageMetadata() {
  copyDeclarations();
  for (const audited of inventory.packages) {
    const entry=packageRows.get(audited.name);
    write(resolve(entry.root,"package.json"),JSON.stringify(packageManifest(entry,audited),null,2)+"\n");
  }
  const root=packageRows.get("vue").root;
  write(resolve(root,"dist/package.json"),"{}\n");
  for(const name of ["compiler-sfc","server-renderer","jsx-runtime"]){
    write(resolve(root,name,"package.json"),JSON.stringify({main:"index.js",module:"index.mjs",types:"index.d.ts"},null,2)+"\n");
  }
  return declarationRows;
}
