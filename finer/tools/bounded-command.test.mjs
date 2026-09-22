import assert from "node:assert/strict"
import { spawn } from "node:child_process"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { setTimeout as delay } from "node:timers/promises"
import test from "node:test"
import { fileURLToPath } from "node:url"
import { runBoundedCommand } from "./bounded-command.mjs"

const options = { encoding: "utf8", timeoutMs: 2000, killAfterMs: 100, maxBuffer: 1024 * 1024 }

test("bounded commands preserve arguments, output and successful or failing status", async () => {
  const run = await runBoundedCommand(process.execPath, ["-e", "console.log(process.argv[1]);console.error('diagnostic');process.exit(7)", "a ; literal argument"], options)
  assert.equal(run.error, undefined)
  assert.equal(run.status, 7)
  assert.equal(run.stdout, "a ; literal argument\n")
  assert.equal(run.stderr, "diagnostic\n")
  assert.equal((await runBoundedCommand(process.execPath, ["-e", ""], options)).status, 0)
})

test("invalid or ambiguous deadlines cannot start an unbounded command", () => {
  for (const timeoutMs of [0, -1, Infinity, NaN, 0.5, undefined]) {
    assert.throws(() => runBoundedCommand("missing", [], { ...options, timeoutMs }), /positive integer/)
  }
  assert.throws(() => runBoundedCommand("missing", [], { ...options, killAfterMs: 0 }), /positive integer/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, timeout: 500 }), /bounded runner/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, detached: true }), /bounded runner/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, stdio: "inherit" }), /bounded runner/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, signal: new AbortController().signal }), /bounded runner/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, killSignal: "SIGTERM" }), /bounded runner/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, timeoutMs: 2147483648 }), /maximum delay/)
  assert.throws(() => runBoundedCommand("missing", [], { ...options, killAfterMs: 2147483648 }), /maximum delay/)
  for (const encoding of ["not-an-encoding", "", 0, {}, true]) {
    assert.throws(() => runBoundedCommand("missing", [], { ...options, encoding }), /encoding/)
  }
})

test("missing executable remains an explicit failure", async () => {
  assert.notEqual((await runBoundedCommand("/missing/lilscript-command", [], options)).status, 0)
  const missing = await runBoundedCommand("missing-lilscript-command", [], { ...options, env: { ...process.env, PATH: "/missing" } })
  assert.equal(missing.error?.code, "ENOENT")
  assert.notEqual(missing.status, 0)
})

function active(pid) {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8")
    return !["Z", "X"].includes(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[0])
  } catch (error) {
    if (error.code === "ENOENT") return false
    throw error
  }
}

function killGroupFile(path) {
  if (!existsSync(path)) return
  const pid = Number(readFileSync(path, "utf8"))
  assert.ok(Number.isSafeInteger(pid) && pid > 0)
  try { process.kill(-pid, "SIGKILL") }
  catch (error) { if (error.code !== "ESRCH") throw error }
}

async function waitForFiles(paths) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (paths.every(path => existsSync(path))) return
    await delay(20)
  }
  throw new Error(`process fixture did not start: ${paths}`)
}

test("scoped supervision hooks are removed after completion and synchronous spawn rejection", async () => {
  const events = ["exit", "SIGINT", "SIGTERM"]
  const before = events.map(event => process.listeners(event))
  await runBoundedCommand(process.execPath, ["-e", ""], options)
  assert.deepEqual(events.map(event => process.listeners(event)), before)
  await assert.rejects(runBoundedCommand(process.execPath, [], { ...options, cwd: 42 }))
  assert.deepEqual(events.map(event => process.listeners(event)), before)
  const binary = await runBoundedCommand(process.execPath, ["-e", "process.stdout.write('ok')"], { ...options, encoding: null })
  assert.ok(Buffer.isBuffer(binary.stdout))
  assert.equal(binary.stdout.toString(), "ok")
})

test("concurrent owners retain shutdown hooks until the final group settles", async () => {
  const events = ["exit", "SIGINT", "SIGTERM"]
  const before = events.map(event => process.listeners(event))
  const longer = runBoundedCommand(process.execPath, ["-e", "setTimeout(()=>{},300)"], options)
  const shorter = await runBoundedCommand(process.execPath, ["-e", ""], options)
  assert.equal(shorter.status, 0)
  for (const [index, event] of events.entries()) assert.equal(process.listenerCount(event), before[index].length + 1)
  assert.equal((await longer).status, 0)
  assert.deepEqual(events.map(event => process.listeners(event)), before)
})

