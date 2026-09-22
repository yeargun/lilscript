"""Executable architecture contract model; not a production compiler.

Immutable function units, tracked query dependencies, atomic edits, general
structured lowering and an independent small interpreter. No fixture text
escapes into the emitter. See lilscript-finer-structured/design/005-*.md.
"""
from __future__ import annotations

from collections import Counter, defaultdict
from dataclasses import dataclass, replace
from hashlib import sha256
from itertools import count
from types import MappingProxyType
import json


@dataclass(frozen=True)
class Op:
    kind: str
    args: tuple


def op(kind, *args):
    return Op(kind, tuple(args))


def lit(value):
    return op("lit", value)


def read(name):
    return op("read", name)


def walk(value):
    if isinstance(value, Op):
        yield value
        for child in value.args:
            yield from walk(child)
    elif isinstance(value, tuple):
        for child in value:
            yield from walk(child)


def rewrite(value, transform):
    if isinstance(value, Op):
        changed = transform(value)
        if changed is not None:
            return changed
        return Op(value.kind, tuple(rewrite(x, transform) for x in value.args))
    if isinstance(value, tuple):
        return tuple(rewrite(x, transform) for x in value)
    return value


@dataclass(frozen=True)
class Unit:
    key: str
    params: tuple[str, ...]
    captures: tuple[str, ...]
    body: tuple[Op, ...]
    export: str | None = None
    kind: str = "function"

    def facet(self, facet):
        return {"body": self.body, "signature": (self.params, self.kind),
                "captures": self.captures, "boundary": self.export}[facet]


@dataclass(frozen=True, order=True)
class Use:
    unit: str
    kind: str
    detail: str = ""


def unit_uses(unit):
    uses = defaultdict(list)
    for name in unit.captures:
        uses[("cell", name)].append(Use(unit.key, "capture"))
    for node in walk(unit.body):
        if node.kind == "read":
            uses[("cell", node.args[0])].append(Use(unit.key, "whole"))
        elif node.kind in ("get", "store"):
            uses[("cell", node.args[0])].append(Use(unit.key, "field", node.args[1]))
        elif node.kind in ("call", "closure", "global"):
            uses[("unit", node.args[0])].append(Use(unit.key, node.kind))
    return {key: tuple(value) for key, value in uses.items()}


_versions = count(1)  # Shared session allocator: sibling edits cannot collide.
FACETS = ("body", "signature", "captures", "boundary")


class Program:
    def __init__(self, units, previous=None):
        self.units = MappingProxyType(dict(units))
        stamps = dict(previous.stamps) if previous else {}
        unit_indexes = dict(previous.unit_indexes) if previous else {}
        affected = set()
        changed_indexes = set()
        self.reindexed = 0
        for key, unit in self.units.items():
            old = previous.units.get(key) if previous else None
            if old is unit:
                continue
            for facet in FACETS:
                if old is None or old.facet(facet) != unit.facet(facet):
                    stamps[("unit", key, facet)] = next(_versions)
            if old is None or old.body != unit.body or old.captures != unit.captures:
                affected.update(unit_indexes.get(key, {}))
                unit_indexes[key] = unit_uses(unit)
                affected.update(unit_indexes[key])
                changed_indexes.add(key)
                self.reindexed += 1
        if previous and set(previous.units) - set(self.units):
            raise ValueError("unit removal is outside this model's edit vocabulary")
        index = dict(previous.index) if previous else {}
        # Only affected entities get a new use set. Root maps are shallow copies;
        # this small model does not establish production root-table cost.
        for entity in affected:
            uses = tuple(sorted(
                [use for use in index.get(entity, ()) if use.unit not in changed_indexes]
                + [use for key in changed_indexes for use in unit_indexes[key].get(entity, ())]))
            if entity not in index or uses != index[entity]:
                index[entity] = uses
                stamps[("uses", *entity)] = next(_versions)
        self.stamps = MappingProxyType(stamps)
        self.unit_indexes = MappingProxyType(unit_indexes)
        self.index = MappingProxyType(index)

    def fingerprint(self):
        return sha256(repr(sorted(self.units.items())).encode()).hexdigest()


