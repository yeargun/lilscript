"""Matched-task checks for the retained compilation runner."""
import copy
from pathlib import Path
import runpy
import unittest

compare = runpy.run_path(str(Path(__file__).with_name("compile-cost.py")))["compare_policies"]


class PolicyComparison(unittest.TestCase):
    def setUp(self):
        self.before = {"javascript_policy": {
            "contract": {"execution": "module"}, "effort": 13,
            "resources": {}, "constraints": {},
            "objective": {"codec": "Brotli", "codec_settings": {"quality": 11},
                          "priority": "size-first", "walk": {"version": 2}},
            "tactics": [],
        }, "native_policy": None, "request": {
            "logical_work": 200_000_000, "effective_logical_work": 200_000_000,
            "retained_bytes": 256_000_000, "effective_retained_bytes": 256_000_000,
        }}
        self.after = copy.deepcopy(self.before)
        self.after["javascript_policy"]["objective"]["walk"]["version"] = 3
        self.after["javascript_policy"]["tactics"] = [{"id": "naming-alphabet", "state": "auto"}]

    def test_unchanged_and_explicitly_explained_strategy_change(self):
        self.assertEqual(compare(self.before, self.before), [])
        with self.assertRaises(RuntimeError):
            compare(self.before, self.after)
        self.assertEqual(compare(self.before, self.after, "new measured alphabet trials"), ["javascript_policy"])

    def test_explanation_cannot_relax_the_program_objective_or_resources(self):
        for key, value in [("contract", {}), ("effort", 0), ("resources", {"logical_work": 1}), ("constraints", {"startup": 1}), ("objective", None)]:
            altered = copy.deepcopy(self.after)
            altered["javascript_policy"][key] = value
            with self.assertRaises(RuntimeError, msg=key):
                compare(self.before, altered, "not permission to compare a different task")
        for key in ["codec", "codec_settings", "priority"]:
            altered = copy.deepcopy(self.after)
            altered["javascript_policy"]["objective"][key] = None
            with self.assertRaises(RuntimeError, msg=key):
                compare(self.before, altered, "different objective must fail")

    def test_missing_or_changed_target_reports_are_rejected(self):
        for altered in [{}, {"javascript_policy": None, "native_policy": None}]:
            with self.assertRaises(RuntimeError):
                compare(self.before, altered, "cannot hide a missing target")

    def test_effective_ceilings_can_move_into_policy_identity(self):
        after = copy.deepcopy(self.after)
        after["javascript_policy"]["resources"] = {
            "logical_work": 200_000_000, "retained_bytes": 256_000_000,
            "wall_time_ms": None,
        }
        self.assertEqual(compare(self.before, after, "effective ceilings now fingerprinted"), ["javascript_policy"])
        # A roomier caller cannot loosen the configured hard ceiling.
        after["request"]["logical_work"] *= 2
        self.assertEqual(compare(self.before, after, "same effective ceiling"), ["javascript_policy"])
        after["javascript_policy"]["resources"]["wall_time_ms"] = 1000
        with self.assertRaises(RuntimeError):
            compare(self.before, after, "deadlines still constrain the task")

    def test_old_equal_policy_objects_cannot_hide_different_service_limits(self):
        after = copy.deepcopy(self.before)
        after["request"]["logical_work"] -= 1
        after["request"]["effective_logical_work"] -= 1
        with self.assertRaises(RuntimeError):
            compare(self.before, after)

    def test_missing_or_inconsistent_resource_evidence_is_rejected(self):
        for request in [None, {}, {**self.before["request"], "effective_retained_bytes": 1}]:
            after = copy.deepcopy(self.before)
            after["request"] = request
            with self.assertRaises(RuntimeError):
                compare(self.before, after, "a reason cannot repair inconsistent evidence")


if __name__ == "__main__":
    unittest.main()
