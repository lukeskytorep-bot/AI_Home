"""Regression checks for content identity independent of downloaded ZIP filename."""
import shutil,subprocess,sys,tempfile,warnings,zipfile
from pathlib import Path
root=Path(__file__).resolve().parent
archive=root/'AI_HOME_P2_DOWODY.zip';digest=root/'AI_HOME_P2_DOWODY_SHA256.txt';checker=root/'verify_handoff.py'
checks=0
def run(name,path,valid,digest_path=None,reason=None):
 global checks
 args=[sys.executable,str(checker),str(path)]
 if digest_path:args.append(str(digest_path))
 r=subprocess.run(args,capture_output=True,text=True)
 if (r.returncode==0)!=valid:raise AssertionError(name+' unexpected verifier result: '+r.stdout+r.stderr)
 if reason and reason not in r.stderr:raise AssertionError(name+' failed for unexpected reason: '+r.stderr)
 checks+=1;print('PASS '+name)
run('original ZIP and external digest',archive,True,digest)
with tempfile.TemporaryDirectory() as temp:
 renamed=Path(temp)/'AI_HOME_P2_DOWODY(1).zip';shutil.copyfile(archive,renamed)
 run('renamed identical ZIP and original digest filename',renamed,True,digest)
 altered=Path(temp)/'changed-bytes.zip';altered.write_bytes(archive.read_bytes()+b'changed')
 run('changed ZIP bytes rejected by external digest',altered,False,digest,'ZIP digest mismatch')
 for mode,reason in [('extra','missing or unlisted payload'),('missing','missing or unlisted payload'),('duplicate','duplicate entries')]:
  target=Path(temp)/(mode+'.zip')
  with zipfile.ZipFile(archive) as src,zipfile.ZipFile(target,'w') as dst:
   for name in src.namelist():
    if mode=='missing' and name=='contracts/defaults.json':continue
    dst.writestr(name,src.read(name))
   if mode=='extra':dst.writestr('unlisted.txt',b'x')
   if mode=='duplicate':
    with warnings.catch_warnings():
     warnings.simplefilter('ignore',UserWarning);dst.writestr('contracts/defaults.json',src.read('contracts/defaults.json'))
  run(mode+' payload rejected by manifest',target,False,reason=reason)
print(f'RESULT: {checks}/{checks} handoff verifier checks PASS')
