"""Plot measured objective-specific search cost with complete-artifact sizes."""
import argparse
import json
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("summary", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
report = json.loads(args.summary.read_text())
assert report["status"] == "passed-paired-naming-experiment" and report["objectiveStudy"]
cases = ["opaque-value-chains-128-32", "marked-bracket-scanner", "generic-callback", "captured-value-writer"]
titles = ["Largest opaque chain", "Marked bracket scanner", "Generic callback", "Captured writer: Brotli costs more"]
metrics = [("raw", "raw", "#315ea8"), ("gzip", "gzip9", "#19826e"), ("brotli", "brotli11", "#8b4386")]
fig, axes = plt.subplots(2, 2, figsize=(11, 7), constrained_layout=True)
for ax, case, title in zip(axes.flat, cases, titles):
    row = next(row for row in report["workloads"] if row["case"] == case)
    all_goals = row["modes"]["budget-16"]
    baseline = all_goals["medianMilliseconds"]["total"]
    labels, xs = [], [baseline]
    for y, (goal, metric, color) in zip([2, 1, 0], metrics):
        selected = row["modes"][f"{goal}-16"]
        elapsed = selected["medianMilliseconds"]["total"]
        size = selected["winners"][metric]["sizes"][metric]
        assert size == all_goals["winners"][metric]["sizes"][metric]
        xs.append(elapsed)
        labels.append(f"{goal.capitalize()}\n{size:,} bytes")
        ax.plot([baseline, elapsed], [y, y], color=color, linewidth=2, alpha=.65)
        ax.scatter(baseline, y, color="#6b7280", marker="x", s=40, zorder=4)
        ax.scatter(elapsed, y, color=color, s=42, zorder=5)
        ax.annotate(f"{elapsed:.3f} ms", (elapsed, y), xytext=(0, 10),
                    textcoords="offset points", ha="center", fontsize=9, color=color)
    ax.axvline(baseline, color="#6b7280", linewidth=.8, linestyle="--", alpha=.4)
    ax.set_title(f"{title}\nAll objectives: {baseline:.3f} ms", loc="left", fontsize=11)
    ax.set_yticks([2, 1, 0], labels, fontsize=9)
    ax.set_ylim(-.45, 2.65)
    ax.set_xscale("log")
    ax.set_xlim(min(xs) * .65, max(xs) * 1.5)
    ax.set_xlabel("Median total compilation (ms, log scale)")
    ax.spines[["top", "right", "left"]].set_visible(False)
    ax.grid(axis="x", alpha=.16)
fig.suptitle("Requesting one score removes unnecessary codec work", fontsize=14)
fig.supxlabel("16 proposal steps · colored dot: selected objective · gray cross: all objectives\n"
              "Selected sizes match in this run; changed exploration can still increase cost.", fontsize=10)
for extension in ["svg", "png"]:
    path = args.output.with_suffix(f".{extension}")
    assert not path.exists(), "plot output must be new"
    fig.savefig(path, dpi=170, facecolor="white")
plt.close(fig)
