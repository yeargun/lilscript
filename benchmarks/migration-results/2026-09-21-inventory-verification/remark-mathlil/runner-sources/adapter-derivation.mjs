// Derive a library's production-test adapter from its own `npm test` script.
//
// 001 requires every maintained library's original suite to be frozen as a case
// inventory. Writing one bespoke discovery script per library is what made that
// work unbounded. The adapter is not a new contract: it is the original command,
// parsed. Anything this parser cannot account for becomes an explicit deviation
// or an unsupported result, never a silently different command.

// Glob stars are ordinary in these selections; every other shell
// metacharacter is refused by `tokenize` below.
const WORD = /^[A-Za-z0-9:_@./*=-]+$/

// `npm run <script>` segments run before the suite. A build segment recompiles
// `dist`, so an existing-distribution observation must drop it and say so.
const BUILD_SCRIPTS = new Set(["build", "build:dev", "build:min", "prebuild", "compile"])

function tokenize(segment) {
  // Original test scripts in this fleet are plain words: no quotes, no
  // substitution, no redirection. Refuse anything else rather than guess.
  if (/["'`$\\<>|;&()]/.test(segment)) return null
  const tokens = segment.split(/\s+/).filter(Boolean)
  return tokens.every(token => WORD.test(token)) ? tokens : null
}

function splitSegments(script) {
  // Only `&&` sequencing is supported. `;`, `||` and `|` are rejected by the
  // tokenizer above, so a script that uses them yields an unsupported result.
  return script.split("&&").map(part => part.trim()).filter(Boolean)
}

/**
 * Parse an original `npm test` script into an adapter declaration.
 *
 * `scripts` is the library's whole `package.json` scripts map, so an
 * `npm run <name>` segment that itself contains the test runner is followed
 * instead of being executed blind. Returns `{ supported, prerequisites,
 * nodeArguments, patterns, deviations, uncovered, segments, unsupported }`.
 *
 * `supported` is true when exactly one `node ... --test <patterns>` segment was
 * found, reachable directly or through `npm run`, and every other segment is
 * either an executable prerequisite or an explicitly recorded omission.
 */
export function deriveAdapter(script, { dropBuild = true, scripts = {}, depth = 0, seen = new Set() } = {}) {
  const result = {
    supported: false, script, originalSegments: [], segments: [], prerequisites: [], nodeArguments: [],
    patterns: [], deviations: [], uncovered: [], unsupported: [],
  }
  if (typeof script !== "string" || !script.trim()) {
    result.unsupported.push("package.json declares no test script")
    return result
  }
  const segments = splitSegments(script)
  // `originalSegments` is the command exactly as written; `segments` grows to
  // show what a followed `npm run` wrapper actually contributed.
  result.originalSegments = segments
  result.segments = [...segments]
  let testSegment = null
  const pending = []
  for (const segment of segments) {
    const tokens = tokenize(segment)
    if (!tokens) { result.unsupported.push(`segment is not a plain command: ${segment}`); continue }
    if (tokens[0] === "npm" && tokens[1] === "run" && tokens.length === 3) {
      const name = tokens[2]
      if (dropBuild && BUILD_SCRIPTS.has(name)) {
        result.deviations.push(`dropped original build segment \`npm run ${name}\`: an existing-distribution observation must not recompile dist; the source build is a separate 001 baseline task`)
        continue
      }
      // A wrapper script such as `test:artifact` is where the real runner
      // lives. Follow it once rather than treating the wrapper as opaque.
      const body = scripts[name]
      if (body && !seen.has(name) && depth < 4 && /(^|\s)node\s/.test(body)) {
        const inner = deriveAdapter(body, { dropBuild, scripts, depth: depth + 1, seen: new Set([...seen, name]) })
        // Follow the wrapper only when the runner is genuinely inside it.
        // A script that merely happens to call node stays an ordinary
        // prerequisite, executed through `npm run` exactly as written.
        if (inner.patterns.length) {
          if (testSegment) { result.unsupported.push(`more than one node --test segment: ${segment}`); continue }
          result.segments.push(...inner.segments.map(part => `${name}: ${part}`))
          result.prerequisites.push(...inner.prerequisites)
          result.deviations.push(...inner.deviations)
          result.uncovered.push(...inner.uncovered)
          testSegment = segment
          result.nodeArguments = inner.nodeArguments
          result.patterns = inner.patterns
          result.deviations.push(`followed \`npm run ${name}\` to its own script to reach the test runner: ${body}`)
          continue
        }
      }
      pending.push({ id: `original-${name}`, script: name, segment })
      continue
    }
    if (tokens[0] === "node") {
      const flagEnd = tokens.indexOf("--test")
      if (flagEnd < 0) {
        // Another runner — jest, a hand-written driver — in the same command.
        // It is real required coverage this observation does not provide.
        result.uncovered.push(segment)
        continue
      }
      if (testSegment) { result.unsupported.push(`more than one node --test segment: ${segment}`); continue }
      const patterns = tokens.slice(flagEnd + 1)
      if (!patterns.length) { result.unsupported.push(`node --test segment names no files: ${segment}`); continue }
      if (patterns.some(pattern => pattern.startsWith("-"))) { result.unsupported.push(`node --test segment mixes flags into its file list: ${segment}`); continue }
      testSegment = segment
      result.nodeArguments = tokens.slice(1, flagEnd)
      result.patterns = patterns
      continue
    }
    result.unsupported.push(`segment is neither \`npm run <script>\` nor a node command: ${segment}`)
  }
  result.prerequisites.push(...pending)
  if (!testSegment) {
    // Without a built-in-runner segment there is nothing to observe; an
    // uncovered runner is then the reason, not a partial result.
    result.unsupported.push(...result.uncovered.map(segment => `segment does not run the built-in test runner: ${segment}`))
    if (!result.unsupported.length) result.unsupported.push("no node --test segment found")
    return result
  }
  for (const segment of result.uncovered) {
    result.deviations.push(`original command also runs \`${segment}\`, which is not the built-in test runner; this observation does not cover those cases`)
  }
  result.supported = result.unsupported.length === 0
  return result
}

/**
 * Rebuild the original script from a derivation, so a caller can prove the
 * parse lost nothing. Dropped build segments are reinserted at their original
 * position, which is why `segments` is retained.
 */
export function reconstruct(derivation) {
  return derivation.originalSegments.join(" && ")
}

export { BUILD_SCRIPTS }
