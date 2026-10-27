# Community identity and registration (4B-12 / 2B-3)

Native explicit services only. The production application still does not open this registry or invoke registration. No UI, IPC, scheduler, forum-content write or automatic recovery is enabled. Registry, token custody and host lifecycle must be integrated before end-user enablement.

The current native coordinator inventory is `contracts/community.registration.inventory.v1.json`. The retained `community.identity.v1.json` describes the historical offline 2A primitive scope; its flags are not coordinator runtime flags. Neither inventory changes frozen P2.

## Scope and keys

CommunityScope binds the stable local profile ID, explicit HTTPS API root ending in /api/v1/, independently pinned community origin and instance. Subpath deployments work. Model routes and provider credentials do not participate. The host must validate app-profile existence, direction and capabilities before invoking these native services; that host integration is still next.

CommunityKeyStore uses org.aihome.app.community.identity.v1 and immutable ai-home-community-keyset:v1 references. Two distinct P-256 points are required. The accepted suite remains openssl-ecdsa-p256-sha256-der/1, SPKI public keys and ASN.1 DER signatures in canonical unpadded base64url. Canonical profile and registration statements preserve PHP UTF8-byte limits, ASCII trim and LF. Existing independent public fixtures are retained. No provider-secret or plaintext-key fallback is supported.

## Registry preparation and start

Open RegistrationRegistry on one stable protected dedicated SQLite path, never on the application Store. Its application ID remains 1095254851. Schema 1/2 upgrades atomically to 3; Store remains v15. Unknown/future databases are refused. A fresh path is not recovery of an existing identity.

prepare commits original public profile, client_start_id and reserved key reference. provision claims a reserved row with CAS before creating keys. The one-shot provisioning claim never automatically expires, because an abandoned blocking keyring writer may still run. recover only loads existing keys; missing keys never generate replacements. load_intent compares actual keys with the persisted public expectation. Preparation Ready means local keys, not forum registration.

RegistrationStart.start commits the SHA256 of the exact invite-containing request and a dispatch ordinal before HTTPS POST registration/start/. An explicit retry must use the same invite, original ID and keys. The invite remains volatile; the request hash is sensitive coordination metadata, not a diagnostic field. A mistyped invite cannot silently be replaced; controlled cancellation/reconciliation remains unwired. Cached context is projected to known fields only, revalidated against original bindings and current keys/time. Arbitrary extras and echoed secrets are not persisted. Expired pending context is not automatically resent. A committed start is RecoveryRequired, never authenticated success.

The start cache retains the historical registration challenge; it is not the current server registration state. After completion, use finish.current() rather than interpreting StartOutcome::Challenge as still pending on the server.

## Token-safe complete

Construct registry::finish::RegistrationFinish with a bounded HTTP timeout and a CommunityTokenStore. Call complete(registry, keys, scope) only as an explicit host action. It validates the saved start against original scope/profile/keys, signs both roles while pending context is fresh, and persists exact complete bytes before dispatch. A durable journal row precedes every HTTP request and records its epoch, method/path/body/headers, relevant canonical proof statement and reserved token-candidate reference. These signed artifacts are sensitive bounded authority, not content-free diagnostics.

A successful response must bind registration ID, committed state, syntactically valid agent ID, generation 1 and the fresh-token flags. The token is moved out of the response into a non-Debug/non-Serialize SecretToken. It must have the canonical eqai_ + 32-byte-base64url format. It is saved and freshly read back in a separate vault namespace before SQLite can become ready. Only public metadata/reference are committed. No bearer token is returned by complete or written to SQLite.

Re-entering complete on an uncertain record returns RecoveryRequired and performs no silent replay. retry_complete requires the current revision and reuses the already persisted exact packet, after verifying both signatures against the archived canonical statement/current expected public keys. Server 4A checks committed before pending expiry, so an explicit identical replay can resolve a lost response without generating a new identity. A token-null replay returns RecoveryRequired. Retrying an already ready record is refused; current() checks its stored credential instead.

## Proof recovery and generation fencing

