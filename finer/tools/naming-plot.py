"""Plot measured complete-artifact quality against total compilation time."""
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
assert report["status"] == "passed-paired-naming-experiment"
cases = ["opaque-value-chains-128-32", "marked-bracket-scanner", "generic-callback", "typed-string-observations"]
titles = ["Largest opaque chain", "Marked bracket scanner", "Generic callback", "Opaque typed string"]
fig, axes = plt.subplots(2, 2, figsize=(11, 7.1), constrained_layout=True)
for ax, case, title in zip(axes.flat, cases, titles):
    row = next(row for row in report["workloads"] if row["case"] == case)
    modes = row["modes"]
    for mode, color, marker, label in [
        ("previous", "#9b6b43", "x", "Previous"),
        ("global", "#315ea8", "o", "New, global names"),
        ("scoped", "#19826e", "s", "New, scoped names"),
    ]:
        ax.scatter(modes[mode]["medianMilliseconds"]["total"], modes[mode]["artifact"]["brotli11"],
                   color=color, marker=marker, s=42, label=label, zorder=4)
    points = []
    for budget in report["budgets"]:
        mode = modes[f"budget-{budget}"]
        x = mode["medianMilliseconds"]["total"]
        y = mode["winners"]["brotli11"]["sizes"]["brotli11"]
        points.append((x, y))
        ax.annotate(str(budget), (x, y), xytext=(3, 5 if budget != 16 else -13),
                    textcoords="offset points", fontsize=8, color="#8b4386")
    ax.plot(*zip(*points), color="#8b4386", marker=".", label="Search: proposal budget")
    ax.axhline(row["bestMeasuredTerser"]["brotli11"], color="#6b7280", linestyle="--", linewidth=1,
               label="Measured Terser minimum")
    ax.set_xscale("log")
    ax.set_title(title, loc="left", fontsize=12)
    ax.set_xlabel("Median total compilation (ms, log scale)")
    ax.set_ylabel("Complete artifact, Brotli bytes")
    ax.grid(axis="y", alpha=.18)
    ax.spines[["top", "right"]].set_visible(False)
handles, labels = axes.flat[0].get_legend_handles_labels()
fig.legend(handles, labels, loc="outside lower center", ncol=3, frameon=False, fontsize=9)
fig.suptitle("Shared naming: compression improves at a measurable compilation cost", fontsize=14)
for extension in ["svg", "png"]:
    path = args.output.with_suffix(f".{extension}")
    assert not path.exists(), "plot output must be new"
    fig.savefig(path, dpi=170, facecolor="white")
plt.close(fig)
