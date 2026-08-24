#!/usr/bin/env node
/* Name-assignment strategies, applied through the scope analyser so every
   variant is a legal rewrite of the same program.

   The question these answer: given the same set of bindings, does *which*
   name each one gets change the compressed size — beyond the length of the
   name? Frequency order is what a normal mangler does. First-use order is
   the cheapest way to make two similar functions spell themselves the same
   way. The n-gram beam is the greedy version of "look like the text that is
   already behind us in the window". */
import { RESERVED } from "./scope.mjs";

/* Letters ordered by how often the file already spells them outside the
   names we are about to choose. A mangler that reaches for `a, b, c` is
   using an alphabet from 1995; the file itself knows which letters its
   Huffman tree has already paid for. */
export function adaptiveAlphabet(analysis, { mode = "all" } = {}) {
  const counts = new Map();
  const skip = new Set();
  for (const binding of analysis.bindings) {
    if (!binding.renamable) continue;
    for (const ref of binding.references) for (let i = ref.start; i < ref.end; i++) skip.add(i);
  }
  const source = analysis.source;
  if (mode === "dialect") {
    /* Which single letters does this file already use as local names? Keeping
       its own dialect costs nothing and preserves whatever the last mangler
       taught the Huffman tree. */
    for (const binding of analysis.bindings) {
      if (binding.name.length !== 1) continue;
      counts.set(binding.name, (counts.get(binding.name) || 0) + binding.count);
    }
  } else if (mode === "token") {
    /* Only letters in token-initial position: an identifier lands after a
       delimiter, and Brotli prices a literal by the two bytes before it. */
    for (let i = 1; i < source.length; i++) {
      if (skip.has(i)) continue;
      const ch = source[i];
      if (!/[A-Za-z_$]/.test(ch)) continue;
      if (/[A-Za-z0-9_$.]/.test(source[i - 1])) continue;
      counts.set(ch, (counts.get(ch) || 0) + 1);
    }
  } else {
    for (let i = 0; i < source.length; i++) {
      if (skip.has(i)) continue;
      const ch = source[i];
      if (!/[A-Za-z_$]/.test(ch)) continue;
      counts.set(ch, (counts.get(ch) || 0) + 1);
    }
  }
  const letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_".split("");
  letters.sort((a, b) => (counts.get(b) || 0) - (counts.get(a) || 0) || a.localeCompare(b));
  return letters.join("");
}

export const ALPHABETS = {
  /* the canonical base-54 order a mangler reaches for */
  abc: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_",
  /* letters the file already breathes: from function / return / length / typeof */
  etn: "etnioarsclufhpdmgvbywkxjqzETNIOARSCLUFHPDMGVBYWKXJQZ$_",
};

export function twoCharNames(alphabet) {
  const out = [];
  for (const a of alphabet) out.push(a);
  for (const a of alphabet) for (const b of alphabet) out.push(a + b);
  return out;
}

export const firstUse = (b) => Math.min(...b.references.map((n) => n.start));

export const ORDERS = {
  frequency: (a, b) => b.count - a.count || firstUse(a) - firstUse(b),
  firstUse: (a, b) => firstUse(a) - firstUse(b),
  source: (a, b) => a.declarations[0].start - b.declarations[0].start,
};

/* --- availability -----------------------------------------------------
   A name N cannot go to binding B when:
     1. another binding in B's own scope already holds it (or still spells
        itself that way);
     2. some other binding named N, or a free name N, is referenced anywhere
        inside B's scope subtree — B would capture it;
     3. a scope between one of B's references and B's own scope declares N —
        that declaration would shadow B at that reference.
   Everything else is fair game, including a name some unrelated nested
   function uses for its own locals. That reuse is the whole point. */
export function createAllocator(analysis) {
  const outerBindings = new Map(); /* scope -> Set(binding) declared above, used inside */
  const freeNames = new Map();     /* scope -> Set(name) */
  for (const scope of analysis.scopes) { outerBindings.set(scope, new Set()); freeNames.set(scope, new Set()); }
  for (const binding of analysis.bindings) {
    for (const ref of binding.references) {
      for (let s = ref.__scope || binding.scope; s && s !== binding.scope; s = s.parent) {
        outerBindings.get(s).add(binding);
      }
    }
  }
  for (const [name, nodes] of analysis.unresolved) {
    for (const node of nodes) {
      for (let s = node.__scope; s; s = s.parent) freeNames.get(s).add(name);
    }
  }

  const spelling = (binding, mapping) => mapping.get(binding) || binding.name;

  return {
    /* Names this scope must avoid, whatever binding is being named. */
    scopeForbidden(scope, mapping) {
      const forbidden = new Set(RESERVED);
      for (const name of freeNames.get(scope)) forbidden.add(name);
      for (const outer of outerBindings.get(scope)) {
        forbidden.add(outer.name);
        forbidden.add(spelling(outer, mapping));
      }
      for (const b of scope.bindings.values()) if (!b.renamable) forbidden.add(b.name);
      return forbidden;
    },
    /* Names shadowed between a reference of `binding` and its own scope. */
    shadowed(binding, mapping) {
      const names = new Set();
      for (const ref of binding.references) {
        for (let s = ref.__scope; s && s !== binding.scope; s = s.parent) {
          for (const other of s.bindings.values()) {
            names.add(other.name);
            names.add(spelling(other, mapping));
          }
        }
      }
      return names;
    },
  };
}

/* Assign every renamable binding a name. `pick` chooses among the available
   names for one binding, and is where a strategy lives. */
