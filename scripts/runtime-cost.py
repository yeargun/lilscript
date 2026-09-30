#!/usr/bin/env python3
"""Alternating runtime pairs with explicit oracles and pinned inputs.

See docs/testing.md for the manifest/workload protocol. Every sample uses a
fresh Node process; startup/import and warmed work are timed separately.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import resource
import shutil
import statistics
import subprocess
import time


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def describe(values):
    return {"median": statistics.median(values), "min": min(values), "max": max(values),
            "stdev": statistics.stdev(values) if len(values) > 1 else 0}


def positive_integer(value, label, minimum=1):
    if type(value) is not int or value < minimum:
        raise ValueError(f"{label} must be an integer >= {minimum}")
    return value


def read_json(text):
    def reject(value):
        raise ValueError(f"non-finite JSON number: {value}")
    return json.loads(text, parse_constant=reject)


def measure(args, report):
    manifest = read_json(args.manifest.read_text())
    if manifest.get("schema") != 1 or not manifest.get("workloads"):
        raise ValueError("manifest needs schema 1 and nonempty workloads")
    engine = Path(shutil.which(args.node) or args.node).resolve(strict=True)
    worker = Path(__file__).with_name("runtime-worker.mjs").resolve(strict=True)
    runner = Path(__file__).resolve(strict=True)
    identities = {}

    def pin(path):
        path = (args.manifest.parent / path).resolve(strict=True)
        if not path.is_file():
            raise ValueError(f"declare individual input files, not a directory: {path}")
        identities[str(path)] = digest(path)
        return str(path)

    for path in [engine, worker, runner, args.manifest]:
        pin(path)
    workloads, seen = [], set()
    for original in manifest["workloads"]:
        workload = dict(original)
        identifier = workload.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in seen:
            raise ValueError("workload IDs must be nonempty unique strings")
        seen.add(identifier)
        if not isinstance(workload.get("contract"), dict) or not workload["contract"]:
            raise ValueError(f"{identifier}: declare the matched contract")
        workload["iterations"] = positive_integer(workload.get("iterations"), "iterations")
        workload["warmup_runs"] = positive_integer(workload.get("warmup_runs", 1), "warmup_runs", 0)
        if "expected" not in workload:
            raise ValueError(f"{identifier}: an explicit expected oracle is required")
        counters = workload.get("counters")
        if not isinstance(counters, dict) or not counters:
            raise ValueError(f"{identifier}: declare nonempty workload counters")
        for name, count in counters.items():
            positive_integer(count, f"counter {name}", 0)
        workload["workload"] = pin(workload["workload"])
        for path in workload.get("inputs", []):
            pin(path)
        implementations = workload.get("implementations", {})
        if set(implementations) != {"reference", "candidate"}:
            raise ValueError(f"{identifier}: name reference and candidate implementations")
        workload["implementations"] = {}
        for label, implementation in implementations.items():
            if not isinstance(implementation, dict):
                raise ValueError(f"{identifier}/{label}: expected entry and input files")
            entry = pin(implementation["entry"])
            for path in implementation.get("inputs", []):
                pin(path)
            workload["implementations"][label] = {**implementation, "entry": entry}
        workloads.append(workload)

    # A common, small environment prevents ambient Node preload flags or path
    # hooks from changing one machine's measured program invisibly.
    environment = {"PATH": os.defpath, "TZ": "UTC", "LANG": "C", "LC_ALL": "C"}
    report.update({"schema": 1, "complete": False, "manifest": manifest,
                   "inputs": identities, "rounds": args.rounds,
                   "machine": {"platform": platform.platform(), "cpu_count": os.cpu_count()},
                   "environment": environment,
                   "engine": {"path": str(engine), "sha256": identities[str(engine)]},
                   "workloads": []})
    args.work.mkdir(parents=True, exist_ok=True)
    for number, workload in enumerate(workloads):
        directory = args.work / str(number)
        directory.mkdir(parents=True, exist_ok=True)
        row = {"id": workload["id"], "contract": workload["contract"], "samples": []}
        report["workloads"].append(row)
        samples = {"reference": [], "candidate": []}
        for round_index in range(args.rounds):
            order = ["reference", "candidate"] if round_index % 2 == 0 else ["candidate", "reference"]
            for label in order:
                stem = directory / f"{round_index + 1}-{label}"
                request = {key: workload[key] for key in
                           ["workload", "iterations", "warmup_runs", "expected", "counters"]}
                request["entry"] = workload["implementations"][label]["entry"]
                request_path = stem.with_suffix(".request.json")
                request_path.write_text(json.dumps(request) + "\n")
                load = os.getloadavg()
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                started = time.monotonic()
                result = subprocess.run([str(engine), "--expose-gc", str(worker), str(request_path)],
                                        capture_output=True, text=True, timeout=args.timeout,
                                        cwd=args.manifest.parent, env=environment)
                wall = time.monotonic() - started
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                stem.with_suffix(".stdout").write_text(result.stdout)
                stem.with_suffix(".stderr").write_text(result.stderr)
                if result.returncode:
                    raise RuntimeError(f"{workload['id']}/{label}: worker exited {result.returncode}: {result.stderr[-2000:]}")
                observation = read_json(result.stdout)
                if (observation.get("schema") != 1 or observation.get("oracle") != workload["expected"]
                        or observation.get("counters") != workload["counters"]):
                    raise RuntimeError("worker returned an invalid oracle or protocol")
                metrics = observation["metrics"]
                for value in metrics.values():
                    if value is not None and (type(value) not in [int, float] or not math.isfinite(value)):
                        raise RuntimeError("worker returned a non-finite runtime measurement")
                metrics.update({"process_cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
                                "process_wall_seconds": wall})
                observation.update({"implementation": label, "round": round_index + 1,
                                    "load_before": load, "load_after": os.getloadavg()})
                row["samples"].append(observation)
                samples[label].append(observation)
        metrics = list(samples["reference"][0]["metrics"])
        row["summary"], row["ratios"] = {}, {}
        for label, observations in samples.items():
            row["summary"][label] = {
                metric: describe([sample["metrics"][metric] for sample in observations])
                if all(sample["metrics"][metric] is not None for sample in observations) else None
                for metric in metrics}
        for metric in metrics:
            before, after = [row["summary"][label][metric] for label in ["reference", "candidate"]]
            # Nonpositive retained deltas are valid observations but cannot
            # support a meaningful memory ratio. Preserve them without clamping.
            valid = before and after and before["min"] > 0 and after["min"] >= 0
            row["ratios"][metric] = None if not valid else {
                "ratio_of_medians": after["median"] / before["median"],
                "paired": [candidate["metrics"][metric] / reference["metrics"][metric]
                           for reference, candidate in zip(samples["reference"], samples["candidate"])]}
        print(f"{workload['id']}: {2 * args.rounds} isolated samples, oracle passed", flush=True)
    for path, expected in identities.items():
        if digest(path) != expected:
            raise RuntimeError(f"input changed during runtime measurement: {path}")
    report["complete"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["manifest", "work", "json"]:
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--node", default="node")
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    if args.rounds < 1 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("rounds and timeout must be positive and finite")
    args.manifest, args.work, args.json = [path.resolve() for path in [args.manifest, args.work, args.json]]
    # Keep reports away from inputs, including on failure. A dedicated empty
    # work directory also preserves prior samples instead of overwriting them.
    if args.json.exists():
        parser.error("the report path already exists; choose a new path")
    if args.work.exists() and any(args.work.iterdir()):
        parser.error("the work directory must be empty")
    report = {"schema": 1, "complete": False}
    try:
        measure(args, report)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