for (const mode of ["exit", "deadline", "output"]) {
  test(`escaped inherited pipe holder cannot prevent bounded settlement after ${mode}`, async t => {
    const events = ["exit", "SIGINT", "SIGTERM"]
    const before = events.map(event => process.listeners(event))
    const directory = mkdtempSync(join(tmpdir(), "lilscript-bounded-escaped-"))
    const pidFile = join(directory, "escaped.pid")
    t.after(() => { killGroupFile(pidFile); rmSync(directory, { recursive: true, force: true }) })
    const escaped = "setInterval(()=>{},100)"
    const activity = mode === "exit" ? "setTimeout(()=>process.exit(0),150)"
      : mode === "output" ? "setInterval(()=>process.stdout.write('x'.repeat(65536)),10)"
      : "process.on('SIGTERM',()=>{});setInterval(()=>{},100)"
    const parent = `const child=require('node:child_process').spawn(process.execPath,['-e',${JSON.stringify(escaped)}],{detached:true,stdio:'inherit'});require('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(child.pid));child.unref();${activity};`
    const started = performance.now()
    const result = await runBoundedCommand(process.execPath, ["-e", parent], {
      ...options, timeoutMs: mode === "deadline" ? 300 : 2000, maxBuffer: 2000,
    })
    assert.ok(performance.now() - started < 3000, "an escaped pipe must not defeat owner timeout")
    assert.equal(result.status, null)
    assert.equal(result.error?.code, { exit: "ECLEANUP", deadline: "ETIMEDOUT", output: "ENOBUFS" }[mode])
    assert.equal(result.supervision.forcedPipeClosure, true)
    assert.equal(result.supervision.childExitObserved, true)
    assert.equal(result.supervision.timedOut, mode === "deadline")
    assert.deepEqual(events.map(event => process.listeners(event)), before)
    const pid = Number(readFileSync(pidFile, "utf8"))
    assert.equal(active(pid), true, "escaped sessions are explicitly outside group kill coverage")
    killGroupFile(pidFile)
    await delay(100)
    assert.equal(active(pid), false)
  })
}

for (const mode of ["exit", "SIGINT", "SIGTERM", "existing-handler"]) {
  test(`supervisor ${mode} cleans detached groups without changing termination semantics`, async t => {
    const directory = mkdtempSync(join(tmpdir(), "lilscript-bounded-owner-"))
    const parentPid = join(directory, "parent.pid"), workerPid = join(directory, "worker.pid")
    let supervisor
    t.after(() => {
      supervisor?.kill("SIGKILL")
      killGroupFile(parentPid)
      rmSync(directory, { recursive: true, force: true })
    })
    const worker = `require('node:fs').writeFileSync(${JSON.stringify(workerPid)},String(process.pid));process.on('SIGTERM',()=>{});setInterval(()=>{},100);`
    const parent = `require('node:fs').writeFileSync(${JSON.stringify(parentPid)},String(process.pid));require('node:child_process').spawn(process.execPath,['-e',${JSON.stringify(worker)}],{stdio:'ignore'});process.on('SIGTERM',()=>{});setInterval(()=>{},100);`
    const runnerUrl = new URL("./bounded-command.mjs", import.meta.url).href
    const termination = mode === "exit"
      ? `const check=setInterval(()=>{if(existsSync(${JSON.stringify(workerPid)})){clearInterval(check);process.exit(17)}},10);`
      : mode === "existing-handler" ? "process.on('SIGTERM',()=>process.exit(23));" : ""
    const script = `import{existsSync}from'node:fs';import{runBoundedCommand}from${JSON.stringify(runnerUrl)};${termination}await runBoundedCommand(process.execPath,['-e',${JSON.stringify(parent)}],{timeoutMs:60000,killAfterMs:100});`
    supervisor = spawn(process.execPath, ["--input-type=module", "-e", script], { stdio: "ignore" })
    const completion = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("supervisor did not terminate")), 5000)
      supervisor.once("error", error => { clearTimeout(timer); reject(error) })
      supervisor.once("close", (code, signal) => { clearTimeout(timer); resolve({ code, signal }) })
    })
    await waitForFiles([parentPid, workerPid])
    if (mode !== "exit") supervisor.kill(mode === "existing-handler" ? "SIGTERM" : mode)
    const result = await completion
    if (mode === "exit" || mode === "existing-handler") {
      assert.equal(result.code, mode === "exit" ? 17 : 23)
      assert.equal(result.signal, null)
    } else {
      assert.equal(result.code, null)
      assert.equal(result.signal, mode)
    }
    await delay(150)
    assert.equal(active(Number(readFileSync(parentPid, "utf8"))), false)
    assert.equal(active(Number(readFileSync(workerPid, "utf8"))), false)
  })
}

