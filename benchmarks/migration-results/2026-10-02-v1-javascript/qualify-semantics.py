#!/usr/bin/env python3
"""Serial JS qualification against independent oracles and existing native goldens.

Usage: qualify-semantics.py COMPILER OUTPUT_DIR [CASE ...]
Formation (effort 0, optional folding/inlining/scalars off) and effort 13 each
select raw, gzip and Brotli independently. No generated oracle is accepted.
"""
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[3]
compiler = Path(sys.argv[1]).resolve()
work = Path(sys.argv[2]).resolve()
work.mkdir(parents=True, exist_ok=True)
cases = sys.argv[3:] or ["products", "arrays", "call-transport", "callable-containers", "static-data", "generators", "generator-call-defaults"]
rows = []
sources = []
launcher = work / "run.mjs"
launcher.write_text("""import {pathToFileURL} from 'node:url';
for(const name of ['collectNative','assertNativeCount','assertMaxNativeCount','observeNativeStartup'])globalThis[name]=()=>{};
await import(pathToFileURL(process.argv[2]));
""")

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run(name, command, expected=None):
    start = time.monotonic()
    result = subprocess.run(list(map(str, command)), cwd=root, capture_output=True, timeout=180)
    (work / (name + ".stdout")).write_bytes(result.stdout)
    (work / (name + ".stderr")).write_bytes(result.stderr)
    passed = result.returncode == 0 and (expected is None or result.stdout == expected)
    rows.append(dict(name=name, command=list(map(str, command)), seconds=time.monotonic()-start,
                     exit=result.returncode, passed=passed,
                     stdout_sha256=hashlib.sha256(result.stdout).hexdigest()))
    print(name, "pass" if passed else "FAIL", flush=True)
    return passed

try:
    for name in cases:
        folder = root / "tests" / ("cases" if name == "generator-call-defaults" else "native")
        source = folder / (name + ".lil")
        expected = (folder / (name + ".out")).read_bytes()
        oracle = folder / (name + ".oracle.mjs")
        sources.append(dict(case=name, source=str(source.relative_to(root)), sha256=digest(source),
                            oracle_sha256=digest(oracle), expected_sha256=hashlib.sha256(expected).hexdigest()))
        run(name + "-oracle", ["node", oracle], expected)
        original_config = (folder / (name + ".toml"))
        unified = original_config.exists() and 'absence="unified"' in original_config.read_text()
        for effort in [0, 13]:
            for objective in ["raw", "gzip", "brotli"]:
                lane = f"{name}-{effort}-{objective}"
                config = work / (lane + ".toml")
                config.write_text(f'effort.level={effort}\nobjective.codecs="{objective}"\n'
                    + ('[language]\nabsence="unified"\n' if unified else '')
                    + ('[policy.tactics]\nconstant-folding="off"\ninlining="off"\nscalar-replacement="off"\n' if effort == 0 else ''))
                output = work / (lane + ".mjs")
                if run(lane + "-compile", [compiler, source, "--config", config, "--target", "js-module", "-o", output]):
                    run(lane + "-runtime", ["node", launcher, output], expected)
finally:
    (work / "qualification.json").write_text(json.dumps(dict(schema=1,
        compiler=dict(path=str(compiler), sha256=digest(compiler)), sources=sources, rows=rows), indent=2) + "\n")
sys.exit(0 if all(row["passed"] for row in rows) else 1)
