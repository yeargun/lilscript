import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


cal = module("calibrate", ROOT / "scripts/calibrate.py")
gen = module("generate", ROOT / "benchmarks/calibration/generate.py")


class CalibrationTests(unittest.TestCase):
    def test_frozen_split_refuses_source_drift_and_family_leakage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "corpus"
            gen.generate(root)
            manifest = json.loads((root / "manifest.json").read_text())
            cal.validate_manifest(manifest, root)
            mutated = copy.deepcopy(manifest)
            mutated["workloads"][-1]["family"] = mutated["workloads"][0]["family"]
            with self.assertRaisesRegex(ValueError, "disjoint"):
                cal.validate_manifest(mutated, root)
            source = root / manifest["workloads"][0]["entry"]
            source.write_text(source.read_text()+"\n")
            with self.assertRaisesRegex(ValueError, "content changed"):
                cal.validate_manifest(manifest, root)

    def test_evaluation_rejects_unqualified_or_changed_training(self):
        training = dict(complete=True, split="training", identity={"compiler": "pinned"},
                        manifest_sha256="frozen", production_policy_changed=False,
                        summary={"raw": {}}, rows=[dict(split="training")])
        cal.validate_training(training, training["identity"], "frozen")
        for key, value in [("complete", False), ("split", "evaluation"),
                           ("identity", {"compiler": "different"}), ("manifest_sha256", "changed"),
                           ("production_policy_changed", True), ("rows", [dict(split="evaluation")])]:
            with self.assertRaises(ValueError, msg=key):
                cal.validate_training({**training, key: value}, training["identity"], "frozen")

    def test_folds_do_not_select_using_the_validation_family(self):
        def row(family, on, off, missed):
            modes = {}
            for mode in cal.MODES:
                modes[mode] = {key: 0 for key in ["judged", "pruned", "codec_probes", "baseline_work",
                                                 "optional_work", "codec_work", "peak_retained_bytes", "stopped"]}
                modes[mode].update(bytes=off if mode == "off" else on, missed=missed if mode == "audit" else [])
            return dict(id=family, family=family, objective="brotli", margin=150, modes=modes)
        rows = [row("a", 100, 90, [dict(proxy=200, audit=-1)]), row("b", 100, 110, [])]
        result = cal.summarize(rows)["brotli"]
        folds = {fold["held_out_family"]: fold for fold in result["leave_one_family_out"]}
        self.assertEqual(folds["a"]["selected_mode"], "on")
        self.assertEqual(folds["a"]["estimated_minimum_margin"], 150)
        self.assertEqual(folds["a"]["unseen_immediate_misses"], 1)
        self.assertEqual(folds["b"]["selected_mode"], "off")
        self.assertEqual(folds["b"]["validation_delta"], 10)
        self.assertEqual(cal.summarize(rows, cross_validate=False)["brotli"]["leave_one_family_out"], [])

    def test_hidden_policy_adapters_cannot_contaminate_lanes(self):
        with patch.dict(os.environ, {"LILSCRIPT_TIMING": "1", "LILSCRIPT_WALK_AUDIT": "1",
                                     "NODE_OPTIONS": "--require=surprise", "RAYON_NUM_THREADS": "8"}):
            env = cal.environment()
            self.assertFalse(any(name.startswith("LILSCRIPT_") for name in env))
            self.assertNotIn("NODE_OPTIONS", env)
            self.assertEqual(env["RAYON_NUM_THREADS"], "1")


if __name__ == "__main__":
    unittest.main()
