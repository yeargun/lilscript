export const BASE = import.meta.env?.BASE_URL ?? "/";

export function withBase(path) {
  if (!path) return path;
  if (/^(https?:|mailto:|data:)/i.test(path)) return path;
  if (path.startsWith("#")) return path;
  const hashIndex = path.indexOf("#");
  const hash = hashIndex === -1 ? "" : path.slice(hashIndex);
  const rest = hashIndex === -1 ? path : path.slice(0, hashIndex);
  const queryIndex = rest.indexOf("?");
  const query = queryIndex === -1 ? "" : rest.slice(queryIndex);
  const file = queryIndex === -1 ? rest : rest.slice(0, queryIndex);
  return `${BASE}${file.replace(/^\//, "")}${query}${hash}`;
}

export function localPath(pathname) {
  const prefix = BASE.replace(/\/$/, "");
  if (prefix && pathname.startsWith(prefix)) {
    return pathname.slice(prefix.length) || "/";
  }
  return pathname;
}
