#!/usr/bin/env python3
"""Reject source-text corruption patterns that have previously reached CI."""
from pathlib import Path
import re
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
        # Store gained private lifecycle/path state in 4B-6. External struct literals
        # silently become stale whenever Store evolves, so require the constructor.
        if p.name != "persistence.rs" and re.search(rb"(?<![A-Za-z0-9_])Store\s*\{", raw):
            problems.append(f"{p}: external Store struct literal; use Store::from_existing_pool/open/memory")
if problems:
    print("FAIL: suspicious literal escape sequences in Rust source:")
    print("\n".join(problems))
    sys.exit(1)
print("PASS: Rust source-text normalization sweep.")
