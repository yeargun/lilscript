import { spawn } from "node:child_process"
import { constants as bufferConstants } from "node:buffer"

const activeGroups = new Set()
const signals = ["SIGINT", "SIGTERM"]
const signalHandlers = new Map(signals.map(signal => [signal, () => {
  stopActiveGroups()
  // Preserve existing application handlers; otherwise restore Node's default
  // signal termination instead of swallowing it by installing this listener.
  if (process.listeners(signal).every(listener => listener === signalHandlers.get(signal))) {
    removeHooks()
    process.kill(process.pid, signal)
  }
}]))

function stopActiveGroups() {
  for (const stop of activeGroups) stop("SIGKILL")
}

function removeHooks() {
  process.removeListener("exit", stopActiveGroups)
  for (const [signal, handler] of signalHandlers) process.removeListener(signal, handler)
}

function ownGroup(stop) {
  if (!activeGroups.size) {
    process.prependListener("exit", stopActiveGroups)
    for (const [signal, handler] of signalHandlers) process.prependListener(signal, handler)
  }
  activeGroups.add(stop)
  return () => {
    activeGroups.delete(stop)
    if (!activeGroups.size) removeHooks()
  }
}

// Await one process-group owner, with a finite final wait for inherited pipes.
export function runBoundedCommand(command, args, options) {
  if (!options || typeof options !== "object") throw new TypeError("bounded command options are required")
  const { timeoutMs, killAfterMs = 1000, maxBuffer = 1024 * 1024, encoding, ...spawnOptions } = options
  for (const [name, value] of Object.entries({ timeoutMs, killAfterMs, maxBuffer })) {
    if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${name} must be a positive integer`)
  }
  if (timeoutMs > 2147483647 || killAfterMs > 2147483647) throw new Error("command timers exceed Node's maximum delay")
  if (maxBuffer > bufferConstants.MAX_LENGTH) throw new Error("maxBuffer exceeds Node's maximum buffer length")
  if (encoding != null && (typeof encoding !== "string" || !Buffer.isEncoding(encoding))) throw new TypeError("invalid command output encoding")
  if (["timeout", "detached", "stdio", "signal", "killSignal"].some(key => key in spawnOptions)) throw new Error("process deadlines and groups belong to the bounded runner")
  if (process.platform === "win32") throw new Error("bounded execution requires POSIX process groups")
  return new Promise((resolve, reject) => {
    let child, deadline, force, drain
    const chunks = [[], []], bytes = [0, 0]
    let error, status = null, signal = null, timedOut = false
    let settled = false, groupGone = false, childExitObserved = false, forcedPipeClosure = false
    function stop(signal) {
      if (groupGone || !(child?.pid > 0)) return
      try { process.kill(-child.pid, signal) }
      catch (failure) {
        if (failure.code === "ESRCH") groupGone = true
        else {
          error ??= failure
          try { child.kill("SIGKILL") } catch (fallback) { error ??= fallback }
        }
      }
    }
    const release = ownGroup(stop)
    try { child = spawn(command, args, { ...spawnOptions, detached: true, stdio: ["ignore", "pipe", "pipe"] }) }
    catch (failure) { release(); reject(failure); return }
    function finish() {
      if (settled) return
      settled = true
      clearTimeout(deadline)
      clearTimeout(force)
      clearTimeout(drain)
      release()
      const output = chunks.map(parts => Buffer.concat(parts))
      resolve({
        pid: child.pid, status: error ? null : status, signal, error,
        stdout: encoding ? output[0].toString(encoding) : output[0],
        stderr: encoding ? output[1].toString(encoding) : output[1],
        supervision: {
          timeoutMs, killAfterMs, maxBuffer, timedOut, childExitObserved, forcedPipeClosure,
          scope: "POSIX process group; escaped sessions are not killed; final pipe wait is bounded; SIGKILL of the supervisor cannot run cleanup",
        },
      })
    }
    function boundDrain() {
      if (drain || settled) return
      drain = setTimeout(() => {
        error ??= Object.assign(new Error("command cleanup did not close output pipes"), { code: "ECLEANUP" })
        forcedPipeClosure = true
        stop("SIGKILL")
        child.stdout.destroy()
        child.stderr.destroy()
        // A kernel-stuck child may not acknowledge SIGKILL. Do not keep this
        // owner alive indefinitely or misreport its missing exit as success.
        child.unref()
        finish()
      }, killAfterMs)
    }
    function forceStop() {
      stop("SIGKILL")
      boundDrain()
    }
    deadline = setTimeout(() => {
      timedOut = true
      error ??= Object.assign(new Error("command deadline exceeded"), { code: "ETIMEDOUT" })
      stop("SIGTERM")
      force = setTimeout(forceStop, killAfterMs)
    }, timeoutMs)
    for (const [index, stream] of [child.stdout, child.stderr].entries()) {
      stream.on("data", chunk => {
        if (settled) return
        const room = maxBuffer - bytes[index]
        if (room > 0) {
          const kept = chunk.length <= room ? chunk : Buffer.from(chunk.subarray(0, room))
          chunks[index].push(kept)
          bytes[index] += kept.length
        }
        if (chunk.length > room) {
          error ??= Object.assign(new Error("command output exceeds maxBuffer"), { code: "ENOBUFS" })
          forceStop()
        }
      })
      stream.on("error", failure => { if (!settled) { error ??= failure; forceStop() } })
    }
    child.on("error", failure => { if (!settled) { error ??= failure; forceStop() } })
    child.on("exit", (code, exitSignal) => {
      if (settled) return
      status = code
      signal = exitSignal
      childExitObserved = true
      forceStop()
    })
    child.on("close", (code, exitSignal) => {
      if (settled) return
      status = code
      signal = exitSignal
      finish()
    })
  })
}
