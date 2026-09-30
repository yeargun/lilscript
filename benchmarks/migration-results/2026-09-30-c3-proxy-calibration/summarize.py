#!/usr/bin/env python3
"""Verify retained receipts and summarize matched proxy-mode experiments."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
spec = importlib.util.spec_from_file_location("compile_cost", REPOSITORY / "scripts/compile-cost.py")
cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cost)
METRICS = {"raw": (0, "raw"), "gzip9": (1, "gzip"), "brotli11": (2, "brotli")}
MODES = ["on", "audit", "off"]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def normalize(value, work):
    if isinstance(value, dict):
        return {key: normalize(child, work) for key, child in value.items()}
    if isinstance(value, list):
        return [normalize(child, work) for child in value]
    if isinstance(value, str) and value.startswith(str(work) + "/"):
        return "$generated/" + value[len(str(work)) + 1:]
    return value


def main():
    work_root = Path(sys.argv[1]).resolve()
    reports = {mode: json.loads((HERE / f"{mode}.json").read_text()) for mode in MODES}
    reference = reports["on"]
    assert reference["items"] == 642
    assert digest(reference["compiler"]["source"]) == reference["compiler"]["sha256"]
    assert digest(Path(reference["compiler"]["source"]).with_name("lilscript-codec")) == reference["codec"]["sha256"]
    assert digest(REPOSITORY / "tests/ratchet/bars.json") == reference["barsSha256"]
    for mode, report in reports.items():
        assert not report["failures"], (mode, report["failures"])
        for key in ["compiler", "codec", "delivery", "sets", "filter", "items", "barsSha256"]:
            assert report[key] == reference[key], (mode, key)
        for config in report["configurations"]:
            assert digest(config["path"]) == config["sha256"]
            policy = config["resolved"]["policy"]
            assert policy["effort"] == 13
            assert policy["objective"]["search"]["proxy_pruning"] == mode
    verified_files = {}
    output = {
        "schema": 1,
        "scope": "642 inspected development programs; no held-out or CPU claim",
        "summarizer_sha256": digest(__file__),
        "decoder_sha256": digest(REPOSITORY / "scripts/compile-cost.py"),
        "reports": {mode: digest(HERE / f"{mode}.json") for mode in MODES},
        "compiler": reference["compiler"],
        "codec": reference["codec"],
        "work_root": str(work_root),
        "metrics": {},
        "changed": [],
        "potential_proxy_misses": [],
    }
    for metric, (index, lane) in METRICS.items():
        totals = {mode: dict(bytes=0, judged=0, pruned=0, codec_probes=0,
                            baseline_work=0, optional_work=0, codec_work=0,
                            max_peak_retained_bytes=0, audited=0, pruned_starts=0,
                            structural_stopped=0, terminal_stopped=0) for mode in MODES}
        for item in reference["table"]:
            facts = {}
            sizes = {}
            hashes = {}
            starts = {}
            for mode in MODES:
                row = reports[mode]["table"][item][metric]
                assert row["state"] == "pass", (mode, item, metric)
                work = work_root / mode
                explanation = work / row["explanation"]["path"]
                assert digest(explanation) == row["explanation"]["sha256"]
                documents = [doc for doc in cost.json_documents(explanation.read_text()) if "inputs" in doc]
                assert len(documents) == 1
                report = documents[0]
                policy = copy.deepcopy(report["javascript_policy"])
                assert policy["effort"] == 13
                assert policy["objective"]["search"].pop("proxy_pruning") == mode
                facts[mode] = (normalize(report["inputs"], work), policy, report["request"])
                for source in report["inputs"]["modules"]:
                    path = source["path"]
                    if path not in verified_files:
                        verified_files[path] = digest(path)
                    assert verified_files[path] == source["sha256"]
                artifact = report["artifacts"][report["winners"][index]]
                assert artifact[metric] == row["size"]
                assert artifact["sha256"].startswith(row["sha256"])
                assert digest(explanation.with_name(f"{lane}.js")) == artifact["sha256"]
                sizes[mode] = row["size"]
                hashes[mode] = artifact["sha256"]
                terminal = report["search"]["terminal"]["objectives"][0]
                starts[mode] = terminal["starts"]
                total = totals[mode]
                total["bytes"] += row["size"]
                for key in ["judged", "pruned", "codec_probes"]:
                    total[key] += terminal[key]
                for key in ["baseline_work", "optional_work", "codec_work"]:
                    total[key] += report["resources"][key]
                total["max_peak_retained_bytes"] = max(total["max_peak_retained_bytes"], report["resources"]["peak_retained_bytes"])
                total["structural_stopped"] += int(report["search"]["stop"] is not None)
                total["terminal_stopped"] += int(any(trial["outcome"] == "stopped"
                    for kind in ["trials", "choice_trials", "joint_trials", "starts"]
                    for trial in terminal[kind]))
                total["pruned_starts"] += sum(trial["outcome"] == "pruned" for trial in terminal["starts"])
                for kind in ["trials", "choice_trials", "joint_trials", "starts"]:
                    for trial in terminal[kind]:
                        if trial.get("audit") is not None:
                            assert mode == "audit"
                            total["audited"] += 1
                            if trial["audit"] < 0:
                                output["potential_proxy_misses"].append({"item": item, "metric": metric, "kind": kind, "trial": trial})
            assert facts["on"] == facts["audit"] == facts["off"], (item, metric, "unmatched inputs, policy or resources")
            assert hashes["on"] == hashes["audit"], (item, metric, "audit changed output")
            if sizes["on"] != sizes["off"]:
                output["changed"].append({"item": item, "metric": metric, "sizes": sizes, "delta": sizes["off"] - sizes["on"], "sha256": hashes, "starts": starts})
        output["metrics"][metric] = totals
    output["verified_source_files"] = len(verified_files)
    evidence = [{"item": row["item"], "metric": row["metric"], "delta": row["delta"],
                 "starts": row["starts"], "previously_pruned_kept": [trial for trial in row["starts"]["off"]
                     if trial["outcome"] == "kept" and any(old["name"] == trial["name"] and old["outcome"] == "pruned" for old in row["starts"]["on"])]}
                for row in output["changed"]]
    (HERE / "restart-evidence.json").write_text(json.dumps(evidence, indent=1) + "\n")
    (HERE / "summary.json").write_text(json.dumps(output, indent=1) + "\n")
    print(json.dumps({"metrics": output["metrics"], "changed": output["changed"], "potential_proxy_misses": len(output["potential_proxy_misses"])}, indent=1))


if __name__ == "__main__":
    main()
