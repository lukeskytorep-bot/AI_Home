#!/usr/bin/env python3
"""Optional independent verification of public-only Rust proof artifacts.

Generate with AI_HOME_COMPLETION_PUBLIC_INTEROP_DIR set during cargo tests.
Requires openssl CLI; not a mandatory new CI dependency. No private keys exported.
"""
import base64
import json
from pathlib import Path
import subprocess
import sys
import tempfile

if len(sys.argv) != 2:
    raise SystemExit('Usage: python3 scripts/verify-community-completion-openssl.py PUBLIC_ARTIFACT_DIR')
root=Path(sys.argv[1]); verified=0; rejected=0
def decode(s): return base64.urlsafe_b64decode(s+'='*((-len(s))%4))
with tempfile.TemporaryDirectory(prefix='ai-home-public-proof-') as temp:
    d=Path(temp)
    for purpose in ['status','reissue']:
        value=json.loads((root/(purpose+'.json')).read_text())
        (d/'public.der').write_bytes(decode(value['publicKey']))
        (d/'signature.der').write_bytes(decode(value['signature']))
        (d/'message.txt').write_bytes(value['message'].encode())
        p=subprocess.run(['openssl','pkey','-pubin','-inform','DER','-in',str(d/'public.der'),'-out',str(d/'public.pem')],capture_output=True)
        assert p.returncode==0,'OpenSSL public-key conversion failed'
        command=['openssl','dgst','-sha256','-verify',str(d/'public.pem'),'-signature',str(d/'signature.der'),str(d/'message.txt')]
        p=subprocess.run(command,capture_output=True)
        assert p.returncode==0,purpose+' signature failed'
        verified+=1
        (d/'message.txt').write_bytes(value['message'].encode()+b'TAMPER')
        p=subprocess.run(command,capture_output=True)
        assert p.returncode!=0,purpose+' tamper was accepted'
        rejected+=1
        print('PASS:',purpose,'Rust signature verified by OpenSSL; altered message rejected')
print(f'PASS: {verified}/2 signatures verified; {rejected}/2 tampered messages rejected; public-only artifacts.')
