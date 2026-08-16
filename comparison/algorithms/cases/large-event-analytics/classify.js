export function prefixClass(token) {
  return token.startsWith("cache:") ? 11
    : token.startsWith("view:") ? 17
    : token.startsWith("data:") ? 23
    : 3;
}

export function suffixClass(token) {
  return token.endsWith(":hot") ? 13
    : token.endsWith(":cold") ? -5
    : token.endsWith(":idle") ? 7
    : 1;
}

export function keywordClass(token) {
  return token === "render" ? 29
    : token === "hydrate" ? 31
    : token === "commit" ? 37
    : token === "invalidate" ? 41
    : 5;
}

export function tokenClass(token) {
  return (prefixClass(token) * 3 | 0) + (suffixClass(token) * 5 | 0) + keywordClass(token) | 0;
}
