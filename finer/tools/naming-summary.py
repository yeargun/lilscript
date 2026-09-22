"""Summarize a completed naming experiment, retaining every workload and loss."""
import argparse
import hashlib
import json
import random
import statistics
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
assert not args.output.exists()
report = json.loads(args.report.read_text())
assert report["status"] == "passed"
assert report["scope"] == "bounded-naming-search-experiment"
median = statistics.median
rng = random.Random(73195)
metrics = ["raw", "gzip9", "brotli11"]
pairs = [("global", "previous")] if "previous" in report["modes"] else []
if "scoped" in report["modes"]:
    pairs.append(("scoped", "global"))
pairs += [(f"budget-{budget}", "global") for budget in report["budgets"]]
if report.get("objectiveStudy"):
    pairs += [(f"{goal}-{budget}", f"budget-{budget}") for budget in report["budgets"] for goal in ["raw", "gzip", "brotli"]]
mode_specs = {spec["name"]: spec for spec in report.get("modeSpecs", [])}


def interval(first, second):
    assert first.keys() == second.keys()
    trials = sorted(first)
    ratios = []
    for _ in range(5000):
        sample = rng.choices(trials, k=len(trials))
        ratios.append(median(first[t] for t in sample) / median(second[t] for t in sample))
    ratios.sort()
    return [ratios[125], ratios[4875]]


rows = []
for workload in report["workloads"]:
    artifacts = {row["name"]: row for row in workload["artifacts"]}
    hashes = {row["sha256"]: row for row in artifacts.values()}
    assert all(row["test"]["exitCode"] == 0 and row["test"]["stdoutSha256"] == workload["expected"]
               for row in artifacts.values())
    compiles = {}
    for row in workload["compiles"]:
        trials = compiles.setdefault(row["mode"], {})
        assert row["trial"] not in trials
        trials[row["trial"]] = row
    assert set(compiles) == set(report["modes"])
    assert all(len(trials) == report["trials"] for trials in compiles.values())
    summaries = {}
    for mode, trials in compiles.items():
        sample = list(trials.values())
        summaries[mode] = {
            "medianMilliseconds": {key: median(row["metrics"]["milliseconds"][key] for row in sample)
                                   for key in sample[0]["metrics"]["milliseconds"]},
            "medianProcessWallMs": median(row["elapsedMs"] for row in sample),
            "medianPeakRssKiB": median(row["peakRssKiB"] for row in sample),
        }
        if mode.startswith("budget-") or mode_specs.get(mode, {}).get("budget"):
            search = sample[0]["metrics"]["search"]
            summaries[mode]["work"] = {key: search[key] for key in ["proposalSteps", "renderedBytes",
                "retainedBytes", "measurementCalls", "duplicateArtifacts"]}
            summaries[mode]["work"]["rejectedPlans"] = sum(attempt["rejection"] is not None for attempt in search["attempts"])
            summaries[mode]["winners"] = {}
            for index, metric in enumerate(metrics):
                winner = search["winners"][index]
                if winner is None:
                    continue
                candidate = search["candidates"][winner]
                actual = hashes[candidate["sha256"]]
                assert all(candidate["sizes"][key] == actual[key] for key in metrics if candidate["sizes"][key] is not None)
                assert candidate["sizes"][metric] == min(row["sizes"][metric] for row in search["candidates"])
                summaries[mode]["winners"][metric] = {
                    "sha256": candidate["sha256"], "plan": candidate["plan"], "sizes": {key: actual[key] for key in metrics},
                    "measuredDuringCompile": candidate["sizes"]}
        else:
            actual = artifacts[mode]
            assert all(row["sha256"] == actual["sha256"] for row in sample)
            summaries[mode]["artifact"] = {key: actual[key] for key in ["sha256", *metrics]}
    comparisons = []
    for first, second in pairs:
        first_times = {trial: row["metrics"]["milliseconds"]["total"] for trial, row in compiles[first].items()}
        second_times = {trial: row["metrics"]["milliseconds"]["total"] for trial, row in compiles[second].items()}
        comparisons.append({"first": first, "second": second,
            "firstMedianMs": median(first_times.values()), "secondMedianMs": median(second_times.values()),
            "medianRatio": median(first_times.values()) / median(second_times.values()),
            "pairedBootstrap95": interval(first_times, second_times)})
    rows.append({"case": workload["name"], "contract": workload.get("contract"),
        "comparisons": comparisons, "modes": summaries,
        "previousAndGlobalSameArtifact": artifacts["previous"]["sha256"] == artifacts["global"]["sha256"] if "previous" in artifacts else None,
        "bestMeasuredTerser": {metric: min((row[metric] for name, row in artifacts.items()
            if name.startswith("reference-terser-")), default=None) for metric in metrics},
        "rejectedReferences": [attempt for attempt in workload.get("referenceAttempts", [])
                               if attempt["status"] not in ["passed", "emitted"]],
        "distinctTestedArtifacts": workload["distinctTestedArtifacts"],
        "references": workload["references"]})

summary = {
    "schemaVersion": 1, "status": "passed-paired-naming-experiment", "scope": report["scope"],
    "source": {"path": str(args.report.resolve()), "sha256": hashlib.sha256(args.report.read_bytes()).hexdigest(),
               "bytes": args.report.stat().st_size},
    "compiler": report["compiler"], "previousCompiler": report["previousCompiler"],
    "compileTrials": sum(len(row["compiles"]) for row in report["workloads"]),
    "archivalCompiles": sum(len(row["archives"]) for row in report["workloads"]),
    "testedArtifactRecords": sum(len(row["artifacts"]) for row in report["workloads"]),
    "distinctTestedArtifacts": sum(row["distinctTestedArtifacts"] for row in report["workloads"]),
    "distinctExternalCodecEvaluations": sum(row["distinctCodecEvaluations"] for row in report["workloads"]),
    "budgets": report["budgets"], "candidateByteBudget": report["candidateByteBudget"],
    "objectiveStudy": report.get("objectiveStudy", False), "modeSpecs": report.get("modeSpecs"),
    "bootstrap": {"resamples": 5000, "seed": 73195, "statistic": "ratio of medians, resampled jointly by trial"},
    "workloads": rows,
    "limits": ["Bounded programs, not maintained-library acceptance.",
        "Exact sizes select each codec independently. Timing intervals describe this worker and run.",
        "Requested scores guide each search; external replay measures all three scores after timing. Ordinary rendering does not run codecs.",
        "Logical text bounds do not bound allocator or whole-process memory."]
}
args.output.write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({key: summary[key] for key in ["compileTrials", "archivalCompiles", "distinctTestedArtifacts"]}))
