#!/usr/bin/env python3
"""Independent public protocol serialization from accepted PHP field order.
Does not import Rust or execute PHP. Usage: python3 build-proof-fixtures.py OUT.
"""
import hashlib,json,sys
from pathlib import Path
if len(sys.argv)!=2:raise SystemExit('Usage: python3 build-proof-fixtures.py OUTPUT_DIRECTORY')
source=Path(__file__).parent/'proof-fixtures';out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
raw=(source/'fields.json').read_bytes();fields=json.loads(raw);(out/'fields.json').write_bytes(raw)
for purpose in ['status','reissue']:
    status=purpose=='status'
    body=b'' if status else b'expected_token_generation=7\n'
    route='/api/v1/registration/'+fields['registration_id']+('/status' if status else '/token/reissue')
    values=[(k,fields[k]) for k in ['community_origin','community_instance','registration_id','agent_id','operational_key_id']]
    values += [('purpose',purpose),('method','GET' if status else 'POST'),('route',route)]
    values += [(k,fields[k]) for k in ['challenge_id','nonce','expires_at']]
    values += [('body_sha256',hashlib.sha256(body).hexdigest()),('signature_algorithm','openssl-ecdsa-p256-sha256-der/1'),('public_key_format','spki-der-base64url/1'),('signature_format','asn1-der-base64url/1')]
    text=('AGENT-REGISTRATION-STATUS-PROOF/1' if status else 'AGENT-REGISTRATION-REISSUE-PROOF/1')+'\n'+''.join(k+'='+v+'\n' for k,v in values)
    (out/(purpose+'.txt')).write_bytes(text.encode())
print('PASS: 2 public proof statements and public input fields regenerated; no private keys.')
