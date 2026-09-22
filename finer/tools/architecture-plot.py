"""Render the recorded experiment with matplotlib; no measurements happen here."""
import json
import statistics
import sys
from pathlib import Path
import matplotlib.pyplot as plt

source, output = map(Path, sys.argv[1:])
report = json.loads(source.read_text())
assert report["status"] == "passed"
workload = next(w for w in report["workloads"] if w["name"] == "opaque-value-chains-128-32")
modes = ["tree", "indexed", "memoized"]
labels = ["Recompute", "Precompute", "Cache on demand"]
fig, (timing, size) = plt.subplots(1, 2, figsize=(12, 5), gridspec_kw={"width_ratios": [1.4, 1]})
left = [0.0] * 3
for stage, label, color in [
    ("parse", "Parse", "#8ecae6"), ("semantic", "Check types / bindings", "#64748b"),
    ("lower", "Lower", "#a7c957"), ("optimize", "Optimize", "#2563eb"),
    ("legalize", "Choose target forms", "#a78bfa"),
    ("render", "Name / verify / print", "#f4a261"),
]:
    values = [statistics.median(c["metrics"]["milliseconds"].get(stage, 0) for c in workload["compiles"] if c["mode"] == mode) for mode in modes]
    timing.barh(labels, values, left=left, label=label, color=color)
    left = [a+b for a, b in zip(left, values)]
timing.invert_yaxis()
timing.set_xlabel("Milliseconds (sum of stage medians)")
timing.set_title("Same optimized program; different fact reuse")
timing.legend(loc="lower center", bbox_to_anchor=(0.5, -0.47), ncol=2, frameon=False, fontsize=9)
artifacts = {a["name"]: a for a in workload["artifacts"]}
names = ["Experimental compiler", "Terser · 1 pass", "Terser · 3 passes"]
values = [artifacts[name]["brotli11"] for name in ["indexed", "terser-1", "terser-3"]]
bars = size.barh(names, values, color=["#2563eb", "#64748b", "#64748b"])
size.bar_label(bars, padding=5)
size.set_xlim(0, max(values)*1.2)
size.invert_yaxis()
size.set_xlabel("Canonical Brotli 11 bytes · smaller is better")
size.set_title("Compression work remains")
for ax in [timing, size]:
    ax.spines[["top", "right"]].set_visible(False)
    ax.grid(axis="x", alpha=.15)
    ax.set_axisbelow(True)
fig.suptitle("Analysis experiment: 128 functions × 32 values, opaque input", fontsize=14, x=.05, ha="left")
fig.text(.05, .04, f'{report["trials"]} trials per mode on one pinned worker core. This is one synthetic case, not fleet acceptance.\nTerser {report["terserVersion"]} receives the unoptimized experimental JavaScript; every displayed artifact passes its behavior check.', fontsize=9, color="#475569")
fig.subplots_adjust(left=.13, right=.97, bottom=.34, top=.78, wspace=.68)
fig.savefig(output, metadata={"Description": "Derived from " + str(source)})
fig.savefig(output.with_suffix(".png"), dpi=160)