@dataclass(frozen=True)
class Effects:
    writes: bool = False
    throws: bool = False
    diverges: bool = False
    reads: bool = False
    identity: bool = False

    def union(self, other):
        return Effects(*(a or b for a, b in zip(self.bits(), other.bits())))

    def bits(self):
        return (self.writes, self.throws, self.diverges, self.reads, self.identity)

    def droppable(self):
        # Fresh unobserved identity and nonthrowing reads may be discarded.
        return not (self.writes or self.throws or self.diverges)

    def speculatable(self):
        return not any(self.bits())


UNKNOWN_EFFECTS = Effects(True, True, True, True, True)


@dataclass(frozen=True)
class Fact:
    answer: object
    dependencies: tuple

    def valid(self, program):
        return all(program.stamps.get(key, 0) == version
                   for key, version in self.dependencies)


class QueryContext:
    def __init__(self, program, cache):
        self.program, self.cache, self.dependencies = program, cache, {}

    def read(self, key):
        self.dependencies[key] = self.program.stamps.get(key, 0)

    def unit(self, key, facet):
        self.read(("unit", key, facet))
        return self.program.units[key]

    def query(self, key):
        fact = self.cache.query(self.program, key)
        self.dependencies.update(fact.dependencies)
        return fact.answer


class Queries:
    def __init__(self, capacity=512):
        self.entries, self.active = defaultdict(list), set()
        self.computed, self.hits = Counter(), Counter()
        self.capacity = capacity

    def query(self, program, key):
        for fact in self.entries[key]:
            if fact.valid(program):
                self.hits[key[0]] += 1
                return fact
        if key in self.active:
            raise ValueError("recursive queries need the production SCC solver")
        self.active.add(key)
        try:
            cx = QueryContext(program, self)
            answer = self.compute(cx, key)
            fact = Fact(answer, tuple(cx.dependencies.items()))
            self.entries[key].insert(0, fact)
            # Retain a bounded number of revisions of a queried fact.
            del self.entries[key][4:]
            while sum(map(len, self.entries.values())) > self.capacity:
                del self.entries[next(iter(self.entries))]
            self.computed[key[0]] += 1
            return fact
        finally:
            self.active.remove(key)

    def compute(self, cx, key):
        kind, subject = key
        if kind == "uses":
            cx.read(("uses", "cell", subject))
            return cx.program.index.get(("cell", subject), ())
        if kind == "effects":
            unit = cx.unit(subject, "body")
            cx.unit(subject, "captures")
            cx.unit(subject, "signature")
            result = Effects()
            for node in walk(unit.body):
                if node.kind == "call":
                    effect = cx.query(("effects", node.args[0]))
                elif node.kind == "invoke":
                    effect = UNKNOWN_EFFECTS
                elif node.kind in ("store", "set"):
                    effect = Effects(writes=True)
                elif node.kind == "throw":
                    effect = Effects(throws=True)
                elif node.kind == "forever":
                    effect = Effects(diverges=True)
                elif node.kind in ("allocate", "closure", "array"):
                    effect = Effects(identity=True)
                elif node.kind == "get" or (node.kind == "read" and node.args[0] in unit.captures):
                    effect = Effects(reads=True)
                elif node.kind == "repeat":
                    # Semantic repeat has a checked, finite nonnegative count.
                    effect = Effects() if exact(node) is not None else Effects(throws=True)
                else:
                    effect = Effects()
                result = result.union(effect)
            return result
        if kind == "exact":
            owner, expression = subject
            unit = cx.unit(owner, "body")
            if expression not in tuple(walk(unit.body)):
                raise ValueError("exact query does not belong to this unit")
            data = {n.args[0]: cx.query(("data", n.args[0])) for n in walk(expression) if n.kind == "global"}
            return exact(expression, data)
        if kind == "data":
            unit = cx.unit(subject, "body")
            cx.unit(subject, "signature")
            if unit.kind != "value":
                raise ValueError("expected immutable data")
            return exact(unit.body[0].args[0])
        raise ValueError(key)


