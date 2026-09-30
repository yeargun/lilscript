#!/usr/bin/env python3
"""Sequential, alternating process-CPU comparisons with retained samples.

Manifest: {"workloads":[{"id":"example","cwd":"/path","entry":"main.lil",
"config":"lilscript.toml","arguments":[]}]}. Relative cwd paths resolve
against the manifest. Targets default to js-module; arguments apply equally
to both compilers. The compiler's explain report pins transitive source inputs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess
import time


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def json_documents(text):
    decoder, at = json.JSONDecoder(), 0
    while (at := text.find("{", at)) >= 0:
        try:
            value, length = decoder.raw_decode(text[at:])
        except ValueError:
            at += 1
            continue
        yield value
        at += length


def compile_one(binary, workload, output, timeout, explain=False):
    args = [binary, workload["entry"], "--target", workload.get("target", "js-module"),
            "--config", workload["config"], *workload.get("arguments", []), "-o", str(output)]
    environment = os.environ.copy()
    environment.pop("LILSCRIPT_TIMING", None)
    if explain:
        args.extend(["--explain", "json"])
        environment["LILSCRIPT_TIMING"] = "1"
    load = os.getloadavg()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.monotonic()
    result = subprocess.run(args, cwd=workload["cwd"], env=environment,
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=timeout)
    wall = time.monotonic() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    stderr = result.stderr.decode("utf-8", errors="replace")
    if explain or result.returncode:
        output.with_suffix(output.suffix + ".stderr").write_text(stderr)
    if result.returncode:
        raise RuntimeError(f"{workload['id']}: compiler exited {result.returncode}: {stderr[-1000:]}")
    sample = {"cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
              "wall_seconds": wall, "load_before": load, "load_after": os.getloadavg(),
              "bytes": output.stat().st_size, "sha256": digest(output)}
    if explain:
        documents = list(json_documents(stderr))
        report = next((value for value in documents if "inputs" in value), None)
        if report is None:
            raise RuntimeError("compiler produced no input-bearing explain report")
        sample["explain"] = report
        sample["phase_elapsed"] = next((value for value in documents if "wall_ms" in value), None)
    return sample


def summary(samples):
    result = {}
    for key in ["cpu_seconds", "wall_seconds"]:
        values = [sample[key] for sample in samples]
        result[key] = {"median": statistics.median(values), "min": min(values),
                       "max": max(values), "stdev": statistics.stdev(values) if len(values) > 1 else 0}
    return result


def measure(args, report):
    manifest = json.loads(args.manifest.read_text())
    binaries = {key: str(Path(getattr(args, key)).resolve()) for key in ["previous", "candidate"]}
    identities = {key: {"path": path, "sha256": digest(path)} for key, path in binaries.items()}
    codec = str(args.codec.resolve())
    report.update({"schema": 1, "compilers": identities, "codec": {"path": codec, "sha256": digest(codec)},
                   "manifest": manifest, "rounds": args.rounds, "workloads": [], "complete": False})
    seen = set()
    for index, original in enumerate(manifest["workloads"]):
        workload = dict(original)
        if workload["id"] in seen:
            raise ValueError("workload IDs must be unique")
        seen.add(workload["id"])
        workload["cwd"] = str((args.manifest.parent / workload["cwd"]).resolve())
        config = Path(workload["cwd"]) / workload["config"]
        config_hash = digest(config)
        directory = args.work / str(index)
        directory.mkdir(parents=True, exist_ok=True)
        outputs = {key: directory / f"{key}.js" for key in binaries}
        warm = {key: compile_one(binary, workload, outputs[key], args.timeout, True)
                for key, binary in binaries.items()}
        previous, candidate = [warm[key]["explain"] for key in binaries]
        if previous["inputs"] != candidate["inputs"]:
            raise RuntimeError(f"{workload['id']}: compilers reported different inputs")
        for field in ["javascript_policy", "native_policy"]:
            if field not in previous or field not in candidate:
                raise RuntimeError(f"{workload['id']}: missing {field} in explain report")
            if previous[field] != candidate[field]:
                raise RuntimeError(f"{workload['id']}: compilers resolved different {field}")
        samples, ordered = {key: [] for key in binaries}, []
        row = {"id": workload["id"], "cwd": workload["cwd"], "config_sha256": config_hash,
               "warmup": warm, "samples": ordered}
        report["workloads"].append(row)
        for round_index in range(args.rounds):
            order = ["previous", "candidate"] if round_index % 2 == 0 else ["candidate", "previous"]
            for key in order:
                sample = compile_one(binaries[key], workload, outputs[key], args.timeout)
                sample.update({"compiler": key, "round": round_index + 1})
                ordered.append(sample)
                samples[key].append(sample)
                if sample["sha256"] != warm[key]["sha256"]:
                    raise RuntimeError(f"{workload['id']}: {key} output changed across runs")
        for module in previous["inputs"]["modules"]:
            if digest(module["path"]) != module["sha256"]:
                raise RuntimeError(f"input changed: {module['path']}")
        if digest(config) != config_hash:
            raise RuntimeError(f"configuration changed: {config}")
        row["summary"] = {key: summary(values) for key, values in samples.items()}
        row["cpu_ratio"] = (row["summary"]["candidate"]["cpu_seconds"]["median"] /
                            row["summary"]["previous"]["cpu_seconds"]["median"])
        result = subprocess.run([codec, "--json", *map(str, outputs.values())],
                                capture_output=True, check=True, text=True, timeout=args.timeout)
        row["sizes"] = json.loads(result.stdout)
        print(f"{workload['id']}: CPU ratio {row['cpu_ratio']:.3f}, {len(ordered)} retained samples", flush=True)
    for identity in [*identities.values(), report["codec"]]:
        if digest(identity["path"]) != identity["sha256"]:
            raise RuntimeError(f"binary changed: {identity['path']}")
    report["complete"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["previous", "candidate"]:
        parser.add_argument(f"--{name}", required=True)
    for name in ["manifest", "codec", "work", "json"]:
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=1800)
    args = parser.parse_args()
    if args.rounds < 1 or args.timeout <= 0:
        parser.error("rounds and timeout must be positive")
    args.manifest, args.work = args.manifest.resolve(), args.work.resolve()
    report = {}
    try:
        measure(args, report)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
