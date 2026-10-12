#!/usr/bin/env python3
"""Verify this handoff's complete source inventory, from a directory or ZIP."""
import hashlib,json,sys,zipfile
from pathlib import Path,PurePosixPath
EXCLUDED_DIRS={'.git','node_modules','dist','target'}
manifest_path='docs/evidence/manifest.sha256.json'
checksum_path='SHA256SUMS.txt'
target=Path(sys.argv[1]) if len(sys.argv)>1 else Path.cwd()
if target.is_dir():
    data={p.relative_to(target).as_posix():p.read_bytes() for p in target.rglob('*') if p.is_file() and not any(x in EXCLUDED_DIRS for x in p.relative_to(target).parts)}
else:
    with zipfile.ZipFile(target) as z:
        assert z.testzip() is None,'ZIP CRC mismatch'
        names=[n for n in z.namelist() if not n.endswith('/')]
        assert len(names)==len(set(names)),'Duplicate entries'
        for name in names:
            p=PurePosixPath(name);assert not p.is_absolute() and '..' not in p.parts and '\\' not in name,'Unsafe path'
        candidates=[n for n in names if n.endswith('/'+manifest_path) or n==manifest_path]
        assert len(candidates)==1,'Ambiguous/missing live manifest'
        prefix=candidates[0][:-len(manifest_path)]
        assert all(n.startswith(prefix) for n in names),'Entries outside source root'
        data={n[len(prefix):]:z.read(n) for n in names}
manifest=json.loads(data[manifest_path])
assert manifest['schemaVersion']==1
assert manifest['excludedFiles']==[manifest_path,checksum_path]
files=manifest['files']
assert set(data)==set(files)|set(manifest['excludedFiles']),'Complete inventory mismatch'
for name,sha in files.items():
    assert hashlib.sha256(data[name]).hexdigest()==sha,'Hash mismatch: '+name
sums={}
for line in data[checksum_path].decode().splitlines():
    sha,name=line.split('  ',1);assert name not in sums,'Duplicate checksum';sums[name]=sha
assert set(sums)==set(data)-{checksum_path},'Checksum inventory mismatch'
for name,sha in sums.items():
    assert hashlib.sha256(data[name]).hexdigest()==sha,'SHA256SUMS mismatch: '+name
print(f'PASS: complete tree, {len(files)} manifest hashes and {len(sums)} SHA256SUMS entries.')
