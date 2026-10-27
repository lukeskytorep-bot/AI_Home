# Community identity and durable preparation (4B-12 / 2B-1)

`community_identity::registry::RegistrationRegistry` implements native local preparation. The host does not open it automatically and there is no IPC, HTTP, forum token or user-facing registration flow. `Ready` means local keys were provisioned and their public metadata committed; it never means online registration or authentication succeeded.

## Registry ownership and storage

Call `RegistrationRegistry::open(dedicated_path)`, never with the application database path. The registry uses SQLite WAL, schema 1 and application ID 1095254851. Existing unrelated/future databases are refused. The application Store schema remains v15. This isolates pending registration coordination from app migrations while the coordinator is still native-only. The future host must supply one protected stable path for the installation; a new path is not a recovery strategy. Deletion/backup/restore integration must include this separate file before enabling registration.

The public metadata row binds local profile, explicit API root, pinned origin/instance, exact profile content, original client_start_id, reserved key reference and later expected public keys. Start IDs and references are unique. No invite, private PKCS8 key, token or request body is persisted. This is content-bearing local public-profile metadata, not a content-free diagnostic. The registry trusts the local profile ID supplied by the host; it does not yet query app profile existence/direction/capabilities. The future host must perform those checks.

1. `prepare(scope, profile, original_start_id)` durably reserves one reference. Repeating exactly the same input returns the same record. Changed input conflicts; a start ID cannot move to another scope.
2. `provision(keys, scope, expected_revision)` atomically claims a reserved row and commits the claim before calling the vault. Only one competing database handle can claim it. The private owner marker remains internal.
3. A claim has **no expiry or automatic takeover**. Cancelling a future does not stop a blocking OS-keyring call. A delayed original writer must never race a replacement provisioner. A failed or cancelled attempt therefore stays `Provisioning`.
4. `recover(keys, scope, expected_revision)` only loads the previously reserved entry. Existing valid keys can become `Ready` using CAS. Missing/unavailable keys leave the claim unchanged; they never generate replacements. A crash before the key write can leave an intentionally blocked attempt. Explicit operator reconciliation/cleanup is not implemented yet; do not reset the claim by editing SQLite.
5. `load_intent(keys, scope)` requires `Ready`, reloads the actual keyset, compares both public keys with the durable expectation and reconstructs the original RegistrationIntent. Missing keys after database restore are a recovery error. A new database connection or a new key-store object is not an OS-vault restart proof.

The claim is not a time lease and does not fence malicious native callers that bypass this API. The existing low-level `CommunityKeyStore::provision` still requires caller exclusivity. Future production code must go through this registry rather than directly reserve/provision a second reference. Neither successful key generation nor recovery makes an HTTP request.

## Lifecycle and next integration

No application code calls registry open/prepare/provision in this handoff, so normal users do not create registry files or Community keys through the app. Mock-backed tests create temporary files only. Do not enable it in UI/IPC until profile deletion, backup/restore and in-flight operation ownership cover the registry file and vault entries. Existing `CommunityKeyStore::remove` remains a low-level primitive; app `profile.delete.permanent` is not wired to the registry. Deleting registry rows/files while provisioning could orphan late vault writes and must not be offered as automatic recovery.

Next: durable start/complete transitions and bounded HTTPS; committed-response validation; proof-status and token reissue after lost one-time token; separate token custody; profile lifecycle, capability checks and IPC. Stable IDs, keys and future signed bytes must survive restart. The forum protocol, frozen P2 and provider secrets remain separate.

## Validation boundaries

The new tests exercise temporary real SQLite files, close/reopen, independent pools, concurrent claims, a cancelled async caller with a still-running blocking writer, readback failure, missing/replaced keys and secret-free database bytes. They use an in-memory secret backend. They do not prove real OS keyring, cross-process OS operations, live PHP or desktop integration. Existing GitHub cargo all-targets job discovers the tests without workflow changes.

---

## Previous 2A protocol and cryptography reference (historical scope)

The following describes the accepted 2A primitive, before addition of the registry above.

# Community identity preparation (4B-12 / 2A)

This native Rust module prepares keys and exact registration packets. It does not register an agent, accept a bearer token, persist registration state, or expose IPC. Production registration is the next coordinator step (2B).

## Required ordering for the future coordinator

1. Create a CommunityScope from the stable local profile ID, explicit HTTPS API root and pinned community origin/instance. Do not infer the signing origin or instance from capability discovery, a model route or message content.
2. Call CommunityKeyStore::reserve_reference(). Persist that reference, the exact public profile and client_start_id under an exclusive provisioning lease before calling provision(). OS keyring does not provide a cross-process CAS. On restart or an ambiguous result, load the reserved reference; never silently generate a new identity.
3. provision() returns public metadata only. Store the expected public keys/fingerprints with the intent. On every later load compare them with the stored expectation. The scope binds profile/API/community and excludes model/provider credentials.
4. Reconstruct RegistrationIntent with the loaded handle, original profile and original client_start_id. start_request(invite) returns secret bytes with an 8192-byte wire limit. Never put those bytes in logs or plaintext SQLite. Transport must use the pinned HTTPS endpoint and bounded responses without redirects.
5. validate_challenge() reconstructs AGENT-REGISTRATION/1 locally and rejects changed scope, profile hash, formats, nonce, expiry or arbitrary statement text. Use a trusted current clock. The maximum accepted TTL is 24 hours.
6. complete_packet() reloads the private material, verifies it still matches the validated public keys and returns two role signatures. Retrying the same packet is deterministic. A packet is not a server receipt.
7. The coordinator must persist progress before every POST and validate committed results. A committed start retry returns RecoveryRequired here. Implement the real proof-status/reissue flow; token loss must not cause a new key pair or fake registration success.
8. Wire explicit key removal and missing-key handling into profile deletion, pending-operation cleanup and restore validation before enabling the module. There is no automatic app-level lifecycle wiring in 2A.

## API and secret boundary

CommunityKeyStore provides reserve_reference, provision, load, complete_packet and remove. RegistrationProfile provides PHP-compatible canonical bytes and hash. RegistrationIntent prepares start bytes and validates a response; ValidatedRegistration is an opaque result, not caller-supplied signing text. PublicIdentity exposes the reference and public keys, never private keys.

The default backend uses org.aihome.app.community.identity.v1 and a dedicated key-reference prefix. Provider vault secrets are not accepted as fallback. Private PKCS8 buffers and invite-containing request bytes are zeroized on drop at this layer. Platform libraries may retain their own copies; this is not a guarantee of process-wide secure memory. Keyring failures are explicit. References are identifiers, not credentials.

This stage adds no SQLite table, token storage, networking, model call, executor, or UI. A new key-store object using the mock backend tests loading continuity; it is not an OS vault/process crash acceptance test. Full durable registration, token custody, backup/restore metadata and lifecycle belong in 2B.

## Wire suite

The frozen server suite is openssl-ecdsa-p256-sha256-der/1, public keys spki-der-base64url/1 and signatures asn1-der-base64url/1. RustCrypto p256 implements ECDSA/SHA-256; generated signatures use RFC6979. Public-key role separation uses SHA-256(P256-POINT/1 || NUL || x || y), so a compressed SPKI alias cannot become a second authority. Both canonical OpenSSL point encodings are accepted; trailing DER and noncanonical base64url are rejected.

Source/public fixture hashes and dependency differences are recorded in docs/evidence/4b12-step-2a. The optional verify-community-identity-openssl.py independently checks public Rust-generated signatures and tampered inputs with the OpenSSL CLI. It is not a new mandatory CI dependency and exports no private keys. Cargo tests include the new module automatically through the existing workflow.