def exact(node, data=None):
    if node.kind == "lit":
        return node.args[0]
    if node.kind == "global":
        return (data or {}).get(node.args[0])
    if node.kind == "cat":
        left, right = (exact(n, data) for n in node.args)
        if isinstance(left, str) and isinstance(right, str) and len(left) + len(right) <= 16384:
            return left + right
    if node.kind == "repeat":
        value, n = (exact(n, data) for n in node.args)
        if isinstance(value, str) and type(n) is int and 0 <= n <= 16384 and len(value) * n <= 16384:
            return value * n
    return None


class Rejected(Exception):
    pass


@dataclass(frozen=True)
class Proposal:
    base: Program
    rule: str
    proofs: tuple[Fact, ...]
    replacements: tuple[Unit, ...]


@dataclass(frozen=True)
class Commit:
    program: Program
    changed_facets: tuple
    copied_units: int


def commit(program, proposal, unit_budget=64):
    if not all(proof.valid(program) for proof in proposal.proofs):
        raise Rejected("stale proof dependencies")
    if len(proposal.replacements) > unit_budget:
        raise Rejected("transaction work reservation exhausted")
    if any(program.units.get(unit.key) is not proposal.base.units.get(unit.key)
           for unit in proposal.replacements):
        raise Rejected("conflicting write revision")
    units = dict(program.units)
    for unit in proposal.replacements:
        units[unit.key] = unit
    candidate = Program(units, program)
    verify(candidate)  # Private verification: failure cannot modify the base.
    changed = tuple(key for key, version in candidate.stamps.items()
                    if program.stamps.get(key, 0) != version)
    return Commit(candidate, changed, len(proposal.replacements))


def verify(program):
    exports = [u.export for u in program.units.values() if u.export]
    if len(exports) != len(set(exports)):
        raise Rejected("duplicate public spelling")
    allocations = {}
    for unit in program.units.values():
        for node in walk(unit.body):
            if node.kind == "allocate":
                allocations[node.args[0]] = {key for key, _ in node.args[1]}

    def expression(node, visible):
        k, a = node.kind, node.args
        if k == "lit":
            if type(a[0]) not in (str, int, bool):
                raise Rejected("unsupported literal")
        elif k == "read":
            if a[0] not in visible:
                raise Rejected("unbound cell " + a[0])
        elif k == "get":
            if a[0] not in visible or a[1] not in allocations.get(a[0], set()):
                raise Rejected("incomplete allocation/consumer layout")
        elif k == "closure":
            target = program.units[a[0]]
            if not set(target.captures) <= visible:
                raise Rejected("capture representation not provided by creator")
        elif k == "call":
            target = program.units[a[0]]
            if target.captures or len(a[1]) != len(target.params):
                raise Rejected("incompatible callable interface")
            for arg in a[1]:
                expression(arg, visible)
        elif k == "global":
            if program.units[a[0]].kind != "value":
                raise Rejected("incompatible data reference")
        elif k == "invoke":
            expression(a[0], visible)
            for arg in a[1]:
                expression(arg, visible)
        elif k == "array":
            for arg in a[0]:
                expression(arg, visible)
        elif k in ("add", "cat", "repeat"):
            for arg in a:
                expression(arg, visible)
        else:
            raise Rejected("unsupported expression " + k)

    def block(body, inherited):
        visible = set(inherited)
        for node in body:
            k, a = node.kind, node.args
            if k in ("let", "allocate"):
                if a[0] in visible:
                    raise Rejected("duplicate cell")
                if k == "let":
                    expression(a[1], visible)
                else:
                    for _, value in a[1]:
                        expression(value, visible)
                visible.add(a[0])
            elif k == "store":
                expression(op("get", a[0], a[1]), visible)
                expression(a[2], visible)
            elif k == "set":
                expression(read(a[0]), visible)
                expression(a[1], visible)
            elif k in ("return", "throw", "expr"):
                expression(a[0], visible)
            elif k == "if":
                expression(a[0], visible)
                block(a[1], visible)
                block(a[2], visible)
            elif k == "try":
                block(a[0], visible)
                block(a[1], visible)
            elif k != "forever":
                raise Rejected("unsupported statement " + k)

    for unit in program.units.values():
        if unit.kind == "value" and (unit.params or unit.captures or len(unit.body) != 1
                                     or exact(unit.body[0].args[0]) is None):
            raise Rejected("data initialization requires bounded exact pure data")
        block(unit.body, set(unit.params) | set(unit.captures))