for (const inheritPipes of [true, false]) {
  test(`deadline terminates a TERM-resistant grandchild with ${inheritPipes ? "inherited" : "closed"} pipes`, async t => {
    const directory = mkdtempSync(join(tmpdir(), "lilscript-bounded-command-"))
    t.after(() => rmSync(directory, { recursive: true, force: true }))
    const pidFile = join(directory, "child.pid")
    const heartbeat = join(directory, "heartbeat")
    const child = `const fs=require('node:fs');process.on('SIGTERM',()=>{});fs.writeFileSync(${JSON.stringify(pidFile)},String(process.pid));setInterval(()=>fs.appendFileSync(${JSON.stringify(heartbeat)},'.'),10);`
    const parent = `const {spawn}=require('node:child_process');process.on('SIGTERM',()=>{});spawn(process.execPath,['-e',${JSON.stringify(child)}],{stdio:${JSON.stringify(inheritPipes ? "inherit" : "ignore")}});setInterval(()=>{},100);`
    const started = performance.now()
    const result = await runBoundedCommand(process.execPath, ["-e", parent], { ...options, timeoutMs: 800 })
    assert.notEqual(result.status, 0)
    assert.ok(performance.now() - started < 6000, "supervisor must not wait for the orphan's pipes")
    const pid = Number(readFileSync(pidFile, "utf8"))
    const before = readFileSync(heartbeat, "utf8")
    assert.ok(before.length > 0, "grandchild must actually run before the deadline")
    await delay(150)
    assert.equal(readFileSync(heartbeat, "utf8"), before, "grandchild must stop doing work")
    assert.equal(active(pid), false, "grandchild must not remain runnable")
    assert.equal(result.supervision.timeoutMs, 800)
  })
}

for (const inheritPipes of [true, false]) test(`successful parent exit cleans a background worker with ${inheritPipes ? "inherited" : "closed"} pipes`, async t => {
  const directory = mkdtempSync(join(tmpdir(), "lilscript-bounded-success-"))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  const pidFile = join(directory, "child.pid")
  const child = `require('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(process.pid));setInterval(()=>{},100);`
  const parent = `const worker=require('node:child_process').spawn(process.execPath,['-e',${JSON.stringify(child)}],{stdio:${JSON.stringify(inheritPipes ? "inherit" : "ignore")}});worker.unref();setTimeout(()=>{},200);`
  const result = await runBoundedCommand(process.execPath, ["-e", parent], options)
  assert.equal(result.status, 0)
  const pid = Number(readFileSync(pidFile, "utf8"))
  await delay(100)
  assert.equal(active(pid), false)
})

test("output limits stop work and cannot certify a successful exit", async () => {
  const result = await runBoundedCommand(process.execPath, ["-e", "setInterval(()=>process.stdout.write('x'.repeat(65536)),1)"], { ...options, maxBuffer: 2000 })
  assert.notEqual(result.status, 0)
  assert.equal(result.error?.code, "ENOBUFS")
  assert.equal(result.stdout.length, 2000)
  assert.equal(result.supervision.timedOut, false)
})

