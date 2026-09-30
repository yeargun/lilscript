#!/usr/bin/env python3
"""Exercise real isolated workers and the failure receipts, not timing values."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

RUNNER = Path(__file__).with_name("runtime-cost.py")


class RuntimeCostTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name in ["reference", "candidate"]:
            (self.root / f"{name}.mjs").write_text("export const twice = n => n * 2;\n")
        (self.root / "workload.mjs").write_text("""
export function run(api, iterations) {
  let sum = 0;
  for (let n = 0; n < iterations; n++) sum += api.twice(n);
  return {oracle: sum, counters: {calls: iterations}};
}
""")
        self.manifest = {"schema": 1, "workloads": [{
            "id": "twice", "contract": {"exports": ["twice"], "target": "ESM"},
            "workload": "workload.mjs", "iterations": 100, "warmup_runs": 1,
            "expected": 9900, "counters": {"calls": 100},
            "implementations": {label: {"entry": f"{label}.mjs"}
                                for label in ["reference", "candidate"]}}]}

    def run_measurement(self, *, rounds=1, environment=None, timeout=10):
        manifest = self.root / "manifest.json"
        manifest.write_text(json.dumps(self.manifest))
        result = subprocess.run([
            sys.executable, str(RUNNER), "--manifest", str(manifest),
            "--work", str(self.root / "samples"), "--json", str(self.root / "report.json"),
            "--rounds", str(rounds), "--timeout", str(timeout)],
            capture_output=True, text=True, env=environment)
        return result, json.loads((self.root / "report.json").read_text())

    def test_pairs_keep_order_identity_oracles_and_distinct_metrics(self):
        # Ambient Node options must not inject another module into the worker.
        env = {**os.environ, "NODE_OPTIONS": "--require=/missing/preload.cjs"}
        result, report = self.run_measurement(rounds=2, environment=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(report["complete"])
        samples = report["workloads"][0]["samples"]
        self.assertEqual([(s["round"], s["implementation"]) for s in samples],
                         [(1, "reference"), (1, "candidate"), (2, "candidate"), (2, "reference")])
        self.assertIn(str(self.root / "candidate.mjs"), report["inputs"])
        self.assertEqual(len(report["engine"]["sha256"]), 64)
        for sample in samples:
            self.assertEqual(sample["oracle"], 9900)
            self.assertEqual(sample["counters"], {"calls": 100})
            metrics = sample["metrics"]
            for name in ["startup_wall_ns", "startup_cpu_us", "steady_wall_ns", "steady_cpu_us",
                         "process_cpu_seconds", "process_wall_seconds", "rss_bytes"]:
                self.assertGreaterEqual(metrics[name], 0)
            self.assertIsNone(metrics["retained_heap_bytes"])
            self.assertIsNone(metrics["retained_array_buffer_bytes"])

    def test_wrong_result_is_not_a_timing_win(self):
        (self.root / "candidate.mjs").write_text("export const twice = n => n;\n")
        result, report = self.run_measurement()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["complete"])
        self.assertIn("workload oracle differs", report["error"])
        self.assertTrue((self.root / "samples/0/1-candidate.stderr").is_file())

    def test_wrong_counters_are_refused(self):
        self.manifest["workloads"][0]["counters"] = {"calls": 99}
        result, report = self.run_measurement()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["complete"])
        self.assertIn("declared workload counters differ", report["error"])

    def test_changed_dependency_invalidates_all_samples(self):
        dependency = self.root / "data.json"
        dependency.write_text("original")
        self.manifest["workloads"][0]["inputs"] = ["data.json"]
        workload = self.root / "workload.mjs"
        workload.write_text('import {writeFileSync} from "node:fs";\n'
                            'writeFileSync(new URL("./data.json", import.meta.url), "changed");\n'
                            + workload.read_text())
        result, report = self.run_measurement()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["complete"])
        self.assertIn("input changed during runtime measurement", report["error"])
        self.assertEqual(len(report["workloads"][0]["samples"]), 2)

    def test_timeout_retains_incomplete_report(self):
        (self.root / "candidate.mjs").write_text("while (true) {}\n")
        result, report = self.run_measurement(timeout=0.5)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["complete"])
        self.assertIn("timed out", report["error"])

    def test_retained_memory_is_reported_only_when_requested(self):
        workload = self.root / "workload.mjs"
        workload.write_text(workload.read_text().replace(
            "oracle: sum,", "retained: new Uint8Array(4096), oracle: sum,"))
        result, report = self.run_measurement()
        self.assertEqual(result.returncode, 0, result.stderr)
        for sample in report["workloads"][0]["samples"]:
            self.assertIsInstance(sample["metrics"]["retained_heap_bytes"], int)
            self.assertIsInstance(sample["metrics"]["retained_array_buffer_bytes"], int)

    def test_nonfinite_oracle_fails_before_workers(self):
        self.manifest["workloads"][0]["expected"] = float("nan")
        result, report = self.run_measurement()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["complete"])
        self.assertIn("non-finite JSON number", report["error"])
        self.assertNotIn("workloads", report)


if __name__ == "__main__":
    unittest.main()
