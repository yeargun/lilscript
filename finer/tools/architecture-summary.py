"""Summarize a completed architecture experiment without running a compiler.

Pairs compare total internal compilation time, jointly resampled by trial.
All workload rows and all artifact sizes remain visible, including regressions.
"""
import argparse
import hashlib
import json
import random
import statistics
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--pair", action="append", required=True, help="numerator,denominator modes")
parser.add_argument("--resamples", type=int, default=5000)
parser.add_argument("--seed", type=int, default=73195)
args = parser.parse_args()
assert not args.output.exists(), "summary output must be new"
assert 1000 <= args.resamples <= 100000
report = json.loads(args.report.read_text())
assert report["status"] == "passed", "incomplete or failed experiments are not accepted"
pairs = [value.split(",") for value in args.pair]
assert all(len(pair) == 2 and pair[0] != pair[1] for pair in pairs)
rng = random.Random(args.seed)
median = statistics.median


def interval(first, second):
    assert first.keys() == second.keys(), "paired trial inventory differs"
    trials = sorted(first)
    assert len(trials) == report["trials"]
    ratios = []
    for _ in range(args.resamples):
        sample = rng.choices(trials, k=len(trials))
        ratios.append(median(first[t] for t in sample) / median(second[t] for t in sample))
    ratios.sort()
    return [ratios[int(args.resamples * .025)], ratios[int(args.resamples * .975)]]


rows = []
for workload in report["workloads"]:
    artifacts = {row["name"]: row for row in workload["artifacts"]}
    assert len(artifacts) == len(workload["artifacts"])
    assert all(row["test"]["exitCode"] == 0 and row["test"]["stdoutSha256"] == workload["expected"]
               for row in artifacts.values()), "an artifact lacks the required behavior result"
    eligible_references = []
    rejected_references = []
    for attempt in workload["referenceAttempts"]:
        if attempt["status"] in ["emitted", "passed"]:
            assert attempt["name"] in artifacts
            eligible_references.append(attempt)
        else:
            assert attempt["status"] in ["behavior-failed", "failed"]
            assert attempt["name"] not in artifacts
            if attempt["status"] == "behavior-failed":
                assert attempt["test"]["exitCode"] != 0 or attempt["test"]["stdoutSha256"] != workload["expected"] or attempt["test"].get("error")
            rejected_references.append(attempt)
    compiles = {}
    for row in workload["compiles"]:
        mode = compiles.setdefault(row["mode"], {})
        assert row["trial"] not in mode
        assert row["sha256"] == artifacts[row["mode"]]["sha256"]
        mode[row["trial"]] = row
    assert all(len(trials) == report["trials"] for trials in compiles.values())
    summaries = {}
    for mode, trials in compiles.items():
        metrics = [row["metrics"] for row in trials.values()]
        summaries[mode] = {
            "medianMilliseconds": {stage: median(row["milliseconds"][stage] for row in metrics)
                                   for stage in metrics[0]["milliseconds"]},
            "medianProcessWallMs": median(row["elapsedMs"] for row in trials.values()),
            "medianPeakRssKiB": median(row["peakRssKiB"] for row in trials.values()),
            "expressionStorageSlots": sorted(set(row["expressionStorageSlots"] for row in metrics)),
            # Work is deterministic; compare every trial before retaining it once.
            "roundWork": metrics[0]["rounds"],
            "targetWork": metrics[0]["targetChoices"],
        }
        assert all(row["rounds"] == summaries[mode]["roundWork"] for row in metrics)
        assert all(row["targetChoices"] == summaries[mode]["targetWork"] for row in metrics)
    comparisons = []
    for first_mode, second_mode in pairs:
        first = {trial: row["metrics"]["milliseconds"]["total"] for trial, row in compiles[first_mode].items()}
        second = {trial: row["metrics"]["milliseconds"]["total"] for trial, row in compiles[second_mode].items()}
        assert all(value > 0 for value in [*first.values(), *second.values()])
        comparisons.append({
            "first": first_mode, "second": second_mode,
            "firstMedianMs": median(first.values()), "secondMedianMs": median(second.values()),
            "medianRatio": median(first.values()) / median(second.values()),
            "pairedBootstrap95": interval(first, second),
            "sameArtifact": artifacts[first_mode]["sha256"] == artifacts[second_mode]["sha256"],
        })
    rows.append({
        "case": workload["name"], "comparisons": comparisons, "modes": summaries,
        "sizes": {name: {codec: row[codec] for codec in ["raw", "gzip9", "brotli11"]}
                  for name, row in artifacts.items()},
        "bestMeasuredTerser": {
            codec: min((row[codec] for name, row in artifacts.items() if name.startswith("terser-")), default=None)
            for codec in ["raw", "gzip9", "brotli11"]},
        "referenceCoverage": {"eligible": len(eligible_references), "rejected": len(rejected_references)},
        "rejectedReferences": rejected_references,
    })
summary = {
    "schemaVersion": 1, "status": "passed-paired-experiment", "scope": report["scope"],
    "experiment": {"path": str(args.report.resolve()),
                   "sha256": hashlib.sha256(args.report.read_bytes()).hexdigest(),
                   "bytes": args.report.stat().st_size},
    "compiler": report["compiler"], "previousCompiler": report["previousCompiler"],
    "terserVersion": report["terserVersion"], "terserPasses": report["terserPasses"],
    "terserProfiles": report.get("terserProfiles", ["standard"]),
    "compileTrials": sum(len(row["compiles"]) for row in report["workloads"]),
    "testedArtifacts": sum(len(row["artifacts"]) for row in report["workloads"]),
    "distinctCodecEvaluations": sum(row["codecEvaluations"] for row in report["workloads"]),
    "bootstrap": {"resamples": args.resamples, "seed": args.seed,
                  "statistic": "ratio of medians, resampled jointly by trial"},
    "workloads": rows,
}
with args.output.open("x") as output:
    json.dump(summary, output, indent=2)
    output.write("\n")
print(json.dumps({key: summary[key] for key in ["compileTrials", "testedArtifacts", "distinctCodecEvaluations"]}))