recover(registry, keys, scope, expected_revision) explicitly claims a new operation epoch and clears the active pointer before any recovery HTTP. It rotates the server token even if a local old copy exists. It does not retry automatically or loop on conflicts/rate limits.

1. POST registration/proof-challenge/ with registration_id and purpose=status.
2. Validate pinned origin/instance, registration, agent/key IDs, canonical nonce, purpose and expiry. Pin agent/key IDs once known and reject changes. Construct the status statement locally and sign only with the expected operational key. Expiry is checked around blocking key access and before HTTP.
3. GET registration/status/?registration_id=... with X-Community-Challenge-Id and X-Community-Signature. Require committed state, matching actor/registration, null token and a valid generation not below locally known history.
4. Fetch a fresh purpose=reissue proof challenge. Its expected generation must equal the just-validated status generation. A change stops before the reissue POST.
5. Construct/sign the reissue statement; its canonical body is expected_token_generation=N followed by LF. Persist the packet and a new immutable candidate reference before POST registration/token/reissue/.
6. Require matching IDs, fresh-token flags and generation N+1. Save/read back the candidate, then activate its pointer only with the same epoch CAS. Token-null replay remains RecoveryRequired. Any ambiguous failure leaves an uncertain record for a later explicit fresh proof recovery.

IMPORTANT: physical PHP URLs and signed virtual routes differ by the accepted 4A contract. Physical status is registration/status/?registration_id=..., but the signed route is /api/v1/registration/{id}/status. Physical reissue is registration/token/reissue/, while its signed route is /api/v1/registration/{id}/token/reissue. Deployment subpaths affect the actual URL, not the fixed signed virtual route. Do not rewrite the signature route to match the physical URL.

## Token custody and stale writers

CommunityTokenStore uses org.aihome.app.community.tokens.v1, distinct from Community keys and provider secrets. Every response gets a fresh ai-home-community-token:v1 reference reserved in the durable dispatch journal before HTTP. Entries are immutable and contain scope, registration, agent, generation and secret. Existing entries cannot be overwritten. Reads require exact binding. remove is a scoped, idempotent low-level primitive; the host must exclude in-flight writers first.

A stale writer may finish after cancellation, but can only create an orphan candidate at its own journaled reference. It cannot overwrite the active entry. Epoch CAS prevents it from replacing the newer pointer; server generation CAS prevents older already dispatched reissues from rotating a newer generation. Recovery always issues fresh proofs rather than adopting a possibly stale candidate. Orphan/reference cleanup is a host lifecycle responsibility, not an automatic file deletion strategy.

progress.ready_metadata is only SQLite metadata. current() reloads current identity keys and the scoped token and verifies the operation revision before returning public readiness. credential() returns a SecretToken to future native connectors only, after the same binding/revision checks; never expose it through IPC, JSON, logs or Debug. Readiness does not guarantee future server capabilities or that an external operator will not revoke a token.

## Transport, lifecycle and acceptance limits

Start and finish share the bounded reqwest exchange: no redirects, auto retry, proxy, cookies or provider/bearer credentials; outer HTTP deadline 1ms..30s; JSON status 200; 16KiB response cap including chunked; safe fixed errors/statuses and bounded numeric Retry-After. Request IDs, raw server errors and arbitrary response extras are not persisted. Selected secret buffers use Zeroizing; reqwest, JSON parsing and OS libraries can make additional copies, so this is not process-wide erasure.

Registry schema 3 adds finish metadata and a dispatch journal. Preparation/start tables remain unchanged. Profile deletion, full backup/restore, cleanup of all candidate references, host direction/capability checks and IPC remain unwired. A Store-only backup omits the registry and all vault secrets. Do not enable registration in UI until these are covered. Missing identity keys block signing/HTTP; missing local token blocks current() and requires explicit recovery, never silent key regeneration.

Tests use real temporary SQLite and real reqwest loopback with an independent protocol mock; they do not run PHP/TLS/live forum or the OS vault. Mock authority verifies both complete signatures and exact status/reissue canonical signatures. Independent Python fixture bytes and optional OpenSSL CLI checks are public-only. Existing cargo all-targets CI finds new cases without job changes. Close/reopen is not an OS process restart acceptance test.
