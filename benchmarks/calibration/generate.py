#!/usr/bin/env python3
"""Deterministic generic modules, with independently computed numeric oracles.

The reference evaluates data/formulas in Python, never generated LilScript or
compiler output. Evaluation libraries are disjoint from training templates.
Generation freezes both splits before any compiler result is inspected.
"""
import argparse
import binascii
import hashlib
import json
from pathlib import Path
import random

INPUTS = [-2147483648, -1234567, -1, 0, 1, 31, 123456789, 2147483647]
FAMILIES = ["arithmetic", "branches", "strings", "records"]


def int32(value):
    return (value + 2**31) % 2**32 - 2**31


def units(value):
    encoded = value.encode("utf-16-le")
    return [int.from_bytes(encoded[i:i+2], "little") for i in range(0, len(encoded), 2)]


def training(family, count, seed):
    rng = random.Random(seed)
    functions, descriptions = [], []
    for index in range(count):
        a, b, c = [rng.randrange(1, 2**20) for _ in range(3)]
        shift = rng.randrange(1, 31)
        strings = [f'{rng.choice(["attribute", "element", "namespace", "literal"])}_{index}_{tag}_"quoted"_\\path_λ😀'
                   for tag in ["left", "right", "above", "below"]]
        descriptions.append((a, b, c, shift, strings))
        if family == "arithmetic":
            body = (f"int mixed=(value^{a})+{b};"
                    f"return ((mixed<<{shift})|(mixed>>>{32-shift}))^{c};")
        elif family == "branches":
            body = "int total=value;"
            for branch in range(8):
                body += ("if" if branch == 0 else "else if") + f"((value&7)=={branch}){{total+={a+branch*b};}}"
            body += f"if(total<0){{return total^{b};}}return total+{c};"
        elif family == "strings":
            literal = [json.dumps(value, ensure_ascii=False) for value in strings]
            body = f"string text={literal[3]};"
            for branch in range(3):
                body += ("if" if branch == 0 else "else if") + f"((value&3)=={branch}){{text={literal[branch]};}}"
            body += "return text.length+text.codeUnitAt(value&7);"
        elif family == "records":
            body = (f"Record<int> item=record{{count:value,offset:{a},mask:{b}}};"
                    "int saved=item.count??0;item.count=saved+(item.offset??0);"
                    f"return (item.count??0)^((item.mask??0)+{c})^saved;")
        else:
            raise ValueError(family)
        functions.append(f"int operation{index}(int value){{{body}}}\n")
    calls = "".join(f"result^=operation{i}(input+{i});" for i in range(count))
    source = "".join(functions) + f"export int probe(int input){{int result=0;{calls}return result;}}\n"

    def reference(input_value):
        results = []
        for index, (a, b, c, shift, strings) in enumerate(descriptions):
            value = int32(input_value + index)
            if family == "arithmetic":
                mixed = int32((value ^ a) + b) & 0xffffffff
                result = int32(((mixed << shift) | (mixed >> (32-shift))) ^ c)
            elif family == "branches":
                total = int32(value + a + (value & 7)*b)
                result = int32(total ^ b if total < 0 else total + c)
            elif family == "strings":
                text = units(strings[value & 3])
                result = len(text) + text[value & 7]
            else:
                result = int32(int32(value+a) ^ int32(b+c) ^ value)
            results.append(result)
        accumulator = 0
        for result in results:
            accumulator ^= result
        return int32(accumulator)

    return source, [reference(value) for value in INPUTS]


