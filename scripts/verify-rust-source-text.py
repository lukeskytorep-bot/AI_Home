#!/usr/bin/env python3
"""Reject source-text corruption patterns that have previously reached CI."""
from pathlib import Path
import sys
roots=[Path("src-tauri/src"),Path("src-tauri/tests")]
problems=[]
for root in roots:
    for p in root.rglob("*.rs"):
        raw=p.read_bytes()
        checks=[
            (b"\\n#[", "literal \\n before Rust attribute"),
            (b"let \\_=", "escaped underscore in let binding"),
            (b";\\n#[", "literal \\n between Rust items"),
        ]
        for needle,label in checks:
            if needle in raw:
                problems.append(f"{p}: {label}")
if problems:
    print("FAIL: suspicious literal escape sequences in Rust source:")
    print("\n".join(problems))
    sys.exit(1)
print("PASS: Rust source-text normalization sweep.")
