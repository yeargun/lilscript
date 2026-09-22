"""Render the preserved source-binding experiment; no measurements happen here."""
import json
import sys
from pathlib import Path
import matplotlib.pyplot as plt

source, output = map(Path, sys.argv[1:])
report = json.loads(source.read_text())
assert report["status"] == "passed"
row = next(row for row in report["measurements"] if row["case"] == "opaque-value-chains-128-32")
versions = ["initial-analysis", "bounded", "binding-index"]
labels = ["Earlier measured compiler", "Bounded compiler, before registry", "Compiler with binding registry"]
fig, ax = plt.subplots(figsize=(11, 5))
left = [0.0] * len(versions)
for stage, label, color in [
    ("parse", "Parse", "#8ecae6"), ("semantic", "Check types / bindings", "#64748b"),
    ("lower", "Lower", "#a7c957"), ("optimize", "Optimize", "#2563eb"),
    ("legalize", "Choose target forms", "#a78bfa"), ("render", "Name / verify / print", "#f4a261"),
]:
    values = [row["versions"][version]["indexed"].get(stage, 0) for version in versions]
    ax.barh(labels, values, left=left, label=label, color=color)
    left = [a+b for a, b in zip(left, values)]
for index, (version, length) in enumerate(zip(versions, left)):
    total = row["versions"][version]["indexed"]["total"]
    ax.text(length+.4, index, f"{total:.2f} ms total", va="center", fontsize=10)
ax.invert_yaxis()
ax.set_xlim(0, max(left)*1.24)
ax.set_xlabel("Milliseconds · bars sum stage medians; labels give median whole compilation")
ax.set_title("Retain binding knowledge instead of scanning all identifiers per declaration", loc="left", pad=22)
ax.spines[["top", "right"]].set_visible(False)
ax.grid(axis="x", alpha=.15)
ax.set_axisbelow(True)
ax.legend(loc="lower center", bbox_to_anchor=(.5, -.48), ncol=3, frameon=False, fontsize=9)
fig.text(.03, .045, "128 functions × 32 values, opaque input. 20 alternating trials per version and mode on one pinned worker core.\nOutput is identical before and after the registry change. This synthetic result does not establish fleet speed or compression wins.", fontsize=9, color="#475569")
fig.subplots_adjust(left=.3, right=.97, bottom=.35, top=.84)
fig.savefig(output, metadata={"Description": "Derived from " + str(source)})
fig.savefig(output.with_suffix(".png"), dpi=160)
