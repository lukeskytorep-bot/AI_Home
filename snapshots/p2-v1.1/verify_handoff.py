"""Verify frozen handoff, detecting changed, missing, duplicate and unlisted files."""
import hashlib,re,sys,zipfile
from pathlib import Path,PurePosixPath
archive=Path(sys.argv[1]);digest_file=Path(sys.argv[2]) if len(sys.argv)>2 else None
if digest_file:
 line=digest_file.read_text().strip();digest,name=line.split('  ',1)
 assert re.fullmatch('[a-f0-9]{64}',digest) and hashlib.sha256(archive.read_bytes()).hexdigest()==digest,'ZIP digest mismatch'
 # The filename in the digest is descriptive; downloads may append '(1)'.
with zipfile.ZipFile(archive) as z:
 names=z.namelist();assert len(names)==len(set(names)),'duplicate entries'
 for name in names:assert not PurePosixPath(name).is_absolute() and '..' not in PurePosixPath(name).parts,'unsafe path'
 entries={}
 for line in z.read('SHA256SUMS.txt').decode().splitlines():
  digest,name=line.split('  ',1);assert re.fullmatch('[a-f0-9]{64}',digest),'invalid digest'
  assert name not in entries,'duplicate manifest entry';entries[name]=digest
 assert set(entries)==set(names)-{'SHA256SUMS.txt'},'missing or unlisted payload'
 for name,digest in entries.items():assert hashlib.sha256(z.read(name)).hexdigest()==digest,'payload mismatch: '+name
 print(f'PASS: {len(entries)} payload hashes; manifest complete'+('; external ZIP digest matches' if digest_file else '; no external ZIP digest supplied'))
