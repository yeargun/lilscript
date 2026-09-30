#!/usr/bin/env python3
"""Independent closure/capture oracles across identifier widths and spellings.

This is a development corpus, not held-out calibration data. Compare the same
compiler with alphabet trials permitted and vetoed under each exact objective.
All source/configuration text, output hashes and sizes are retained in JSON.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(width, variant):
    names = [[f"value{i}", f"record_counter_{i:03}", f"z{width-i}"][variant]
             for i in range(width)]
    declarations = "".join(f"int {name}=input({i})+seed;" for i, name in enumerate(names))
    expression = "+".join(f"{name}*(shift+{i+1})" for i, name in enumerate(names))
    return ("extern int input(int slot);export func(int)->int make(int seed){"
            + declarations + "return (int shift)=>" + expression + ";}")


def execute(path, width):
    # The oracle uses arrays and an explicit wrapped reduction; it does not
    # compile or evaluate the generated source as its reference implementation.
    script = """
const events=[];
globalThis.input=slot=>{events.push(slot);return (slot*97)^31};
const library=await import(process.argv[1]);
const first=library.make(17),second=library.make(-3);
const actual=[first(3),second(-7),first(2147483647)];
const width=Number(process.argv[2]);
const reference=(seed,shift)=>Array.from({length:width},(_,i)=>(((i*97)^31)+seed)|0)
 .reduce((total,value,i)=>(total+Math.imul(value,(shift+i+1)|0))|0,0);
const expected=[reference(17,3),reference(-3,-7),reference(17,2147483647)];
const expectedEvents=Array.from({length:2*width},(_,i)=>i%width);
if(JSON.stringify(actual)!==JSON.stringify(expected)||
   JSON.stringify(events)!==JSON.stringify(expectedEvents)||library.make.length!==1)
 throw Error(JSON.stringify({actual,expected,events,expectedEvents,length:library.make.length}));
console.log(JSON.stringify({values:actual,events,length:library.make.length}));
"""
    result = subprocess.run(["node", "--input-type=module", "-e", script,
                             path.as_uri(), str(width)], check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--codec", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--json", type=Path, required=True)
    args = parser.parse_args()
    compiler, codec = args.compiler.resolve(), args.codec.resolve()
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "exposure": "development",
              "compiler": {"path": str(compiler), "sha256": digest(compiler)},
              "codec": {"path": str(codec), "sha256": digest(codec)},
              "rows": [], "complete": False}
    try:
        for width in [12, 56, 80]:
            for variant in range(3):
                source = fixture(width, variant)
                entry = work / f"capture-{width}-{variant}.lil"
                entry.write_text(source)
                for objective in ["raw", "gzip", "brotli"]:
                    row = {"width": width, "variant": variant, "source": source,
                           "objective": objective, "artifacts": {}}
                    report["rows"].append(row)
                    outputs = []
                    for permission in ["off", "on"]:
                        stem = f"{entry.stem}-{objective}-{permission}"
                        output, config = work / f"{stem}.mjs", work / f"{stem}.toml"
                        text = (f'objective.codecs="{objective}"\n[effort]\nlevel=13\n'
                                f'[policy.tactics]\nnaming-alphabet="{permission}"\n')
                        config.write_text(text)
                        command = [str(compiler), str(entry), "--config", str(config),
                                   "--target", "js-module", "-o", str(output)]
                        result = subprocess.run(command, capture_output=True, text=True, timeout=180)
                        if result.returncode:
                            raise RuntimeError(result.stderr)
                        observation = execute(output, width)
                        row["artifacts"][permission] = {"sha256": digest(output), "config": text,
                                                        "observation": observation}
                        outputs.append(str(output))
                    result = subprocess.run([str(codec), "--json", *outputs], check=True,
                                            capture_output=True, text=True, timeout=180)
                    row["measurements"] = json.loads(result.stdout)
        report["complete"] = True
    finally:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