def scalarize(program, queries, binding):
    proof = queries.query(program, ("uses", binding))
    if any(use.kind not in ("field", "capture") for use in proof.answer):
        raise Rejected("whole object is observed")
    found = [(u, n) for u in program.units.values() for n in walk(u.body)
             if n.kind == "allocate" and n.args[0] == binding]
    if len(found) != 1:
        raise Rejected("no unique static allocation definition")
    producer, allocation = found[0]
    fields = tuple(key for key, _ in allocation.args[1])
    stamps = Fact(None, ((("unit", producer.key, "body"),
                         program.stamps[("unit", producer.key, "body")]),))
    def cell(field):
        return binding + "$" + field
    def expr(node):
        if node.kind == "get" and node.args[0] == binding:
            return read(cell(node.args[1]))
        return None
    def body(nodes):
        output = []
        for node in nodes:
            k, a = node.kind, node.args
            if k == "allocate" and a[0] == binding:
                output.extend(op("let", cell(f), rewrite(v, expr)) for f, v in a[1])
            elif k == "store" and a[0] == binding:
                output.append(op("set", cell(a[1]), rewrite(a[2], expr)))
            elif k == "try":
                output.append(op("try", body(a[0]), body(a[1])))
            elif k == "if":
                output.append(op("if", rewrite(a[0], expr), body(a[1]), body(a[2])))
            else:
                output.append(rewrite(node, expr))
        return tuple(output)
    replacements = []
    for unit in program.units.values():
        captures = tuple(c for old in unit.captures
                         for c in (tuple(cell(f) for f in fields) if old == binding else (old,)))
        new = replace(unit, body=body(unit.body), captures=captures)
        if new != unit:
            replacements.append(new)
    return Proposal(program, "scalarize", (proof, stamps), tuple(replacements))


def inline(program, queries, helper):
    unit = program.units[helper]
    proof = queries.query(program, ("effects", helper))
    if unit.captures or unit.export or len(unit.body) != 1 or unit.body[0].kind != "return":
        raise Rejected("model inlining requires a private expression helper")
    serial = count()
    def body(nodes, owner):
        output = []
        for node in nodes:
            k, a = node.kind, node.args
            if k == "let" and a[1].kind == "call" and a[1].args[0] == helper:
                args = a[1].args[1]
                mapping = {p: owner + "$arg$" + str(next(serial)) for p in unit.params}
                # Evaluate each actual once, in order, before the substituted body.
                output.extend(op("let", mapping[p], actual) for p, actual in zip(unit.params, args))
                result = rewrite(unit.body[0].args[0], lambda n:
                                 read(mapping[n.args[0]]) if n.kind == "read" and n.args[0] in mapping else None)
                output.append(op("let", a[0], result))
            elif k == "try":
                output.append(op("try", body(a[0], owner), body(a[1], owner)))
            elif k == "if":
                output.append(op("if", a[0], body(a[1], owner), body(a[2], owner)))
            else:
                output.append(node)
        return tuple(output)
    replacements = tuple(replace(u, body=body(u.body, u.key)) for u in program.units.values())
    return Proposal(program, "inline", (proof,), tuple(u for u in replacements if u != program.units[u.key]))


def materialize(program, queries):
    proofs, replacements = [], []
    for unit in program.units.values():
        def expr(node):
            if node.kind not in ("cat", "repeat"):
                return None
            fact = queries.query(program, ("exact", (unit.key, node)))
            if isinstance(fact.answer, str):
                proofs.append(fact)
                return lit(fact.answer)
            return None
        changed = replace(unit, body=rewrite(unit.body, expr))
        if changed != unit:
            replacements.append(changed)
    return Proposal(program, "materialize", tuple(proofs), tuple(replacements))


