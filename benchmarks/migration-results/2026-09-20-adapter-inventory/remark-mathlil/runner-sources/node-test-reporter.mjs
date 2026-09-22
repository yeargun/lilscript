// Node's structured event stream is the source of test identities and results.
// Do not infer a green test run by scraping human-readable TAP/Jest messages.
import { relative, resolve } from "node:path"

export default async function* report(source) {
  const occurrences = new Map()
  for await (const event of source) {
    if (event.type === "test:pass" || event.type === "test:fail") {
      const row = event.data
      // A file with no node:test declarations can produce a synthetic file
      // success. It cannot populate a behavioral case inventory.
      if (!row.file || resolve(row.name) === resolve(row.file)) continue
      const signature = [relative(process.cwd(), row.file), row.line, row.column, row.nesting, row.details?.type ?? "test", row.name]
      const key = JSON.stringify(signature)
      const occurrence = (occurrences.get(key) ?? 0) + 1
      occurrences.set(key, occurrence)
      yield `${JSON.stringify({ kind: "test-case", id: JSON.stringify([...signature, occurrence]), file: signature[0], name: row.name, type: row.details?.type ?? "test", status: row.skip ? "skip" : row.todo ? "todo" : event.type === "test:pass" ? "pass" : "fail", error: row.details?.error?.message ?? null })}\n`
    } else if (event.type === "test:stdout" || event.type === "test:stderr" || event.type === "test:diagnostic") {
      yield `${JSON.stringify({ kind: "diagnostic", event: event.type, message: event.data.message })}\n`
    }
  }
}
