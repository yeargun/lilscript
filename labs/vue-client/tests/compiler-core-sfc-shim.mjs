import { parse } from "@babel/parser";

export function babelParse(source, options = {}) {
  return parse(source, {
    sourceType: "module",
    plugins: ["typescript"],
    ...options,
  });
}

export function walkIdentifiers(root, onIdentifier, includeAll = false) {
  function visit(node, parent, parentStack) {
    if (!node || typeof node !== "object") return;
    const stack = parent ? [...parentStack, parent] : parentStack;
    if (node.type === "Identifier") {
      const isReference = false;
      if (includeAll || isReference) {
        onIdentifier(node, parent, stack, isReference, false);
      }
    }
    for (const [key, value] of Object.entries(node)) {
      if (key === "loc" || key === "start" || key === "end") continue;
      if (Array.isArray(value)) {
        for (const child of value) visit(child, node, stack);
      } else if (value?.type) {
        visit(value, node, stack);
      }
    }
  }
  visit(root, null, []);
}