test("portgate records a timed-out build without inheriting artifacts or orphaning its compiler", async t => {
  const root = mkdtempSync(join(tmpdir(), "lilscript-bounded-portgate-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const workspace = join(root, "boundedlil")
  mkdirSync(join(workspace, "scripts"), { recursive: true })
  const pidFile = join(root, "compiler.pid")
  const child = `require('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(process.pid));process.on('SIGTERM',()=>{});setInterval(()=>{},100);`
  writeFileSync(join(workspace, "scripts/build.mjs"), `import {spawn} from 'node:child_process';process.on('SIGTERM',()=>{});spawn(process.execPath,['-e',${JSON.stringify(child)}],{stdio:'inherit'});setInterval(()=>{},100);`)
  const manifest = join(root, "workloads.json")
  writeFileSync(manifest, JSON.stringify({ libraries: [{ id: "boundedlil", required: true, kind: "maintained-library", workspace }] }))
  const out = join(root, "evidence")
  const command = fileURLToPath(new URL("./portgate.mjs", import.meta.url))
  const result = await runBoundedCommand(process.execPath, [command, "record", "--manifest", manifest, "--ports", "boundedlil", "--compiler", process.execPath, "--codec", process.execPath, "--out", out, "--timeout", "0.6"], { ...options, timeoutMs: 6000 })
  assert.equal(result.status, 1, result.stderr)
  const receipt = JSON.parse(readFileSync(join(out, "boundedlil.json"), "utf8"))
  assert.equal(receipt.build.trust, "untrustworthy")
  assert.notEqual(receipt.build.status, 0)
  assert.equal(receipt.build.supervision.timeoutMs, 600)
  assert.deepEqual(receipt.build.invocations, [])
  assert.deepEqual(receipt.artifacts, [])
  assert.equal(receipt.tests, null)
  const pid = Number(readFileSync(pidFile, "utf8"))
  await delay(100)
  assert.equal(active(pid), false)
})

test("portgate preserves a failed codec receipt after a trustworthy fixture build", async t => {
  const root = mkdtempSync(join(tmpdir(), "lilscript-bounded-codec-"))
  const pidFile = join(root, "codec.pid")
  t.after(() => { killGroupFile(pidFile); rmSync(root, { recursive: true, force: true }) })
  const workspace = join(root, "codeclil")
  mkdirSync(join(workspace, "scripts"), { recursive: true })
  writeFileSync(join(workspace, "entry.lil"), "export int value=42;\n")
  writeFileSync(join(workspace, "scripts/build.mjs"), "import {spawnSync} from 'node:child_process';process.exitCode=spawnSync(process.env.LILSCRIPT_COMPILER,['entry.lil','-o','dist/index.js'],{stdio:'inherit'}).status??1;\n")
  const compiler = join(root, "fixture-compiler"), codec = join(root, "fixture-codec")
  writeFileSync(compiler, `#!${process.execPath}\nconst fs=require('node:fs'),path=require('node:path');const output=process.argv[process.argv.indexOf('-o')+1];fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,'export const value=42;\\n');\n`, { mode: 0o755 })
  writeFileSync(codec, `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(process.pid));process.on('SIGTERM',()=>{});setInterval(()=>{},100);\n`, { mode: 0o755 })
  const manifest = join(root, "workloads.json")
  writeFileSync(manifest, JSON.stringify({ libraries: [{ id: "codeclil", required: true, kind: "maintained-library", workspace }] }))
  const out = join(root, "evidence"), command = fileURLToPath(new URL("./portgate.mjs", import.meta.url))
  const result = await runBoundedCommand(process.execPath, [command, "record", "--manifest", manifest, "--ports", "codeclil", "--compiler", compiler, "--codec", codec, "--skip-tests", "--out", out, "--timeout", "0.8"], { ...options, timeoutMs: 6000 })
  assert.equal(result.status, 1, result.stderr)
  const receipt = JSON.parse(readFileSync(join(out, "codeclil.json"), "utf8"))
  assert.equal(receipt.build.status, 0)
  assert.equal(receipt.build.invocations.length, 1)
  assert.deepEqual(receipt.build.invocations[0].validationErrors, [])
  assert.equal(receipt.build.trust, "untrustworthy")
  assert.match(receipt.build.why, /codec command did not complete/)
  assert.equal(receipt.measurement.status, null)
  assert.equal(receipt.measurement.supervision.timedOut, true)
  assert.deepEqual(receipt.artifacts, [])
  assert.equal(receipt.tests, null)
  const pid = Number(readFileSync(pidFile, "utf8"))
  await delay(100)
  assert.equal(active(pid), false)
})
