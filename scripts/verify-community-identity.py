#!/usr/bin/env python3
"""Static identity scope guards; native tests and OpenSSL log provide behavior proof."""
from pathlib import Path
import hashlib,json
r=Path(__file__).resolve().parents[1];d=r/'src-tauri/src/community_identity';m=(d/'mod.rs').read_text();c=(d/'crypto.rs').read_text();v=(d/'vault.rs').read_text();t=(d/'tests.rs').read_text();metadata=json.loads((r/'contracts/community.identity.v1.json').read_text());e=r/'docs/evidence/4b12-step-2a';identity=json.loads((e/'source-identity.json').read_text());checks=[
 ('known 4A source',identity['forumSourceSha256']=='afe25c58590ec146f7be01ef83fc4854e717813bb99c86cd7d2c89251e4ebc8a'),
 ('PHP snapshot identity',all(hashlib.sha256((e/'forum-reference'/n).read_bytes()).hexdigest()==h for n,h in identity['phpSourceFiles'].items())),
 ('public fixture identity',all(hashlib.sha256((r/'src-tauri/tests/fixtures/community-identity'/n).read_bytes()).hexdigest()==h for n,h in identity['copiedPublicFixtures'].items())),
 ('separate namespace',metadata['vaultService']=='org.aihome.app.community.identity.v1' and metadata['vaultService'] in v),
 ('typed validated registration','pub struct ValidatedRegistration' in m and 'reg.expected_public' in v),
 ('exact local statement validation','registration_statement' in m and 'r["statement"] != statement' in m),
 ('zeroizing secret buffers','Zeroizing' in m and 'Zeroizing' in c and 'Zeroizing' in v),
 ('point identity separates roles','P256-POINT/1' in c and 'distinct' in m and 'distinct' in v),
 ('reserved-reference provisioning','pub fn reserve_reference' in v and 'pub async fn provision' in v and 'AlreadyExists' in v),
 ('no fallback or model/network/SQL integration',all(x not in m+c+v for x in ['reqwest::Client','sqlx::','ActionExecutor','std::fs::','ModelRoute','ProviderConnection'])),
 ('explicit removal','pub async fn remove' in v and '.delete_credential()' in v),
 ('committed response is not token success','RecoveryRequired' in m and not metadata['tokenAcceptanceEnabled']),
 ('expiry and request-size guards','wire.len() > 8192' in m and 'registration.expires_at' in v),
 ('public interoperability and recovery tests','public_registration_packet_is_available' in t and 'reserved_reference_is_reusable' in t and 'compressed_key_alias' in t),
]
for name,ok in checks:
 if not ok:raise SystemExit('FAIL: '+name)
print(f'PASS: {len(checks)}/{len(checks)} Community identity static guards; not registration/server acceptance.')