export function assign(analysis, { order = "frequency", alphabet = "abc", pick = null, onScope = null } = {}) {
  const names = twoCharNames(ALPHABETS[alphabet] || alphabet);
  const allocator = createAllocator(analysis);
  const compare = ORDERS[order] || ORDERS.frequency;
  const mapping = new Map();
  /* Outer scopes first: an inner scope must know what its parents took. */
  const scopes = [...analysis.scopes].sort((a, b) => a.id - b.id);
  for (const scope of scopes) {
    const bindings = [...scope.bindings.values()].filter((b) => b.renamable);
    if (onScope) onScope(scope, mapping);
    if (!bindings.length) continue;
    bindings.sort(compare);
    const base = allocator.scopeForbidden(scope, mapping);
    const taken = new Set();
    for (const binding of bindings) {
      const forbidden = new Set(base);
      for (const name of taken) forbidden.add(name);
      for (const name of allocator.shadowed(binding, mapping)) forbidden.add(name);
      for (const other of bindings) if (other !== binding) forbidden.add(other.name);
      const available = () => names.filter((n) => !forbidden.has(n));
      const chosen = pick
        ? pick({ binding, scope, forbidden, names, mapping, available })
        : names.find((n) => !forbidden.has(n));
      if (!chosen || forbidden.has(chosen)) continue;
      mapping.set(binding, chosen);
      taken.add(chosen);
      taken.add(binding.name);
    }
  }
  return mapping;
}

/* --- the n-gram model -------------------------------------------------- */

/* Every n-gram already emitted. A candidate spelling scores by how much of
   it the window has seen before. */
export class NgramModel {
  constructor(n = 8) { this.n = n; this.seen = new Set(); }
  add(text) {
    const { n, seen } = this;
    for (let i = 0; i + n <= text.length; i++) seen.add(text.slice(i, i + n));
  }
  score(text) {
    const { n, seen } = this;
    let hits = 0;
    for (let i = 0; i + n <= text.length; i++) if (seen.has(text.slice(i, i + n))) hits++;
    return hits;
  }
}

/* A closer proxy for what the codec will do: greedy LZ77 over everything
   emitted so far. `covered` is how many of the candidate's bytes a copy
   could supply, which is the quantity a match finder is maximizing. */
export class LzModel {
  constructor(minMatch = 6) {
    this.minMatch = minMatch;
    this.text = "";
    this.index = new Map(); /* minMatch-gram -> last position */
    this.freq = new Float64Array(256);
    this.total = 0;
  }
  add(chunk) {
    const start = this.text.length;
    this.text += chunk;
    const { minMatch, index, text } = this;
    for (let i = Math.max(0, start - minMatch + 1); i + minMatch <= text.length; i++) {
      index.set(text.slice(i, i + minMatch), i);
    }
    for (let i = 0; i < chunk.length; i++) { this.freq[chunk.charCodeAt(i) & 0xff]++; this.total++; }
  }
  covered(candidate) {
    const { minMatch, index, text } = this;
    let matched = 0, i = 0;
    while (i + minMatch <= candidate.length) {
      const at = index.get(candidate.slice(i, i + minMatch));
      if (at === undefined) { i++; continue; }
      let len = minMatch;
      while (at + len < text.length && i + len < candidate.length && text[at + len] === candidate[i + len]) len++;
      matched += len;
      i += len;
    }
    return matched;
  }

  /* Coverage alone is the wrong objective: a codec pays per command as well
     as per literal, and a literal's price is its rarity. This estimates the
     bits a Brotli-shaped coder would spend on `candidate` given the history:
     order-0 literal entropy plus a flat price per copy command. */
  estimateBits(candidate, { commandBits = 22 } = {}) {
    const { minMatch, index, text } = this;
    let bits = 0, i = 0;
    while (i < candidate.length) {
      if (i + minMatch <= candidate.length) {
        const at = index.get(candidate.slice(i, i + minMatch));
        if (at !== undefined) {
          let len = minMatch;
          while (at + len < text.length && i + len < candidate.length && text[at + len] === candidate[i + len]) len++;
          bits += commandBits;
          i += len;
          continue;
        }
      }
      bits += this.literalBits(candidate.charCodeAt(i));
      i++;
    }
    return bits;
  }
  literalBits(code) {
    const count = this.freq[code] || 0;
    return -Math.log2((count + 0.5) / (this.total + 128));
  }
}

/* Render one scope's span with the current mapping plus a candidate overlay. */
export function renderScope(analysis, scope, mapping, overlay) {
  const { start, end } = scope.node;
  const edits = [];
  const walk = (s) => {
    for (const binding of s.bindings.values()) {
      const name = (overlay && overlay.get(binding)) || mapping.get(binding);
      if (!name || name === binding.name) continue;
      for (const ref of binding.references) {
        if (ref.start >= start && ref.end <= end) {
          edits.push({ start: ref.start, end: ref.end,
            text: binding.shorthandNodes.has(ref) ? `${binding.name}: ${name}` : name });
        }
      }
    }
    for (const child of s.children) walk(child);
  };
  walk(scope);
  /* bindings from outer scopes referenced inside this span */
  for (const binding of analysis.bindings) {
    const name = (overlay && overlay.get(binding)) || mapping.get(binding);
    if (!name || name === binding.name) continue;
    if (binding.scope.node && binding.scope.node.start >= start && binding.scope.node.end <= end) continue;
    for (const ref of binding.references) {
      if (ref.start >= start && ref.end <= end) {
        edits.push({ start: ref.start, end: ref.end, text: name });
      }
    }
  }
  edits.sort((a, b) => a.start - b.start);
  let out = "", cursor = start;
  for (const edit of edits) {
    if (edit.start < cursor) continue;
    out += analysis.source.slice(cursor, edit.start) + edit.text;
    cursor = edit.end;
  }
  return out + analysis.source.slice(cursor, end);
}
