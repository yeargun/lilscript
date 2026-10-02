#!/usr/bin/env python3
"""Qualify the frozen C3 evaluation libraries without fitting another policy.

This is a follow-up on previously exposed evaluation programs, not a new blind
set. No pruning, effort or tactic parameter is selected from these results.
A candidate failure is retained and requires promotion/replacement before a
further generalization claim. Maintained external ports are qualified separately.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "benchmarks/calibration/corpus/manifest.json"
POLICY = """[policy]
version = 3
[policy.resources]
logical_work = 40000000000
retained_bytes = 256000000
[effort]
level = 13
[objective]
codecs = "{objective}"
"""
METRICS = {"raw": "raw", "gzip": "gzip9", "brotli": "brotli11"}
ORACLE = """import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const row = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const library = await import(pathToFileURL(process.argv[1]));
const actual = row.inputs.map(input => library.probe(input));
if (JSON.stringify(actual) !== JSON.stringify(row.expected))
  throw Error(JSON.stringify({actual, expected: row.expected}));
console.log(JSON.stringify(actual));
"""


def identity(path):
    path = Path(path).resolve()
    return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["previous", "candidate", "codec", "node", "work", "json"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if args.json.exists():
        raise ValueError("retain the existing report and choose a new destination")
    args.work = args.work.resolve()
    args.work.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location("cost", ROOT / "scripts/compile-cost.py")
    cost = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cost)
    tools = {key: identity(getattr(args, key)) for key in ["previous", "candidate", "codec", "node"]}
    tools["runner"] = identity(__file__)
    tools["decoder"] = identity(ROOT / "scripts/compile-cost.py")
    manifest = json.loads(MANIFEST.read_text())
    report = dict(schema=1, complete=False, tools=tools, manifest=identity(MANIFEST),
                  policy_template=POLICY, fitting=False,
                  exposure="C3's previously qualified evaluation libraries; no new blind claim.",
                  rows=[], failures=[])
    # Write the fixed experiment before looking at any new result.
    args.json.parent.mkdir(parents=True, exist_ok=True)
    save = lambda: args.json.write_text(json.dumps(report, indent=2) + "\n")
    save()
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("LILSCRIPT_") and key not in ["NODE_OPTIONS", "RAYON_NUM_THREADS"]}
    env.update(TZ="UTC", LC_ALL="C", RAYON_NUM_THREADS="1")

    def run(command, log):
        result = subprocess.run(list(map(str, command)), env=env, cwd=ROOT,
                                text=True, capture_output=True, timeout=1800)
        log.with_suffix(".stdout").write_text(result.stdout)
        log.with_suffix(".stderr").write_text(result.stderr)
        if result.returncode:
            raise RuntimeError(f"{log.name}: exit {result.returncode}; see retained stderr")
        return result

    try:
        for workload in manifest["workloads"]:
            if workload["split"] != "evaluation":
                continue
            source = MANIFEST.parent / workload["entry"]
            if identity(source)["sha256"] != workload["source_sha256"]:
                raise ValueError("evaluation source content lock changed")
            for objective, metric in METRICS.items():
                stem = f'{workload["id"]}-{objective}'
                config = args.work / (stem + ".toml")
                config.write_text(POLICY.format(objective=objective))
                oracle = args.work / (stem + ".oracle.json")
                oracle.write_text(json.dumps(workload) + "\n")
                row = dict(id=workload["id"], objective=objective, source=identity(source),
                           config=identity(config), expected=workload["expected"], compilers={})
                report["rows"].append(row)
                explanations = []
                for label in ["previous", "candidate"]:
                    name = stem + "-" + label
                    output = args.work / (name + ".mjs")
                    compiled = run([tools[label]["path"], source, "--config", config,
                                    "--target", "js-module", "--format", "esm", "--mode", "production",
                                    "--logical-work", "40000000000", "--retained-bytes", "256000000",
                                    "--explain", "json", "-o", output], args.work / (name + "-compile"))
                    explanation, = [doc for doc in cost.json_documents(compiled.stderr) if "inputs" in doc]
                    policy = explanation["javascript_policy"]
                    if policy["effort"] != 13 or policy["objective"]["codec"].lower() != objective:
                        raise RuntimeError("compiler ignored the frozen objective or effort")
                    cost.effective_resources(explanation, policy)
                    for module in explanation["inputs"]["modules"]:
                        if identity(module["path"])["sha256"] != module["sha256"]:
                            raise RuntimeError("source changed during compilation")
                    observed = run([tools["node"]["path"], "--input-type=module", "-e", ORACLE,
                                    output, oracle], args.work / (name + "-runtime"))
                    encoded = run([tools["codec"]["path"], "--json", output], args.work / (name + "-codec"))
                    sizes = json.loads(encoded.stdout)["artifacts"][0]
                    winner = explanation["artifacts"][explanation["winners"][list(METRICS).index(objective)]]
                    artifact = identity(output)
                    if artifact["sha256"] != winner["sha256"] or sizes[metric] != winner[metric]:
                        raise RuntimeError("delivered artifact disagrees with its exact judgment")
                    row["compilers"][label] = dict(artifact=artifact, observation=json.loads(observed.stdout),
                        sizes={key: sizes[key] for key in METRICS.values()}, selected_bytes=sizes[metric],
                        policy_fingerprint=explanation.get("policy_fingerprint"), resources=explanation["resources"])
                    explanations.append(explanation)
                cost.compare_policies(*explanations, reason="V1 semantic and target optimization repairs; frozen contract, effort, objective and ceilings")
                if explanations[0]["inputs"] != explanations[1]["inputs"]:
                    raise RuntimeError("compiler input graphs differ")
                row["delta"] = row["compilers"]["candidate"]["selected_bytes"] - row["compilers"]["previous"]["selected_bytes"]
                save()
                print(stem, "qualified", row["delta"], flush=True)
        for value in [*tools.values(), report["manifest"]]:
            if identity(value["path"]) != value:
                raise RuntimeError("frozen experiment input changed")
        report["totals"] = {objective: sum(row["delta"] for row in report["rows"] if row["objective"] == objective) for objective in METRICS}
        report["complete"] = True
    except Exception as error:
        report["failures"].append(str(error))
        report["generalization"] = "Unqualified; investigate and follow the promotion/replacement protocol for a candidate failure."
        raise
    finally:
        save()


if __name__ == "__main__":
    main()