def share_data(program, queries):
    counts = Counter(node.args[0] for u in program.units.values() if u.kind == "function"
                     for node in walk(u.body) if node.kind == "lit" and isinstance(node.args[0], str)
                     and len(node.args[0]) >= 8)
    eligible = sorted(value for value, count_ in counts.items() if count_ > 1)
    if not eligible:
        return Proposal(program, "share-data", (), ())
    value = eligible[0]
    key = "data_" + sha256(value.encode()).hexdigest()[:12]
    replacements = []
    proofs = []
    for unit in program.units.values():
        if unit.kind != "function":
            continue
        changed = replace(unit, body=rewrite(unit.body, lambda n:
                          op("global", key) if n.kind == "lit" and n.args[0] == value else None))
        if changed != unit:
            proofs.append(Fact(None, ((("unit", unit.key, "body"),
                                       program.stamps[("unit", unit.key, "body")]),)))
            replacements.append(changed)
    replacements.append(Unit(key, (), (), (op("return", lit(value)),), kind="value"))
    return Proposal(program, "share-data", tuple(proofs), tuple(replacements))


class Returned(Exception):
    def __init__(self, value):
        self.value = value


class Thrown(Exception):
    def __init__(self, value):
        self.value = value


def i32(value):
    return (value + 2**31) % 2**32 - 2**31


class Interpreter:
    def __init__(self, program):
        self.program = program
        self.data = {key: exact(unit.body[0].args[0]) for key, unit in program.units.items() if unit.kind == "value"}

    def function(self, key, environment=None):
        unit = self.program.units[key]
        captured = {name: environment[name] for name in unit.captures}
        def call(*args):
            env = dict(captured)
            env.update((p, [value]) for p, value in zip(unit.params, args))
            try:
                self.block(unit.body, env)
            except Returned as result:
                return result.value
        return call

    def expr(self, node, env):
        k, a = node.kind, node.args
        if k == "lit": return a[0]
        if k == "read": return env[a[0]][0]
        if k == "get": return env[a[0]][0][a[1]]
        if k == "global": return self.data[a[0]]
        if k == "closure": return self.function(a[0], env)
        if k == "call": return self.function(a[0])(*(self.expr(x, env) for x in a[1]))
        if k == "invoke":
            target = self.expr(a[0], env)
            return target(*(self.expr(x, env) for x in a[1]))
        if k == "array": return [self.expr(x, env) for x in a[0]]
        if k == "add": return i32(self.expr(a[0], env) + self.expr(a[1], env))
        if k == "cat": return self.expr(a[0], env) + self.expr(a[1], env)
        if k == "repeat":
            value, n = (self.expr(x, env) for x in a)
            if n < 0 or n > 16384: raise Thrown("repeat-range")
            return value * n
        raise ValueError(k)

    def block(self, nodes, env):
        env = dict(env)  # Block-local declarations, shared mutable cells.
        for node in nodes:
            k, a = node.kind, node.args
            if k == "let": env[a[0]] = [self.expr(a[1], env)]
            elif k == "allocate": env[a[0]] = [{f: self.expr(v, env) for f, v in a[1]}]
            elif k == "store":
                target = env[a[0]][0]
                target[a[1]] = self.expr(a[2], env)
            elif k == "set": env[a[0]][0] = self.expr(a[1], env)
            elif k == "expr": self.expr(a[0], env)
            elif k == "return": raise Returned(self.expr(a[0], env))
            elif k == "throw": raise Thrown(self.expr(a[0], env))
            elif k == "if": self.block(a[1] if self.expr(a[0], env) else a[2], env)
            elif k == "try":
                try: self.block(a[0], env)
                finally: self.block(a[1], env)
            elif k == "forever": raise ValueError("divergence fixture is never executed")
            else: raise ValueError(k)


