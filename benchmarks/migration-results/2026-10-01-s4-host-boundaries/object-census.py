# Read-only census; the compiler lexer ignores comments and string contents.
from pathlib import Path
import subprocess, json, hashlib
folder = Path(__file__).resolve().parent
root = folder.parents[2]
sha = lambda value: hashlib.sha256(value).hexdigest()
deps = root / "target/release/deps"
library = max(deps.glob("liblilscript-*.rlib"), key=lambda p: p.stat().st_mtime)
scanner = Path("/tmp/lilscript-s4-object-census")
subprocess.run(["rustc", "--edition=2021", str(folder / "object-census.rs"),
    "--extern", "lilscript=" + str(library), "-L", "dependency=" + str(deps),
    "-o", str(scanner)], check=True)
rows = []
for entry in json.loads((root / "benchmarks/libraries/maintained-workloads.json").read_text())["libraries"]:
    paths = [Path(entry["workspace"]), Path("/home/azureuser") / entry["id"]]
    workspace = next(path for path in paths if path.is_dir())
    files = sorted(subprocess.check_output(["rg", "--files", "-g", "*.lil",
        "-g", "!node_modules", "-g", "!dist", "-g", "!_site"], cwd=workspace, text=True).splitlines())
    positions = subprocess.check_output([str(scanner)], text=True,
        input="".join(str(workspace / path) + "\n" for path in files)).splitlines()
    assert len(positions) == len(files)
    sources = [{"path":path, "sha256":sha((workspace/path).read_bytes()),
                "singleton_byte_offsets":json.loads(found)} for path,found in zip(files,positions)]
    rows.append({"port":entry["id"], "workspace":str(workspace),
        "head":subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace,text=True).strip(),
        "source_files":sources, "declarations":sum(len(f["singleton_byte_offsets"]) for f in sources)})
report={"scope":"27 maintained workloads; .lil sources excluding dependencies/built output; same token predicate as ParserCore::looks_like_object_declaration",
    "lexer_sha256":sha((root/"src/lexer.rs").read_bytes()),
    "scanner_sha256":sha((folder/"object-census.rs").read_bytes()),
    "scanner_library_sha256":sha(library.read_bytes()),
    "ports":rows, "total_files":sum(len(r["source_files"]) for r in rows),
    "declarations":sum(r["declarations"] for r in rows)}
(folder/"object-census.json").write_text(json.dumps(report,indent=2)+"\n")
print({key:report[key] for key in ["total_files", "declarations"]})
