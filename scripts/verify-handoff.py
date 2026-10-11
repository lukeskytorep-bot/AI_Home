#!/usr/bin/env python3
"""Check a complete source handoff; the downloaded filename is irrelevant."""
import sys,json,hashlib,zipfile
from pathlib import PurePosixPath
with zipfile.ZipFile(sys.argv[1]) as z:
    assert z.testzip() is None, "ZIP CRC failure"
    names=z.namelist();assert len(names)==len(set(names)), "Duplicate archive entry"
    manifest_name="AI-Home/docs/evidence/manifest.sha256.json"
    manifest=json.loads(z.read(manifest_name))
    assert set(names)==set(manifest)|{manifest_name}, "Inventory mismatch"
    for name,sha in manifest.items():
        p=PurePosixPath(name);assert not p.is_absolute() and ".." not in p.parts, "Unsafe archive path"
        assert hashlib.sha256(z.read(name)).hexdigest()==sha, "Hash mismatch: "+name
    print(f"PASS: {len(manifest)} file hashes and complete inventory; filename-independent.")
