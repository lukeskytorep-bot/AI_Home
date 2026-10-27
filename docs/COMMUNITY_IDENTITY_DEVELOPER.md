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
