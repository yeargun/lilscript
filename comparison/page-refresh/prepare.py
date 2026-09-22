#!/usr/bin/env python3
"""Create isolated, committed inputs and preserve the currently served Pages evidence."""
import concurrent.futures
import hashlib
import json
import pathlib
import subprocess
import sys
import urllib.error
import urllib.request

RUN = pathlib.Path(sys.argv[1]).resolve()
RUN.mkdir(parents=True, exist_ok=True)


def command(args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True, stderr=subprocess.PIPE).strip()


def download(url, target):
    try:
        request = urllib.request.Request(url, headers={"User-Agent": "LilScript-Pages-audit"})
        with urllib.request.urlopen(request, timeout=45) as response:
            content = response.read()
            target.write_bytes(content)
            return {"url": url, "status": response.status, "sha256": hashlib.sha256(content).hexdigest(),
                    "lastModified": response.headers.get("Last-Modified")}
    except Exception as error:
        return {"url": url, "error": str(error)}


def prepare(row):
    name = row["name"]
    root = RUN / "repos" / name
    root.parent.mkdir(exist_ok=True)
    remote = row.get("remote", f"https://github.com/yeargun/{name}.git")
    if not root.exists():
        command(["git", "clone", "--quiet", "--shared", row["path"], str(root)] if row.get("path") else
                ["git", "clone", "--quiet", remote, str(root)])
        command(["git", "remote", "set-url", "origin", remote], root)
    command(["git", "fetch", "--quiet", "origin", "main"], root)
    local = row.get("commit", command(["git", "rev-parse", "HEAD"], root))
    public = command(["git", "rev-parse", "origin/main"], root)
    is_ancestor = lambda a, b: subprocess.run(["git", "merge-base", "--is-ancestor", a, b], cwd=root).returncode == 0
    if is_ancestor(public, local):
        base = local
    elif is_ancestor(local, public):
        base = public
    else:
        raise RuntimeError(f"{name}: local/public history diverged; needs explicit reconciliation")
    command(["git", "checkout", "--quiet", "-B", "refresh-pages-20260910", base], root)
    evidence = RUN / "before" / name
    evidence.mkdir(parents=True, exist_ok=True)
    package = json.loads((root / "package.json").read_text())
    page = f"https://yeargun.github.io/{name}/"
    served = [download(page, evidence / "index.html"), download(page + "results.json", evidence / "results.json")]
    try:
        pages = json.loads(command(["gh", "api", f"repos/yeargun/{name}/pages"]))
        pages = {key: pages.get(key) for key in ["html_url", "status", "build_type", "source"]}
    except Exception as error:
        pages = {"error": str(error)}
    row.update({"snapshot": str(root), "baseCommit": base, "publicCommit": public,
                "unpublishedCommits": int(command(["git", "rev-list", "--count", f"{public}..{base}"], root)),
                "package": package, "served": served, "pages": pages})
    print(name, base[:10], "public=" + public[:10], "page=" + str(served[0].get("status", served[0].get("error"))), flush=True)
    return row


rows = json.loads((RUN / "inventory.json").read_text())
rows = [row for row in rows if row["name"] != "lil-solidjs"]
rows.append({"name": "vuelil"})
results = []
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
    futures = {executor.submit(prepare, row): row["name"] for row in rows}
    for future in concurrent.futures.as_completed(futures):
        try:
            results.append(future.result())
        except Exception as error:
            results.append({"name": futures[future], "preparationError": str(error)})
            print(futures[future], "ERROR", str(error), flush=True)
(RUN / "prepared.json").write_text(json.dumps(sorted(results, key=lambda row: row["name"]), indent=2) + "\n")
