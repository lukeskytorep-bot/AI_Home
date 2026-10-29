"""Exercise actual Git checkout with Windows-style core.autocrlf=true."""
from pathlib import Path
import json,subprocess,sys,tempfile
root=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path.cwd()
paths=sorted(p for folder in ['community-identity','community-completion'] for p in (root/'src-tauri/tests/fixtures'/folder).rglob('*') if p.is_file())
original={p.relative_to(root).as_posix():p.read_bytes() for p in paths}
assert all(b'\r\n' not in b for b in original.values())
def run(directory,*args):return subprocess.run(['git','-C',str(directory),*args],check=True,capture_output=True).stdout
with tempfile.TemporaryDirectory(prefix='ai-home-eol-') as temporary:
 d=Path(temporary);run(d,'init','-q');run(d,'config','core.autocrlf','true');run(d,'config','user.name','Fixture checkout verification');run(d,'config','user.email','fixture-test@example.invalid')
 for n,b in original.items():
  p=d/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
 run(d,'add','.');run(d,'commit','-qm','canonical fixtures')
 for n in original:(d/n).unlink()
 run(d,'checkout-index','--all','--force')
 red=sum((d/n).read_bytes()!=b for n,b in original.items());assert red>0
 (d/'.gitattributes').write_bytes((root/'.gitattributes').read_bytes())
 run(d,'add','.gitattributes');run(d,'commit','-qm','pin LF checkout')
 for n in original:(d/n).unlink()
 run(d,'checkout-index','--all','--force')
 assert all((d/n).read_bytes()==b for n,b in original.items())
 # Even a pre-existing CRLF working copy is normalized when added under eol=lf.
 for n,b in original.items():(d/n).write_bytes(b.replace(b'\n',b'\r\n'))
 run(d,'add','src-tauri/tests/fixtures')
 assert all(run(d,'show',':'+n)==b for n,b in original.items())
 for n in original:(d/n).unlink()
 run(d,'checkout-index','--all','--force');assert all((d/n).read_bytes()==b for n,b in original.items())
 print(json.dumps({'core.autocrlf':True,'withoutAttributesChangedFiles':red,'withAttributesExactFiles':len(original),'existingCRLFTreeNormalizesToOriginalIndex':True,'canonicalFixtureBytesAndSignaturesUnchanged':True},indent=2))
