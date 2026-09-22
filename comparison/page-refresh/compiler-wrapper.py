#!/usr/bin/env python3
"""Time real compiler invocations; never satisfy a build from an existing artifact."""
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import time

args = sys.argv[1:]
started = time.time()
result = subprocess.run([os.environ["PAGE_AUDIT_REAL_COMPILER"], *args])
record = {"args": args, "startedAt": started, "wallSeconds": time.time() - started, "exitCode": result.returncode}
for option in ["-o", "--output"]:
    if option in args:
        output = pathlib.Path(args[args.index(option) + 1])
        if result.returncode == 0 and output.is_file():
            record.update({"output": str(output), "sha256": hashlib.sha256(output.read_bytes()).hexdigest()})
        break
if os.environ.get("PAGE_AUDIT_INVOCATIONS") and "--version" not in args:
    with open(os.environ["PAGE_AUDIT_INVOCATIONS"], "a") as log:
        log.write(json.dumps(record) + "\n")
sys.exit(result.returncode)
