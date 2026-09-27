// Validation of the idiom debt ledger (tests/idiom-debt.json) against the rules
// of its schema (tests/idiom-debt.schema.json), without a schema engine: the
// repository's scripts run on Node alone.
import { existsSync } from "node:fs";
import { join } from "node:path";

const ENTRY = ["id", "port", "recorded", "idiomatic", "workaround", "case", "codec", "sizes", "owner", "status"];
const OPTIONAL = ["paid", "evidence"];

function checkForm(form, where, problems) {
  if (!form || typeof form !== "object") return problems.push(`${where}: missing`);
  if (typeof form.description !== "string" || !form.description.trim()) problems.push(`${where}.description: required`);
  for (const key of Object.keys(form)) if (!["description", "files", "commit"].includes(key)) problems.push(`${where}: unknown field ${key}`);
  if (form.files !== undefined && !(Array.isArray(form.files) && form.files.every((file) => typeof file === "string"))) problems.push(`${where}.files: a list of paths`);
}

export function validateIdiomDebt(document, repository = null) {
  const problems = [];
  if (document?.schema !== 1) problems.push("schema must be 1");
  if (typeof document?.about !== "string" || !document.about.trim()) problems.push("about: required");
  if (!Array.isArray(document?.entries)) return [...problems, "entries: a list"];
  const ids = new Set();
  document.entries.forEach((entry, index) => {
    const where = `entry ${index + 1}${entry?.id ? ` (${entry.id})` : ""}`;
    for (const key of ENTRY) if (entry[key] === undefined) problems.push(`${where}: ${key} is required`);
    for (const key of Object.keys(entry)) if (!ENTRY.includes(key) && !OPTIONAL.includes(key)) problems.push(`${where}: unknown field ${key}`);
    if (!/^NO4-[0-9]{3}$/.test(entry.id ?? "")) problems.push(`${where}: id must look like NO4-001`);
    if (ids.has(entry.id)) problems.push(`${where}: duplicate id`);
    ids.add(entry.id);
    if (!/^[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(entry.recorded ?? "")) problems.push(`${where}: recorded must be a date`);
    checkForm(entry.idiomatic, `${where}.idiomatic`, problems);
    checkForm(entry.workaround, `${where}.workaround`, problems);
    if (!/^(comparison\/cases\/canonical|tests\/cases)\//.test(entry.case ?? "")) problems.push(`${where}: case must be a regression case under comparison/cases/canonical or tests/cases`);
    else if (repository && !existsSync(join(repository, entry.case))) problems.push(`${where}: case ${entry.case} does not exist`);
    if (!["brotli11", "gzip9", "raw"].includes(entry.codec)) problems.push(`${where}: codec must be brotli11, gzip9 or raw`);
    const sizes = entry.sizes ?? {};
    for (const key of ["idiomatic", "workaround"]) if (!Number.isInteger(sizes[key]) || sizes[key] < 0) problems.push(`${where}: sizes.${key} must be a byte count`);
    if (typeof sizes.binary !== "string" || !sizes.binary) problems.push(`${where}: sizes.binary names the measuring compiler`);
    if (typeof entry.owner !== "string" || !entry.owner.trim()) problems.push(`${where}: owner (a plan task) is required`);
    if (!["open", "paid"].includes(entry.status)) problems.push(`${where}: status must be open or paid`);
    if (entry.status === "paid" && !entry.paid) problems.push(`${where}: a paid entry records what paid it`);
    if (entry.status === "open" && Number.isInteger(sizes.idiomatic) && Number.isInteger(sizes.workaround) && sizes.idiomatic <= sizes.workaround) {
      problems.push(`${where}: size(idiomatic) <= size(workaround): the debt is paid; set status to paid`);
    }
  });
  return problems;
}
