#!/usr/bin/env python3
"""Optional independent interoperability check. All input/output key material is public."""
from pathlib import Path
import base64,json,shutil,subprocess,sys,tempfile
root=Path(__file__).resolve().parents[1]
source=Path(sys.argv[1]) if len(sys.argv)>1 else root/'docs/evidence/4b12-step-2a/openssl-public.json'
if not shutil.which('openssl'):raise SystemExit('NOT RUN: OpenSSL CLI unavailable; this optional check is separate from Cargo CI.')
v=json.loads(source.read_text());assert set(v)=={'statement','identity_public_key','operational_public_key','complete'}
def decode(s):
 b=base64.urlsafe_b64decode(s+'='*((-len(s))%4));assert base64.urlsafe_b64encode(b).rstrip(b'=').decode()==s;return b
with tempfile.TemporaryDirectory(prefix='ai-home-public-interop-') as d:
 d=Path(d);text=d/'statement.txt';text.write_bytes(v['statement'].encode());tampered=d/'tampered.txt';tampered.write_bytes(b'TAMPERED\n'+text.read_bytes())
 for role in ['identity','operational']:
  der=d/(role+'.der');pem=d/(role+'.pem');sig=d/(role+'.sig');der.write_bytes(decode(v[role+'_public_key']));sig.write_bytes(decode(v['complete'][role+'_signature']))
  p=subprocess.run(['openssl','pkey','-pubin','-inform','DER','-in',str(der),'-out',str(pem)],capture_output=True);assert p.returncode==0
  cmd=['openssl','dgst','-sha256','-verify',str(pem),'-signature',str(sig)]
  assert subprocess.run(cmd+[str(text)],capture_output=True).returncode==0
  assert subprocess.run(cmd+[str(tampered)],capture_output=True).returncode!=0
print('PASS: 2/2 Rust-generated role signatures verified by OpenSSL CLI; 2/2 tampered statements rejected. No private keys exported.')
