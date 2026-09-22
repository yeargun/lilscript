"""Plot recorded paired results; no measurements or compilation happen here."""
import json
import sys
from pathlib import Path
import matplotlib.pyplot as plt

source, output = map(Path, sys.argv[1:])
report = json.loads(source.read_text())
assert report["status"] == "passed-paired-experiment"
by_name = {row["case"]: row for row in report["workloads"]}
selected = [
    ("probe-parity", "Parity (small)"),
    ("shared-value-dependencies-72", "72 shared definitions"),
    ("opaque-value-chains-64-16", "Opaque 64 × 16"),
    ("opaque-value-chains-128-32", "Opaque 128 × 32"),
    ("materialization-chain-1200", "1,200-step chain"),
    ("owned-array-length", "Private array length"),
    ("owned-array-effects", "Effectful private array"),
]
fig, (timing, size) = plt.subplots(1, 2, figsize=(12, 5.6), sharey=True)
for y, (name, _) in enumerate(selected):
    row = by_name[name]
    pair = row["compileComparison"]
    ratio = pair["medianRatio"]
    low, high = pair["pairedBootstrap95"]
    timing.errorbar(ratio, y, xerr=[[ratio-low], [high-ratio]], fmt="o", capsize=4,
                   color="#2563eb", markersize=6)
    current = row["sizes"]["indexed"]["brotli11"]
    baseline = row["bestMeasuredTerser"]["brotli11"]
    ratio = current / baseline
    size.plot(ratio, y, "o", color="#15803d" if ratio < 1 else "#a16207", markersize=7)
    size.annotate(f"{current:,} / {baseline:,}", (ratio, y), xytext=(8, 0),
                  textcoords="offset points", va="center", fontsize=9)
timing.set_yticks(range(len(selected)), [label for _, label in selected])
timing.invert_yaxis()
timing.set_xlim(.77, 1.31)
size.set_xlim(.5, 2.17)
timing.set_title("Compilation: current / previous")
size.set_title("Brotli bytes: current / measured Terser")
timing.set_xlabel("Ratio of medians · paired 95% bootstrap interval")
size.set_xlabel("Exact size ratio · labels show current / reference bytes")
for ax in (timing, size):
    ax.axvline(1, color="#334155", linestyle="--", linewidth=1)
    ax.spines[["top", "right"]].set_visible(False)
    ax.grid(axis="x", alpha=.15)
    ax.set_axisbelow(True)
fig.suptitle("Owned representations improve some cases; costs and losses remain", x=.03, ha="left", fontsize=14)
fig.text(.03, .04,
    "Below 1 is better. Seven selected cases from 16; all 176 labeled artifacts pass execution checks.\n"
    "20 alternating trials per arm. Terser 5.50.0: safe 1/3/5/10-pass recipes. These are microprograms, not fleet acceptance.\n"
    "Largest-case median RSS rises from 18,860 to 19,570 KiB; a smaller live program does not establish lower peak memory.",
    fontsize=9, color="#475569")
fig.subplots_adjust(left=.18, right=.97, bottom=.26, top=.85, wspace=.16)
fig.savefig(output, metadata={"Description": "Derived from " + str(source)})
fig.savefig(output.with_suffix(".png"), dpi=160)
