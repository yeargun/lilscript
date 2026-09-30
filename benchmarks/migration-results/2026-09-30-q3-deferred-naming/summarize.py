#!/usr/bin/env python3
"""Verify and compare the effort-13 protected-tail experiment with C3's pin."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PREVIOUS = HERE.parent / "2026-09-30-c3-proxy-calibration"
spec = importlib.util.spec_from_file_location("compile_cost", REPO / "scripts/compile-cost.py")
cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cost)
METRICS = {"raw": (0, "raw"), "gzip9": (1, "gzip"), "brotli11": (2, "brotli")}


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
    roots = dict(zip(["previous", "candidate"], map(lambda p: Path(p).resolve(), sys.argv[1:])))
    assert len(roots) == 2, "previous and candidate measurement directories are required"
    paths = {"previous": PREVIOUS / "on.json", "candidate": HERE / "effort13.json"}
    reports = {role: json.loads(path.read_text()) for role, path in paths.items()}
    old = reports["previous"]
    assert old["items"] == 642
    for role, report in reports.items():
        assert not report["failures"], (role, report["failures"])
        for field in ["delivery", "sets", "filter", "items", "barsSha256"]:
            assert report[field] == old[field], (role, field)
        assert digest(report["compiler"]["source"]) == report["compiler"]["sha256"]
        assert digest(Path(report["compiler"]["source"]).with_name("lilscript-codec")) == report["codec"]["sha256"]
        assert digest(REPO / "tests/ratchet/bars.json") == report["barsSha256"]
        for config in report["configurations"]:
            assert digest(config["path"]) == config["sha256"]
            assert config["resolved"]["policy"]["effort"] == 13
    assert set(reports["candidate"]["table"]) == set(old["table"])
    assert [(c["set"], c["lane"], c["sha256"]) for c in reports["candidate"]["configurations"]] == [(c["set"], c["lane"], c["sha256"]) for c in old["configurations"]]
    summary = {"schema": 1, "scope": "642 inspected development programs; matched effort 13; no held-out claim",
               "reports": {role: digest(path) for role, path in paths.items()},
               "compilers": {role: report["compiler"] for role, report in reports.items()},
               "codecs": {role: report["codec"] for role, report in reports.items()},
               "summarizer_sha256": digest(__file__), "decoder_sha256": digest(REPO / "scripts/compile-cost.py"),
               "work_roots": {role: str(path) for role, path in roots.items()}, "metrics": {}, "changed": []}
    verified = {}
    for metric, (index, lane) in METRICS.items():
        totals = {role: dict(bytes=0, judged=0, pruned=0, codec_probes=0, heads=0,
                            baseline_work=0, optional_work=0, codec_work=0,
                            max_peak_retained_bytes=0, deferred=0, deferred_kept=0,
                            structural_stopped=0, terminal_stopped=0) for role in reports}
        for item in old["table"]:
            facts, starts, sizes, hashes, terminals = {}, {}, {}, {}, {}
            for role, report in reports.items():
                row = report["table"][item][metric]
                assert row["state"] == "pass", (role, item, metric)
                work = roots[role]
                explanation = work / row["explanation"]["path"]
                assert digest(explanation) == row["explanation"]["sha256"]
                documents = [doc for doc in cost.json_documents(explanation.read_text()) if "inputs" in doc]
                assert len(documents) == 1
                receipt = documents[0]
                policy = copy.deepcopy(receipt["javascript_policy"])
                assert policy["effort"] == 13
                search = policy["objective"]["search"]
                assert search["proxy_pruning"] == "on"
                assert search.pop("version") == (27 if role == "previous" else 28)
                assert policy["objective"]["walk"].pop("version") == (6 if role == "previous" else 7)
                if role == "candidate":
                    assert search.pop("deferred_naming_starts") is True
                facts[role] = (normalize(receipt["inputs"], work), policy, receipt["request"])
                for source in receipt["inputs"]["modules"]:
                    path = source["path"]
                    if path not in verified:
                        verified[path] = digest(path)
                    assert verified[path] == source["sha256"]
                artifact = receipt["artifacts"][receipt["winners"][index]]
                assert artifact[metric] == row["size"]
                assert artifact["sha256"].startswith(row["sha256"])
                assert digest(explanation.with_name(f"{lane}.js")) == artifact["sha256"]
                sizes[role], hashes[role] = row["size"], artifact["sha256"]
                terminal = terminals[role] = receipt["search"]["terminal"]["objectives"][0]
                starts[role] = terminal["starts"]
                total = totals[role]
                total["bytes"] += row["size"]
                for key in ["judged", "pruned", "codec_probes", "heads"]:
                    total[key] += terminal[key]
                for key in ["baseline_work", "optional_work", "codec_work"]:
                    total[key] += receipt["resources"][key]
                total["max_peak_retained_bytes"] = max(total["max_peak_retained_bytes"], receipt["resources"]["peak_retained_bytes"])
                total["structural_stopped"] += int(receipt["search"]["stop"] is not None)
                total["terminal_stopped"] += int(any(trial["outcome"] == "stopped"
                    for kind in ["trials", "choice_trials", "joint_trials", "starts"] for trial in terminal[kind]))
                deferred = [s for s in terminal["starts"] if s["name"].startswith("deferred-naming")]
                total["deferred"] += len(deferred)
                total["deferred_kept"] += sum(s["outcome"] == "kept" for s in deferred)
            assert facts["previous"] == facts["candidate"], (item, metric, "unmatched task")
            for kind in ["trials", "choice_trials", "joint_trials", "starts", "stops"]:
                prefix = terminals["previous"][kind]
                assert terminals["candidate"][kind][:len(prefix)] == prefix, (item, metric, kind, "prefix changed")
            assert sizes["candidate"] <= sizes["previous"], (item, metric, "growth")
            if hashes["previous"] != hashes["candidate"]:
                summary["changed"].append({"item": item, "metric": metric, "sizes": sizes,
                    "delta": sizes["candidate"] - sizes["previous"], "sha256": hashes, "starts": starts})
        summary["metrics"][metric] = totals
    summary["verified_source_files"] = len(verified)
    (HERE / "effort13-summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps({"metrics": summary["metrics"], "changed": len(summary["changed"]),
                      "verified_source_files": len(verified)}, indent=1))


if __name__ == "__main__":
    main()
