// Derive a library's production-test adapter from its own `npm test` script.
//
// 001 requires every maintained library's original suite to be frozen as a case
// inventory. Writing one bespoke discovery script per library is what made that
// work unbounded. The adapter is not a new contract: it is the original command,
// parsed. Anything this parser cannot account for becomes an explicit deviation
// or an unsupported result, never a silently different command.

// Glob stars are ordinary in these selections; every other shell
// metacharacter is refused by `tokenize` below.
const WORD = /^[A-Za-z0-9:_@./*-]+$/

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
 * Returns `{ supported, prerequisites, nodeArguments, patterns, deviations,
 * segments, unsupported }`. `supported` is true only when exactly one
 * `node ... --test <patterns>` segment was found and every other segment is an
 * `npm run <script>` this discovery can either execute or explicitly drop.
 */
export function deriveAdapter(script, { dropBuild = true } = {}) {
  const result = {
    supported: false, script, segments: [], prerequisites: [], nodeArguments: [],
    patterns: [], deviations: [], unsupported: [],
  }
  if (typeof script !== "string" || !script.trim()) {
    result.unsupported.push("package.json declares no test script")
    return result
  }
  const segments = splitSegments(script)
  result.segments = segments
  let testSegment = null
  for (const segment of segments) {
    const tokens = tokenize(segment)
    if (!tokens) { result.unsupported.push(`segment is not a plain command: ${segment}`); continue }
    if (tokens[0] === "npm" && tokens[1] === "run" && tokens.length === 3) {
      const name = tokens[2]
      if (dropBuild && BUILD_SCRIPTS.has(name)) {
        result.deviations.push(`dropped original build segment \`npm run ${name}\`: an existing-distribution observation must not recompile dist; the source build is a separate 001 baseline task`)
      } else {
        result.prerequisites.push({ id: `original-${name}`, script: name, segment })
      }
      continue
    }
    if (tokens[0] === "node") {
      const flagEnd = tokens.indexOf("--test")
      if (flagEnd < 0) { result.unsupported.push(`node segment does not run the built-in test runner: ${segment}`); continue }
      if (testSegment) { result.unsupported.push(`more than one node --test segment: ${segment}`); continue }
      const patterns = tokens.slice(flagEnd + 1)
      if (!patterns.length) { result.unsupported.push(`node --test segment names no files: ${segment}`); continue }
      if (patterns.some(pattern => pattern.startsWith("-"))) { result.unsupported.push(`node --test segment mixes flags into its file list: ${segment}`); continue }
      testSegment = segment
      result.nodeArguments = tokens.slice(1, flagEnd)
      result.patterns = patterns
      continue
    }
    result.unsupported.push(`segment is neither \`npm run <script>\` nor \`node --test\`: ${segment}`)
  }
  if (!testSegment) {
    if (!result.unsupported.length) result.unsupported.push("no node --test segment found")
    return result
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
  return derivation.segments.join(" && ")
}

export { BUILD_SCRIPTS }
