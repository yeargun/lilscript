#!/usr/bin/env python3
"""Content-locked, objective-separated calibration and protected evaluation.

Training uses on/audit/off pruning. A completed training receipt is mandatory
for evaluation and pins the compiler, codecs, runner, Node and policy. No
parameter is fitted from evaluation output. Reports retain failures; a failed
oracle is never a size observation eligible for tuning.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import statistics
import subprocess
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cost)
OBJECTIVES = {"raw": (0, "raw"), "gzip": (1, "gzip9"), "brotli": (2, "brotli11")}
MODES = ["on", "audit", "off"]
POLICY = "objective.codecs='{objective}'\n[policy]\nversion=3\n[effort]\nlevel=13\n[policy.search]\nproxy_pruning='{mode}'\n"
ORACLE = """
const fs = await import('node:fs');
const row = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const library = await import(process.argv[1]);
const actual = row.inputs.map(value => library.probe(value));
if (JSON.stringify(actual) !== JSON.stringify(row.expected))
  throw Error(JSON.stringify({actual, expected: row.expected}));
console.log(JSON.stringify(actual));
"""


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def identity(path):
    path = Path(path).resolve()
    return {"path": str(path), "sha256": digest(path)}


def stable_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def environment():
    # No hidden compatibility adapter, loader or thread count may distinguish
    # lanes. Keep required platform variables, explicitly remove policy inputs.
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("LILSCRIPT_") and key not in ["NODE_OPTIONS", "RAYON_NUM_THREADS"]}
    env.update(TZ="UTC", LC_ALL="C", RAYON_NUM_THREADS="1")
    return env


def invoke(command, *, env, timeout):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    load = os.getloadavg()
    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=timeout)
    elapsed = time.monotonic()-start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    timing = dict(cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,
                  wall_seconds=elapsed, peak_rss_kib=after.ru_maxrss,
                  peak_rss_scope="Cumulative child high-water mark, not isolated invocation RSS",
                  load_before=load, load_after=os.getloadavg())
    return result, timing


def terminal_trials(report):
    terminal = report["search"]["terminal"]["objectives"][0]
    return [(kind, trial) for kind in ["trials", "choice_trials", "joint_trials", "starts"]
            for trial in terminal[kind]]


def summarize(rows, cross_validate=True):
    result = {}
    for objective in OBJECTIVES:
        own = [row for row in rows if row["objective"] == objective]
        totals = {mode: {"bytes": 0, "judged": 0, "pruned": 0, "codec_probes": 0,
                         "baseline_work": 0, "optional_work": 0, "codec_work": 0,
                         "peak_retained_bytes": 0, "stopped": 0} for mode in MODES}
        missed, changed = [], []
        for row in own:
            for mode, value in row["modes"].items():
                total = totals[mode]
                total["bytes"] += value["bytes"]
                for key in ["judged", "pruned", "codec_probes", "baseline_work", "optional_work", "codec_work", "stopped"]:
                    total[key] += value[key]
                total["peak_retained_bytes"] = max(total["peak_retained_bytes"], value["peak_retained_bytes"])
            missed += [{"id": row["id"], **trial} for trial in row["modes"]["audit"]["missed"]]
            if row["modes"]["off"]["bytes"] != row["modes"]["on"]["bytes"]:
                changed.append(dict(id=row["id"], delta=row["modes"]["off"]["bytes"]-row["modes"]["on"]["bytes"]))
        # Fit only an explicit experimental choice between the two measured
        # schedules. The safety margin estimator uses immediate misses only;
        # it is diagnostic and must never claim to replay changed search paths.
        folds = []
        for family in sorted({row["family"] for row in own}) if cross_validate else []:
            training = [row for row in own if row["family"] != family]
            validation = [row for row in own if row["family"] == family]
            sums = {mode: sum(row["modes"][mode]["bytes"] for row in training) for mode in ["on", "off"]}
            selected = min(sums, key=lambda mode: (sums[mode], mode != "on"))
            original_margin = own[0]["margin"]
            safety_margin = max([original_margin] + [trial["proxy"] for row in training
                                for trial in row["modes"]["audit"]["missed"]])
            folds.append(dict(held_out_family=family, training_ids=[row["id"] for row in training],
                              validation_ids=[row["id"] for row in validation],
                              selected_mode=selected, training_bytes=sums,
                              validation_delta=sum(row["modes"][selected]["bytes"]-row["modes"]["on"]["bytes"] for row in validation),
                              estimated_minimum_margin=safety_margin,
                              unseen_immediate_misses=sum(trial["proxy"] > safety_margin for row in validation
                                  for trial in row["modes"]["audit"]["missed"])))
        result[objective] = dict(totals=totals, immediate_proxy_misses=missed,
                                 final_no_prune_changes=changed, leave_one_family_out=folds)
    return result


def validate_manifest(manifest, root):
    if manifest.get("schema") != 1:
        raise ValueError("unsupported manifest")
    ids, families = set(), {"training": set(), "evaluation": set()}
    for row in manifest["workloads"]:
        if row["id"] in ids or row["split"] not in families or not row.get("exposure"):
            raise ValueError("duplicate identity, invalid split or missing exposure")
        ids.add(row["id"])
        families[row["split"]].add(row["family"])
        entry = (root / row["entry"]).resolve()
        if not entry.is_relative_to(root) or digest(entry) != row["source_sha256"]:
            raise ValueError("source escapes manifest or content changed")
        if entry.stat().st_size != row["source_bytes"]:
            raise ValueError("incorrect source size")
        if row["split"] == "training" and not 10_000 <= row["source_bytes"] <= 300_000:
            raise ValueError("training source outside the preregistered medium size range")
        if not row["inputs"] or len(row["inputs"]) != len(row["expected"]):
            raise ValueError("missing oracle")
    if len(families["training"]) < 3 or len(families["evaluation"]) < 3 or families["training"] & families["evaluation"]:
        raise ValueError("need three disjoint families in each split")


def validate_training(training, tool_identity, manifest_hash):
    if training.get("complete") is not True or training.get("split") != "training":
        raise ValueError("evaluation requires a completed training receipt")
    if training["identity"] != tool_identity or training["manifest_sha256"] != manifest_hash:
        raise ValueError("evaluation compiler, policy, tools or frozen corpus differ from training")
    if training.get("production_policy_changed") is not False:
        raise ValueError("evaluation requires the declared frozen production policy")
    if not training.get("summary") or any(row["split"] != "training" for row in training["rows"]):
        raise ValueError("training receipt contains protected evaluation data")


def reuse_rows(path, tools, manifest, work, env, timeout):
    """Reuse complete training cells only after replaying their retained evidence.