class Printer:
    def __init__(self, program, short):
        self.program = program
        names = set(program.units)
        for unit in program.units.values():
            names.update(unit.params)
            names.update(unit.captures)
            names.update(n.args[0] for n in walk(unit.body) if n.kind in ("let", "allocate"))
        # All internal names are generated from stable identities. Public aliases
        # below preserve exact spellings. Unique names also protect captures.
        self.names = {key: (self.short_name(i) if short else "internal_" + key)
                      for i, key in enumerate(sorted(names))}

    @staticmethod
    def short_name(i):
        alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        return "_" + (alphabet[i] if i < 52 else str(i))

    def expr(self, node):
        k, a, n = node.kind, node.args, self.names
        if k == "lit": return json.dumps(a[0], ensure_ascii=True, separators=(",", ":"))
        if k == "read": return n[a[0]]
        if k == "get": return n[a[0]] + "[" + json.dumps(a[1]) + "]"
        if k == "global": return n[a[0]]
        if k == "closure": return self.function(self.program.units[a[0]], False)
        if k == "call": return n[a[0]] + "(" + ",".join(map(self.expr, a[1])) + ")"
        if k == "invoke": return "(" + self.expr(a[0]) + ")(" + ",".join(map(self.expr, a[1])) + ")"
        if k == "array": return "[" + ",".join(map(self.expr, a[0])) + "]"
        if k == "add": return "((" + self.expr(a[0]) + "+" + self.expr(a[1]) + ")|0)"
        if k == "cat": return "(" + self.expr(a[0]) + "+" + self.expr(a[1]) + ")"
        if k == "repeat":
            # No assumption about String.prototype.repeat or other host globals.
            # Parameters isolate scratch cells; argument evaluation remains ordered.
            return ('((_s,_n)=>{if(_n<0||_n>16384)throw "repeat-range";let _r="";'
                    'for(let _i=0;_i<_n;_i++)_r+=_s;return _r})('
                    + self.expr(a[0]) + "," + self.expr(a[1]) + ")")
        raise ValueError(k)

    def block(self, nodes):
        output, n = [], self.names
        for node in nodes:
            k, a = node.kind, node.args
            if k == "let": output.append("let " + n[a[0]] + "=" + self.expr(a[1]) + ";")
            elif k == "allocate":
                output.append("let " + n[a[0]] + "={" + ",".join("[" + json.dumps(f) + "]:" + self.expr(v) for f, v in a[1]) + "};")
            elif k == "store": output.append(n[a[0]] + "[" + json.dumps(a[1]) + "]=" + self.expr(a[2]) + ";")
            elif k == "set": output.append(n[a[0]] + "=" + self.expr(a[1]) + ";")
            elif k in ("return", "throw"): output.append(k + " " + self.expr(a[0]) + ";")
            elif k == "expr": output.append(self.expr(a[0]) + ";")
            elif k == "if": output.append("if(" + self.expr(a[0]) + "){" + self.block(a[1]) + "}else{" + self.block(a[2]) + "}")
            elif k == "try": output.append("try{" + self.block(a[0]) + "}finally{" + self.block(a[1]) + "}")
            elif k == "forever": output.append("for(;;){}")
            else: raise ValueError(k)
        return "".join(output)

    def function(self, unit, named):
        return ("function" + (" " + self.names[unit.key] if named else "") + "("
                + ",".join(self.names[x] for x in unit.params) + "){" + self.block(unit.body) + "}")

    def render(self):
        live, pending = set(), [u.key for u in self.program.units.values() if u.export]
        declarations = set(pending)
        while pending:
            key = pending.pop()
            if key in live: continue
            live.add(key)
            for node in walk(self.program.units[key].body):
                if node.kind in ("call", "closure", "global"):
                    pending.append(node.args[0])
                    if node.kind != "closure": declarations.add(node.args[0])
        output = []
        for key in sorted(declarations):
            unit = self.program.units[key]
            if unit.kind == "value":
                output.append("const " + self.names[key] + "=" + self.expr(unit.body[0].args[0]) + ";")
            else:
                output.append(self.function(unit, True))
        output.append("export{" + ",".join(self.names[u.key] + " as " + u.export
                      for u in self.program.units.values() if u.export) + "};")
        return "".join(output)