def evaluation(name):
    if name == "checksum-library":
        # Table lookup implementation; Python's separately maintained native
        # zlib implementation is the reference, not this recurrence.
        table = []
        for byte in range(256):
            value = byte
            for _ in range(8):
                value = (value >> 1) ^ (0xedb88320 if value & 1 else 0)
            table.append(int32(value))
        source = "Array<int> table=[" + ",".join(map(str, table)) + "];\n"
        source += ("export int checksum(Array<int> bytes){int value=-1;"
                   "for(int i=0;i<bytes.length;i++){value=table[(value^bytes[i])&255]^(value>>>8);}return value^-1;}\n"
                   "export int probe(int input){Array<int> bytes=[];for(int i=0;i<257;i++){bytes.push((input+i*31)&255);}return checksum(bytes);}\n")
        expected = [int32(binascii.crc32(bytes((value+i*31) & 255 for i in range(257)))) for value in INPUTS]
    elif name == "distance-library":
        # Linear-space dynamic program versus the reference's full matrix.
        source = ("export int distance(string a,string b){Array<int> row=[];"
                  "for(int j=0;j<=b.length;j++){row.push(j);}"
                  "for(int i=1;i<=a.length;i++){int diagonal=row[0];row[0]=i;"
                  "for(int j=1;j<=b.length;j++){int prior=row[j];"
                  "int replace=diagonal;if(a.codeUnitAt(i-1)!=b.codeUnitAt(j-1)){replace++;}"
                  "int insert=row[j-1]+1;int erase=prior+1;"
                  "int best=replace;if(insert<best){best=insert;}if(erase<best){best=erase;}row[j]=best;"
                  "diagonal=prior;}}return row[b.length];}\n"
                  'export int probe(int input){string a="compiler😀";if((input&1)!=0){a="compressλ";}'
                  'string b="compression😀";if((input&2)!=0){b="compilationλ";}return distance(a,b);}\n')
        def reference(value):
            a = units("compiler😀" if value & 1 == 0 else "compressλ")
            b = units("compression😀" if value & 2 == 0 else "compilationλ")
            matrix = [[0]*(len(b)+1) for _ in range(len(a)+1)]
            for i in range(len(a)+1):
                matrix[i][0] = i
            for j in range(len(b)+1):
                matrix[0][j] = j
            for i, x in enumerate(a, 1):
                for j, y in enumerate(b, 1):
                    matrix[i][j] = min(matrix[i-1][j]+1, matrix[i][j-1]+1, matrix[i-1][j-1]+(x != y))
            return matrix[-1][-1]
        expected = list(map(reference, INPUTS))
    elif name == "interval-library":
        # Binary search over sorted disjoint intervals versus linear reference.
        starts = [i*23-6000 for i in range(521)]
        ends = [start + 3 + i % 17 for i, start in enumerate(starts)]
        source = "Array<int> starts=[" + ",".join(map(str, starts)) + "];\n"
        source += "Array<int> ends=[" + ",".join(map(str, ends)) + "];\n"
        source += ("export int locate(int point){int lo=0;int hi=starts.length;"
                   "while(lo<hi){int mid=(lo+hi)>>>1;if(starts[mid]<=point){lo=mid+1;}else{hi=mid;}}"
                   "int index=lo-1;if(index>=0&&point<=ends[index]){return index;}return -1;}\n"
                   "export int probe(int input){int total=0;for(int i=0;i<97;i++){total+=locate(((input+i*17)&16383)-8192);}return total;}\n")
        expected = []
        for value in INPUTS:
            points = [((value+i*17) & 16383)-8192 for i in range(97)]
            expected.append(sum(next((i for i, (lo, hi) in enumerate(zip(starts, ends)) if lo <= p <= hi), -1) for p in points))
    else:
        raise ValueError(name)
    return source, expected


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(output):
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    cases = [(f"{family}-{count}", "training", family, training(family, count, 0x43414c+index*101+count))
             for index, family in enumerate(FAMILIES) for count in [128, 384]]
    cases += [(name, "evaluation", name, evaluation(name))
              for name in ["checksum-library", "distance-library", "interval-library"]]
    for name, split, family, (source, expected) in cases:
        entry = output / f"{name}.lil"
        entry.write_text(source)
        rows.append(dict(id=name, split=split, family=family, entry=entry.name,
                         source_sha256=digest(entry), source_bytes=entry.stat().st_size,
                         inputs=INPUTS, expected=expected,
                         exposure="Authored generator/formulas; no compiler measurements before the frozen split. "
                         + ("Available for tuning." if split == "training" else
                            "Protected from parameter selection; not blind to the author. Failure inspection requires promotion and replacement.")))
    manifest = dict(schema=1, generator_sha256=digest(Path(__file__)), workloads=rows,
                    prior_exposure=dict(development="All previously measured ports and 642 ratchet programs, including their derivatives.",
                                        monacolil="Previously inspected scoreboards and idiom census; not blind.",
                                        solidlil="Previously inspected scoreboards and idiom census; not blind.",
                                        syntax_repair="Initial training1 measured arithmetic and branch families. String and distance fixtures used unsupported conditional-expression syntax; repaired before any evaluation scores. Expected values and all other sources unchanged. No heuristic tuning."))
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    generate(parser.parse_args().output.resolve())