The corpus may have a repaired *other* workload. A cell's compiler, policy,
source, oracle, artifact, explanation and metrics must still match exactly.
The old runner identity is retained, rather than relabeled as a fresh compile.
Only the single-module/no-host workloads of this generator are supported.
"""
    previous = json.loads(path.read_text())
    if previous.get("schema") != 1 or previous.get("split") != "training":
        raise ValueError("reuse accepts training measurements only")
    for key in tools.keys() - {"runner"}:
        if previous["identity"].get(key) != tools[key]:
            raise ValueError(f"reused {key} differs from this experiment")
    for key in ["runner", "decoder"]:
        if identity(previous["identity"][key]["path"]) != previous["identity"][key]:
            raise ValueError("original measurement source is unavailable or changed")
    old = {row["id"]: row for row in previous["manifest"]["workloads"]}
    current = {row["id"]: row for row in manifest["workloads"] if row["split"] == "training"}
    kept = {}
    for row in previous["rows"]:
        item = current.get(row["id"])
        if item is None or set(row["modes"]) != set(MODES):
            continue
        if any(item[key] != old[item["id"]][key] for key in
               ["id", "split", "family", "entry", "source_sha256", "source_bytes", "inputs", "expected"]):
            continue
        if any(row[key] != item[key] for key in ["id", "split", "family", "source_bytes"]):
            raise ValueError("reused cell belongs to a different workload")
        objective = row["objective"]
        index, metric = OBJECTIVES[objective]
        oracle_path = work / f'reuse-{item["id"]}-{objective}.oracle.json'
        oracle_path.write_text(json.dumps(item))
        comparable = None
        for mode, cell in row["modes"].items():
            for key in ["artifact", "config", "explanation"]:
                if identity(cell[key]["path"]) != cell[key]:
                    raise ValueError("reused artifact/config/explanation changed")
            if Path(cell["config"]["path"]).read_text() != POLICY.format(objective=objective, mode=mode):
                raise ValueError("reused configuration does not match policy")
            docs = [doc for doc in cost.json_documents(Path(cell["explanation"]["path"]).read_text()) if "inputs" in doc]
            if len(docs) != 1:
                raise ValueError("missing reused explanation")
            explanation = docs[0]
            modules = explanation["inputs"]["modules"]
            if (len(modules) != 1 or modules[0]["sha256"] != item["source_sha256"]
                    or digest(modules[0]["path"]) != item["source_sha256"]
                    or modules[0]["dependencies"] or modules[0]["dynamic_dependencies"]
                    or explanation["inputs"]["host_modules"]):
                raise ValueError("reused source graph differs or is not an isolated module")
            policy = copy.deepcopy(explanation["javascript_policy"])
            if policy["effort"] != 13 or policy["objective"]["search"].pop("proxy_pruning") != mode:
                raise ValueError("reused resolved policy mismatch")
            facts = (policy, explanation["request"])
            if comparable is not None and comparable != facts:
                raise ValueError("reused lanes have different policy/resources")
            comparable = facts
            cost.effective_resources(explanation, policy)
            artifact = explanation["artifacts"][explanation["winners"][index]]
            if artifact["sha256"] != cell["artifact"]["sha256"] or artifact[metric] != cell["bytes"]:
                raise ValueError("reused selected artifact mismatch")
            terminal = explanation["search"]["terminal"]["objectives"][0]
            for key in ["judged", "pruned", "codec_probes"]:
                if cell[key] != terminal[key]:
                    raise ValueError("reused search count mismatch")
            for key in ["baseline_work", "optional_work", "codec_work", "peak_retained_bytes"]:
                if cell[key] != explanation["resources"][key]:
                    raise ValueError("reused resource count mismatch")
            trials = terminal_trials(explanation)
            missed = [dict(kind=kind, **trial) for kind, trial in trials
                      if trial.get("audit") is not None and trial["audit"] < 0]
            if missed != cell["missed"] or row["margin"] != policy["objective"]["walk"]["margin"]:
                raise ValueError("reused pruning evidence mismatch")
            if (cell["raw_bytes"] != artifact["raw"]
                    or cell["audited"] != sum(trial.get("audit") is not None for _, trial in trials)
                    or cell["stopped"] != int(explanation["search"]["stop"] is not None or
                                              any(trial["outcome"] == "stopped" for _, trial in trials))):
                raise ValueError("reused artifact/audit/stop evidence mismatch")
            output = Path(cell["artifact"]["path"])
            oracle, _ = invoke([tools["node"]["path"], "--input-type=module", "-e", ORACLE,
                                output.as_uri(), str(oracle_path)], env=env, timeout=timeout)
            if oracle.returncode or json.loads(oracle.stdout) != cell["observation"]:
                raise ValueError("reused artifact failed the current independent oracle")
            measured, _ = invoke([tools["codec"]["path"], "--json", str(output)], env=env, timeout=timeout)
            if measured.returncode:
                raise ValueError("reused artifact could not be remeasured")
            actual = json.loads(measured.stdout)["artifacts"][0]
            if {key: actual[key] for key in cell["all_sizes"]} != cell["all_sizes"] or actual[metric] != cell["bytes"]:
                raise ValueError("reused exact measurements disagree")
        if row["modes"]["on"]["artifact"]["sha256"] != row["modes"]["audit"]["artifact"]["sha256"]:
            raise ValueError("reused audit changed ordinary output")
        if len(row["modes"]["on"]["warm_samples"]) < 3:
            raise ValueError("reused CPU baseline lacks repeated samples")
        row["reused_from"] = identity(path)
        row["original_runner"] = previous["identity"]["runner"]
        key = (row["id"], objective)
        if key in kept:
            raise ValueError("duplicate reused cell")
        kept[key] = row
    return kept


def run(args):
    if args.json.exists():
        raise ValueError("report exists; retain it and choose a new destination")
    root = args.manifest.resolve().parent
    manifest = json.loads(args.manifest.read_text())
    validate_manifest(manifest, root)
    env = environment()
    node = shutil.which("node", path=env.get("PATH"))
    if not node:
        raise ValueError("Node is required")
    tool_identity = dict(compiler=identity(args.compiler), codec=identity(args.codec),
                         runner=identity(__file__), decoder=identity(HERE / "compile-cost.py"),
                         node=identity(node), policy_template=POLICY, policy_sha256=stable_hash(POLICY),
                         environment={key: env[key] for key in ["LC_ALL", "TZ", "RAYON_NUM_THREADS"]})
    manifest_hash = digest(args.manifest)
    if args.split == "evaluation":
        if args.training is None:
            raise ValueError("--training receipt required before protected evaluation")
        validate_training(json.loads(args.training.read_text()), tool_identity, manifest_hash)
    elif args.training is not None:
        raise ValueError("training never consumes an evaluation or prior tuning receipt")
    args.work.mkdir(parents=True, exist_ok=False)
    report = dict(schema=1, split=args.split, identity=tool_identity,
                  manifest_sha256=manifest_hash, manifest=manifest,
                  training_receipt=identity(args.training) if args.training else None,
                  production_policy_changed=False, complete=False, rows=[], failures=[],
                  timing="On-lane explanation is discarded as warmup; timed samples omit --explain. No speed comparison is claimed.",
                  historical_a1_cpu_pair="Unavailable; current samples establish a new baseline, not a reconstruction.")
    try:
        reused = {}
        if args.reuse:
            if args.split != "training":
                raise ValueError("protected evaluation cannot reuse tuning measurements")
            report["reuse_receipt"] = identity(args.reuse)
            reused = reuse_rows(args.reuse, tool_identity, manifest, args.work, env, args.timeout)
        for workload in manifest["workloads"]:
            if workload["split"] != args.split:
                continue
            for objective, (index, metric) in OBJECTIVES.items():
                if (workload["id"], objective) in reused:
                    report["rows"].append(reused[(workload["id"], objective)])
                    print(f'{args.split}: {len(report["rows"])} objective cells qualified (verified reuse)', flush=True)
                    continue
                row = dict(id=workload["id"], family=workload["family"], split=args.split,
                           source_bytes=workload["source_bytes"], objective=objective, modes={})
                report["rows"].append(row)
                comparable = None
                for mode in MODES:
                    stem = f'{workload["id"]}-{objective}-{mode}'
                    output, config = args.work / f"{stem}.mjs", args.work / f"{stem}.toml"
                    config.write_text(POLICY.format(objective=objective, mode=mode))
                    command = [str(args.compiler), str(root / workload["entry"]), "--target", "js-module",
                               "--config", str(config), "-o", str(output)]
                    completed, timing = invoke(command+["--explain", "json"], env=env, timeout=args.timeout)
                    explanation_path = args.work / f"{stem}.stderr"
                    explanation_path.write_text(completed.stderr)
                    if completed.returncode:
                        raise RuntimeError(f"{stem}: compilation failed; see {explanation_path}")
                    explanations = [doc for doc in cost.json_documents(completed.stderr) if "inputs" in doc]
                    if len(explanations) != 1:
                        raise RuntimeError(f"{stem}: missing or ambiguous explanation")
                    explanation = explanations[0]
                    policy = copy.deepcopy(explanation["javascript_policy"])
                    if policy["effort"] != 13 or policy["objective"]["search"].pop("proxy_pruning") != mode:
                        raise RuntimeError(f"{stem}: ignored experiment policy")
                    facts = dict(policy=policy, inputs=explanation["inputs"], request=explanation["request"])
                    # Config hashes differ deliberately; compiler input modules
                    # and resolved policy/resources must remain identical.
                    facts["inputs"] = facts["inputs"]["modules"]
                    if comparable is not None and facts != comparable:
                        raise RuntimeError(f"{stem}: unmatched policy, resources or source modules")
                    comparable = facts
                    cost.effective_resources(explanation, policy)
                    for source in explanation["inputs"]["modules"]:
                        if digest(source["path"]) != source["sha256"]:
                            raise RuntimeError("source changed during compilation")
                    artifact = explanation["artifacts"][explanation["winners"][index]]
                    if digest(output) != artifact["sha256"]:
                        raise RuntimeError("artifact/explanation mismatch")
                    observed = args.work / f"{stem}.oracle.json"
                    observed.write_text(json.dumps(workload))
                    oracle, _ = invoke([node, "--input-type=module", "-e", ORACLE, output.as_uri(), str(observed)], env=env, timeout=args.timeout)
                    (args.work / f"{stem}.oracle.stderr").write_text(oracle.stderr)
                    if oracle.returncode:
                        raise RuntimeError(f"{stem}: independent oracle failed; see {stem}.oracle.stderr")
                    observation = json.loads(oracle.stdout)
                    measured, _ = invoke([str(args.codec), "--json", str(output)], env=env, timeout=args.timeout)
                    if measured.returncode:
                        raise RuntimeError("canonical codec failed")
                    encoded = json.loads(measured.stdout)
                    actual = encoded["artifacts"][0]
                    if actual[metric] != artifact[metric]:
                        raise RuntimeError("independent canonical score disagrees with explanation")
                    terminal = explanation["search"]["terminal"]["objectives"][0]
                    trials = terminal_trials(explanation)
                    record = dict(artifact=identity(output), config=identity(config),
                                  explanation=identity(explanation_path), observation=observation,
                                  bytes=artifact[metric], raw_bytes=output.stat().st_size,
                                  all_sizes={key: actual[key] for key in ["raw", "gzip9", "brotli11"]},
                                  diagnostic_timing=timing,
                                  missed=[dict(kind=kind, **trial) for kind, trial in trials
                                          if trial.get("audit") is not None and trial["audit"] < 0],
                                  audited=sum(trial.get("audit") is not None for _, trial in trials),
                                  stopped=int(explanation["search"]["stop"] is not None or
                                              any(trial["outcome"] == "stopped" for _, trial in trials)),
                                  **{key: terminal[key] for key in ["judged", "pruned", "codec_probes"]},
                                  **{key: explanation["resources"][key] for key in
                                     ["baseline_work", "optional_work", "codec_work", "peak_retained_bytes"]})
                    row["modes"][mode] = record
                    row["margin"] = policy["objective"]["walk"]["margin"]
                    if mode == "on":
                        samples = []
                        for _ in range(args.rounds):
                            sample_result, sample = invoke(command, env=env, timeout=args.timeout)
                            if sample_result.returncode or digest(output) != record["artifact"]["sha256"]:
                                raise RuntimeError("timed compilation failed or changed output")
                            samples.append(sample)
                        record["warm_samples"] = samples
                        record["median_warm_cpu_seconds"] = statistics.median(sample["cpu_seconds"] for sample in samples)
                    if mode == "audit" and record["artifact"]["sha256"] != row["modes"]["on"]["artifact"]["sha256"]:
                        raise RuntimeError("audit changed ordinary output")
                print(f'{args.split}: {len(report["rows"])} objective cells qualified', flush=True)
        if digest(args.manifest) != manifest_hash:
            raise RuntimeError("manifest changed during measurement")
        validate_manifest(manifest, root)
        for key in ["compiler", "codec", "runner", "decoder", "node"]:
            if identity(tool_identity[key]["path"]) != tool_identity[key]:
                raise RuntimeError(f"{key} changed during measurement")
        if args.training and identity(args.training) != report["training_receipt"]:
            raise RuntimeError("training receipt changed during evaluation")
        if args.reuse and identity(args.reuse) != report["reuse_receipt"]:
            raise RuntimeError("reuse receipt changed during measurement")
        for row in report["rows"]:
            for cell in row["modes"].values():
                for key in ["artifact", "config", "explanation"]:
                    if identity(cell[key]["path"]) != cell[key]:
                        raise RuntimeError("retained measurement changed during the batch")
        report["summary"] = summarize(report["rows"], cross_validate=args.split == "training")
        # Cross-validation belongs only to training; evaluation has no fitting.
        if args.split == "evaluation":
            for value in report["summary"].values():
                value.pop("leave_one_family_out")
        report["complete"] = True
    except Exception as error:
        report["failures"].append(str(error))
        raise
    finally:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2)+"\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ["manifest", "compiler", "codec", "work", "json"]:
        parser.add_argument("--"+flag, type=Path, required=True)
    parser.add_argument("--split", choices=["training", "evaluation"], default="training")
    parser.add_argument("--training", type=Path)
    parser.add_argument("--reuse", type=Path, help="reverify complete matching training cells from a retained earlier report")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--timeout", type=int, default=600)
    args = parser.parse_args()
    if args.rounds < 3 or args.timeout <= 0:
        parser.error("at least three warm baseline samples and a positive timeout are required")
    for name in ["manifest", "compiler", "codec", "work", "json", "training", "reuse"]:
        if getattr(args, name) is not None:
            setattr(args, name, getattr(args, name).resolve())
    run(args)


if __name__ == "__main__":
    main()
