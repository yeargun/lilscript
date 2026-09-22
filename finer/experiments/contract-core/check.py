"""Run the bounded integrated contract experiment and write its exact artifacts."""
from collections import deque
from dataclasses import replace
from hashlib import sha256
from pathlib import Path
import argparse
import json
import platform

from model import (Program, Unit, op, lit, read, verify, Queries, Proposal, commit,
                   scalarize, inline, materialize, share_data, Interpreter,
                   Printer, Thrown, Rejected)


def fixture():
    label = op("repeat", lit("closure-state:abc/"), lit(6))
    get = lambda field: op("get", "state", field)
    work = (
        op("let", "increment", op("call", "helper", (read("delta"),))),
        op("store", "state", "value", op("add", get("value"), read("increment"))),
        op("expr", op("invoke", read("notify"), (read("peek_cell"),))),
        op("if", read("fail"), (op("throw", lit("marker")),), ()),
        op("let", "result", op("call", "helper", (get("value"),))),
        op("let", "second_label", label),
        op("return", op("array", (read("result"), read("label"), read("second_label")))),
    )
    cleanup = (op("store", "state", "value", op("add", get("value"), get("stride"))),)
    units = [
        Unit("helper", ("number",), (), (op("return", op("add", read("number"), read("number"))),)),
        Unit("peek", (), ("state",), (op("return", get("value")),)),
        Unit("step", ("delta", "fail"), ("state", "notify", "peek_cell", "label"),
             (op("try", work, cleanup),)),
        Unit("make", ("seed", "notify"), (), (
            op("allocate", "state", (("value", read("seed")), ("stride", lit(2)))),
            op("let", "label", label),
            op("let", "peek_cell", op("closure", "peek")),
            op("return", op("closure", "step"))), "make"),
        Unit("public_state", ("public_seed",), (), (
            op("allocate", "public_object", (("value", read("public_seed")), ("stride", lit(2)))),
            op("return", read("public_object"))), "publicState"),
        Unit("unrelated", (), (), (op("return", lit(41)),), "unrelated"),
        Unit("throws_only", (), (), (op("throw", lit("checked-error")),)),
        Unit("diverges_only", (), (), (op("forever"),)),
    ]
    program = Program({u.key: u for u in units})
    verify(program)
    return program, label


def observe(program, seed, throw_notify):
    vm = Interpreter(program)
    notifications, peeks = [], []
    def notify(peek):
        notifications.append(peek())
        peeks.append(peek)
        if throw_notify and len(peeks) == 2:
            raise Thrown("host-error")
    step = vm.function("make")(seed, notify)
    outcomes = []
    for delta, should_throw in ((3, False), (1, True), (-5, False)):
        try:
            outcomes.append(["return", step(delta, should_throw)])
        except Thrown as error:
            outcomes.append(["throw", error.value])
    before_other = [p() for p in peeks]
    other_peeks = []
    other = vm.function("make")(17, lambda p: other_peeks.append(p))
    other_result = other(2, False)
    public = vm.function("public_state")(seed)
    public["value"] = 123
    return dict(outcomes=outcomes, notifications=notifications,
                final_peeks=[p() for p in peeks], same_peek=all(p is peeks[0] for p in peeks),
                independent=[p() for p in peeks] == before_other,
                other_result=other_result, other_final=other_peeks[0](),
                public=public, public_keys=list(public), unrelated=vm.function("unrelated")())


def must_reject(action, reason):
    try:
        action()
    except Rejected as error:
        assert reason in str(error), (reason, str(error))
        return str(error)
    raise AssertionError("invalid proposal accepted: " + reason)


def explore(base, queries, cap):
    states, pending, seen = [], deque([(base, ())]), {base.fingerprint()}
    attempts = 0
    while pending and attempts < cap:
        program, trace = pending.popleft()
        states.append((program, trace))
        for generate in (lambda p: scalarize(p, queries, "state"),
                         lambda p: inline(p, queries, "helper"),
                         lambda p: materialize(p, queries),
                         lambda p: share_data(p, queries)):
            if attempts == cap: break
            attempts += 1
            try:
                proposal = generate(program)
                if not proposal.replacements: continue
                child = commit(program, proposal).program
            except Rejected:
                continue
            key = child.fingerprint()
            if key in seen: continue
            seen.add(key)
            pending.append((child, trace + (proposal.rule,)))
    states.extend(pending)
    return states, attempts


def run(output):
    base, label = fixture()
    queries = Queries()
    before = base.fingerprint()
    value = queries.query(base, ("exact", ("make", label)))
    assert value.answer == "closure-state:abc/" * 6
    assert base.fingerprint() == before
    assert any(n.kind == "let" and n.args[1] == label for n in base.units["make"].body)
    old_effect = queries.query(base, ("effects", "helper"))
    old_caller_effect = queries.query(base, ("effects", "step"))
    local_unit = replace(base.units["unrelated"], body=(op("return", lit(42)),))
    local = commit(base, Proposal(base, "source-edit", (), (local_unit,)))
    assert local.program.units["make"] is base.units["make"]
    assert local.program.units["helper"] is base.units["helper"]
    assert queries.query(local.program, ("effects", "helper")) is old_effect
    assert queries.query(local.program, ("exact", ("make", label))) is value
    assert local.program.reindexed == 1

    scalar = scalarize(base, queries, "state")
    split = commit(base, scalar)
    assert split.program.units["helper"] is base.units["helper"]
    assert queries.query(split.program, ("effects", "helper")) is old_effect
    helper_changed = replace(base.units["helper"], body=(op("return", lit(7)),))
    helper_edit = commit(base, Proposal(base, "source-edit", (), (helper_changed,)))
    assert not old_effect.valid(helper_edit.program)
    assert not old_caller_effect.valid(helper_edit.program)
    assert queries.query(helper_edit.program, ("effects", "helper")) is not old_effect
    assert not queries.query(base, ("effects", "throws_only")).answer.droppable()
    assert not queries.query(base, ("effects", "diverges_only")).answer.speculatable()
    assert not queries.query(base, ("effects", "step")).answer.speculatable()

    rejections = {}
    rejections["public_escape"] = must_reject(lambda: scalarize(base, queries, "public_object"), "whole object")
    producer_only = tuple(u for u in scalar.replacements if u.key == "make")
    rejections["partial_layout"] = must_reject(
        lambda: commit(base, replace(scalar, replacements=producer_only)), "allocation/consumer layout")
    # The host already receives peek. A source edit making peek return the
    # object introduces a real whole-object escape without editing make.
    late = replace(base.units["peek"], body=(op("return", read("state")),))
    escaped = commit(base, Proposal(base, "source-edit", (), (late,)))
    assert escaped.program.units["make"] is base.units["make"]
    rejections["new_remote_use"] = must_reject(lambda: commit(escaped.program, scalar), "stale proof")
    rejections["transaction_budget"] = must_reject(lambda: commit(base, scalar, 0), "reservation")
    altered_step = replace(base.units["step"], body=base.units["step"].body + (op("expr", lit(0)),))
    altered = commit(base, Proposal(base, "source-edit", (), (altered_step,)))
    rejections["write_conflict"] = must_reject(lambda: commit(altered.program, scalar), "conflicting write")
    assert base.fingerprint() == before

    # A single expression helper argument can throw or invoke a callback.
    # Actuals are captured once before inlined use, including repeated params.
    caller = Unit("effect_caller", ("source",), (), (
        op("let", "answer", op("call", "helper", (op("invoke", read("source"), ()),))),
        op("return", read("answer"))), "effectCaller")
    with_caller = commit(base, Proposal(base, "add-case", (), (caller,))).program
    inlined = commit(with_caller, inline(with_caller, queries, "helper")).program
    for arm in (with_caller, inlined):
        count_ = [0]
        def source():
            count_[0] += 1
            return 9
        assert Interpreter(arm).function("effect_caller")(source) == 18 and count_[0] == 1
        def throws(): raise Thrown("argument-error")
        try: Interpreter(arm).function("effect_caller")(throws)
        except Thrown as error: assert error.value == "argument-error"
        else: raise AssertionError("lost argument throw")

    seeds = (-2147483648, -1, 0, 7, 2147483647)
    scenarios = [dict(seed=s, throw_notify=t, expected=observe(base, s, t)) for s in seeds for t in (False, True)]
    direct, direct_attempts = explore(base, queries, 0)
    assert direct == [(base, ())] and direct_attempts == 0
    fast, fast_attempts = explore(base, queries, 4)
    fast_keys = {p.fingerprint() for p, _ in fast}
    cap = 128
    states, attempts = explore(base, queries, cap)
    assert fast_keys <= {p.fingerprint() for p, _ in states}
    assert any({"scalarize", "inline", "materialize", "share-data"} <= set(trace) for _, trace in states)
    artifacts, hashes = [], set()
    observations = 0
    for program, trace in states:
        for case in scenarios:
            assert observe(program, case["seed"], case["throw_notify"]) == case["expected"], trace
            observations += 1
        for short in (True, False):
            js = Printer(program, short).render()
            key = sha256(js.encode()).hexdigest()
            if key in hashes: continue
            hashes.add(key)
            artifacts.append(dict(sha256=key, javascript=js, trace=trace,
                                  fast=program.fingerprint() in fast_keys,
                                  naming="short" if short else "source"))
    extra_artifacts = [dict(javascript=Printer(p, True).render()) for p in (with_caller, inlined)]
    result = dict(schema=1, role="executable contract model, not production performance evidence",
                  python=platform.python_version(), source_sha256={p.name: sha256(p.read_bytes()).hexdigest()
                         for p in Path(__file__).parent.iterdir() if p.suffix in (".py", ".mjs")},
                  units=len(base.units), local_edit_reindexed_units=local.program.reindexed,
                  scalarization_copied_units=split.copied_units,
                  scalarization_changed_facets=[list(k) for k in split.changed_facets],
                  query_computations=dict(queries.computed), query_hits=dict(queries.hits),
                  rejections=rejections, proposals=attempts, proposal_cap=cap,
                  fast_states=len(fast), fast_proposals=fast_attempts,
                  candidate_states=len(states), interpreter_observations=observations,
                  scenarios=scenarios, artifacts=artifacts, extra_artifacts=extra_artifacts)
    output.mkdir(parents=True, exist_ok=True)
    (output / "model.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("candidate_states", "proposals", "interpreter_observations", "rejections")}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    run(parser.parse_args().output)
