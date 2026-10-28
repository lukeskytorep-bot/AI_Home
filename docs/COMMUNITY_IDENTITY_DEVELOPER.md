# Audit correction: legacy restore completion recognition (2B-5.3.1 Fix 1)

The managed host read-only guard now validates the exact legacy request-v2 and terminal-result formats before treating a request as completed. A parseable JSON file alone is insufficient. Request fields, safe backup IDs, checksum, RFC3339 timestamp and canonical request-key filename are checked. Completion requires boolean applied=true, requestId/backupId/safetyBackupId bound to that request, a valid appliedAt, and matching known positive integer source/restored schema versions. Missing results remain pending; malformed, incomplete, mistyped, mismatched or unknown formats remain recovery errors. Direct result-file symlinks are refused.

The check does not compare the present Main hash to the historical backup/result: normal app writes after a completed restore are valid. It neither requires old backup files for historical recognition nor rewrites/deletes ledger records. Legacy Standard writers and restore execution are unchanged; the correction is confined to the native Community host guard. No IPC contract, migration, dependency or production desktop activation is added.

Ten regressions reproduce 8 failing negative groups and 2 passing positive cases on the old guard. The corrected host suite has 27 passing tests, including an actual legacy-writer receipt followed by a later Main change. Keep the rest of the integration gates below. New CI and re-audit are required before accepting this substep.

---

# Native managed Community host (4B-12 / 2B-5.3.1)

`community_host::maintenance::CommunityHost` now owns the Application lifecycle around the two-store offline coordinator. This is a trusted native integration seam. The production Tauri entry point, UI and existing IPC contract are unchanged. Direction/capability authorization, command/result adapters and desktop activation remain subsequent 2B-5.3 work.

Construct with an existing protected absolute app directory, the exact `community.sqlite3` registry owner, `CommunityLifecycle`, the model Vault and an optional FilePicker. Construction opens no Store and reads no secrets. It returns Offline or RecoveryRequired. It does not initialize a fresh installation. A missing Main never becomes an empty replacement database.

Call `open_application()` explicitly. It refuses native pending/fence markers, unresolved or malformed legacy restore ledgers, missing Main and a mismatched durable registry pin. It waits at most five seconds for Main ownership to drain before opening Store. It never automatically applies a restore, reissues tokens, generates keys or posts registration HTTP. The legacy Standard desktop startup continues to use its existing entry point.

All native application requests must go through `host.dispatch(request)`. All host clones share one admission gate and Application; no Application/Store/pool is returned. A second ordinary request while one dispatch holds the gate receives native Busy. These host errors are not additions to frozen IPC enums. The caller remains responsible for authenticated local authorization and eventual IPC error mapping. Do not route arbitrary UI-supplied filesystem paths to this API.

`create_backup(destination, id)`, `restore(bundle, id, policy)` and `finish_cleanup(id)` immediately mark Maintenance, reject new dispatch and other maintenance attempts, wait for earlier dispatch, consume/stop Application, close its pool and verify actual OS ownership drain. Closing is bounded to five seconds; a separate ownership drain also waits at most five seconds. Neither timeout releases another owner's lock. The offline coordinator then reacquires Main and registry ownership for the entire operation; losing that race fails safely. An external cooperating pool or registry credential lease can still block maintenance. A pool held outside the managed host is unsupported integration, never a reason to force its owner closed.

Once admitted, the local worker retains its gate even if the waiting caller is cancelled. A previously admitted dispatch is retained too, allowing local deletion work to settle before maintenance. Requests already queued/running in other privileged raw services are outside this dispatch gate; their pool ownership and registry leases still fence actual maintenance. No background AI restart is introduced.

The terminal phase is published once while holding the admission gate, so an older worker cannot reset the phase of a newly admitted action. After maintenance the host stays Offline, or RecoveryRequired when durable guards or missing Main remain. Reopen is explicit and constructs a fresh Application with the same vault, file picker and Community lifecycle. Reopen also requires ownership drain, including after a failed close. Ordinary dispatch never implicitly reopens.

Use the same bundle/request/policy to resume an interrupted restore. `receipt(id)` reads an already committed result; `finish_cleanup(id)` can release the remaining guards without original input only when the existing coordinator validates that receipt. The underlying merge-only registry semantics, tombstones, token generation rules and original-key reconciliation remain those of 2B-5.2. Recovery does not imply credential readiness.

This substep adds no registration IPC, production desktop setup, new frontend, schema migration, dependency change, permission, release workflow or custom scheduler. Test evidence uses real SQLite and deterministic fake secrets. Linux host orchestration is covered locally; real OS keyring, Windows/Tauri/WebView, live forum and physical power loss are not covered. The older offline API and historical notes follow.

---

# Community offline backup/restore integration (4B-12 / 2B-5.2)

The native offline coordinator is implemented; production host/UI/IPC enablement remains next in 2B-5.3. Keep the current app command guards until the host can close every Store pool and enter a recovery bootstrap. Store is 16; Community registry is 4. No schema migration or dependency update is introduced here.

## Native API

```rust
use ai_home::community_backup::{OfflineCommunityBackup, RestorePolicy};
// Protected existing absolute UTF-8 app_data; the stable registry is already open.
// Close and drop ALL Store-derived pools before maintenance, including clones.
let coordinator = OfflineCommunityBackup::new(&app_data, registry.clone())?;
let manifest = coordinator.create(&destination, "backup-request-id").await?;
let applied = coordinator.restore(
    &destination, "restore-request-id", RestorePolicy::SameInstallation
).await?;
let receipt = coordinator.receipt("restore-request-id")?;
// If a terminal result committed but marker cleanup was interrupted:
let applied = coordinator.finish_cleanup("restore-request-id").await?;
```

These results are native values/formats, **not frozen P2 IPC DTOs**. Add proper command/result validation and error mapping in the future IPC seam; do not expose filesystem paths or native enum names accidentally. Native errors are bounded categories without SQL/key/token payloads. No HTTP or keyring calls occur in this coordinator.

## Ownership and capture

Store pools now retain a shared fs2 owner on a persistent sibling `.app-owner-lock`. A surviving clone of a Store-created pool retains it even after the Store wrapper is dropped. Maintenance requires exclusive app ownership and the registry's exclusive lease. It never expires ownership. The Standard startup restore waits at most five seconds for asynchronous SQLx disposal, then refuses rather than forcing access. This protocol assumes a protected local filesystem and cooperating participants; raw SQL/keyring and unowned pools are privileged integration mechanisms, not a supported bypass.

Capture takes a logical VACUUM snapshot of Main and reuses the sanitized registry snapshot under the same held lease. The bundle contains exactly application.sqlite3, community.sqlite3 and manifest.json. Both files, manifest and directory are synced before a private temporary directory is renamed into the final destination. Existing destinations are verified idempotent replays or rejected, never overwritten. Incomplete private temporary directories can remain after an I/O failure; they are not published backups or silently removed.

Format 1 is deliberately strict: canonical current schema 16/4, known metadata fields, per-component SHA-256, integrity/FKs, profile coverage and disabled imported token pointers. Extra DDL is rejected even after rehash. Maximum sizes: Main 1 GiB, Community 64 MiB, JSON 64 KiB. This does not replace the legacy Standard backup/migration format. Direct component/directory symlinks and extra bundle files are rejected. Private Unix files/directories use 0600/0700; Windows relies on the protected directory ACL and needs acceptance. Bundles contain private app content and sensitive signed metadata despite excluding vault secrets.

## Restore and recovery

Restore validates both inputs and current installation, refuses pending deletion/deleting registry state or non-inactive current Experimental, and refuses to orphan a newer live Community profile absent from the source app DB. Source Main remains an app snapshot; it never authorizes stopping a live Experimental instance.

A durable request journal and main pending marker precede mutation. A persistent registry restore fence closes normal registry/start/finish and public lifecycle/reconciliation entry points across interruptions. Main startup and the old restore path reject the marker, including when the main DB was moved but its replacement has not yet been installed. Do not delete owner/fence/journal files manually.

A complete safety bundle is committed and recorded before Community merge. Restore merges only missing registry IDs, preserving live history/generation/tombstones. Imported profiles remain reconcile, with no trusted token pointer. The staged app DB is verified before any DML, pruned with the shared app deletion tree for currently deleted IDs, and explicitly pinned to the selected registry. Source bytes are checked again before swap. The previous Main is retained; WAL/SHM are never manually unlinked.

SameInstallation requires the correct pin. FreshInstallationRelocation explicitly permits rebinding only into an empty Main with no pin and an empty registry; it does not generate keys or change signed Community scope. A bound but empty registry is refused during restore because missing tombstones cannot be ruled out. Keep current registry history intact. A moved installation or input bundle during an in-flight restore is unsupported.

Replaying the same request continues roll-forward. A different fingerprint cannot take over a pending job. The terminal receipt is strictly validated and readable without the original input. finish_cleanup only uses an already committed receipt: before releasing a main pending marker it verifies the installed hash; it never turns an incomplete import into success. The normal restore replay path also checks the installed Main hash while its pending marker remains; a missing or changed file keeps both guards intact. If main was already released but the registry fence survived, housekeeping can clear the matching fence without rolling back subsequent legitimate app changes. Future host startup must offer explicit resume/cleanup before enabling normal Community work.

Local create/restore/cleanup workers retain their owners after the awaiting caller drops its future. No restart automatically posts to a provider/forum, generates keys, or reissues tokens. Missing keys stay missing until explicit reconciliation; original existing keys can be adopted through the existing lifecycle APIs. Live retained records remain subject to current()/credential() checks. Retention and a user-facing maintenance/recovery flow are not implemented here.

22 new Linux tests include four interruption windows, a normal cross-process restart, file ownership in a second process, receipt corruption, input removal after committed restore, tombstone pruning, current generation preservation, original/missing keys and destination/policy isolation. Two functions are subprocess probes. Tests use fake vault backends and test-only per-instance fault points. They do not prove real OS-vault, Windows/WebView, live TLS/API, forced process kill or physical power-loss behavior.

The previous integration details below remain useful. Their statements about backup being "next" describe the historical 2B-5.1 host seam; the native offline coordinator above now exists, while app IPC/desktop routing is still disabled.

---

# Community identity, registration and app deletion (4B-12 / 2B-5.1)

Native explicit services only. `Application::with_community_lifecycle(CommunityLifecycle::new(registry, keys, tokens))` connects the existing profile.delete.permanent dispatcher to real Community cleanup. This is an opt-in native integration seam; the production desktop still does not construct it. Registration UI/IPC, scheduler, forum-content writes and automatic recovery remain disabled.

## App deletion saga (2B-5.1)

Store schema 16 adds `profile_deletion_jobs` and `community_host_binding`; the Community registry stays at 4. All prior app rows/preferences are preserved. The existing deletion result/command contract is unchanged. After archived/revision/no-active-operations/inactive-Experimental preflight, the main transaction commits the original request ID, fingerprint and revision intent. No main-store transaction is held across keyring maintenance. Community cleanup drains its writers, removes original scoped keys and all journaled/active token candidates, then commits its tombstone. Only a subsequent main transaction deletes app data, tombstones old receipts, removes the job and commits success receipts.

A failed vault call or final transaction retains the profile and durable intent. Scoped commands are blocked while the job exists; other profiles remain independent. An explicit retry may use the original ID or a new ID joining the same profile/revision intent. Final commit records the original receipt too. Changed content under the reserved ID is an idempotency conflict, including app-preferences writes. The host worker retains its gate if the awaiting IPC future is dropped. A process restart does not automatically execute the job; it requires an explicit retry with the same registry.

The host pin hashes the canonical registry location with a domain prefix; it is an installation-location pin, not a cryptographic community identity or credential. Wrong paths are refused, and detaching the native service cannot bypass deletion or backup guards. The pin is persisted by accepted mutations, never by a read alone. The native host is trusted to select the correct protected registry initially and keep its DB/owner-lock path stable. No API permits silently clearing or rebinding the pin. A moved/restored installation needs explicit verified reconciliation in the next substep.

**Do not enable this seam in production yet.** Coordinated backup/restore is next (2B-5.2). The attached service or persisted pin/job deliberately refuses Store-only backup.create/restore preflight/request; backup dispatch and startup also guard durable state. This prevents an incomplete snapshot or old restore ledger from discarding pending cleanup or pretending to include Community. An unreadable current DB with a pending restore fails startup rather than assuming there is no deletion intent. Ordinary unbound Standard backups remain functional at v16. Registration direction/capability checks and IPC then follow in 2B-5.3.

The native integration suite uses the actual dispatcher and both SQLite stores with fake secret backends. It tests receipt ordering, orphan tokens, failures after key removal and after Community commit, restart, dropped waiters, two Application instances sharing a Store pool/registry owner, wrong-registry/detach guards, preflight, and v15 migration. It does not accept a real OS vault, desktop/WebView or a live server.

The current native coordinator inventory is `contracts/community.registration.inventory.v1.json`. The retained `community.identity.v1.json` describes the historical offline 2A primitive scope; its flags are not coordinator runtime flags. Neither inventory changes frozen P2.

## Native lifecycle, ownership and cancellation

Use one stable, protected, absolute UTF8 registry path for the installation. Registry::open holds an fs2 OS lock on the persistent sibling .owner-lock file. Independent handles in one process share the same owner; a second cooperating process is refused while that owner lives. Do not unlink/replace the registry or owner-lock file while running, and do not run multiple live copies against the same vault references. Raw SQLite access and low-level vault calls are privileged and must not bypass this protocol.

Normal registry/start/finish operations acquire a read lease. Scoped key/token stores clone it into spawn_blocking jobs, so cancelling the async caller cannot falsely drain an OS-keyring writer. A credential returned by RegistrationFinish::credential retains the lease until the SecretToken is dropped. Keep it only for the native request, never in a long-lived credential cache. Maintenance waits for an exclusive lease, including outstanding credentials and blocking workers. It must not be called while the same caller retains such a credential.

lifecycle::delete_profile affects only Community data/secrets for the supplied local profile ID, not app conversations/profile rows or the remote forum actor. It commits deleting first, removes the original key references and the union of journaled/active token candidates with scope checks and fresh absence checks, then atomically erases Community rows and keeps a minimal deleted profile-ID tombstone. No display/about/key references survive in that tombstone. A vault failure leaves deleting and metadata/references in place for idempotent retry; ordinary work stays blocked. Orphan tokens can be removed by scope without trusting an absent agent/generation receipt. Other profiles are untouched. Exported backups/media are not erased by this operation.

Local maintenance tasks (delete/snapshot/restore/reconcile) complete their critical work even if the caller drops its future. They retain ownership while doing so. This is not cancellation of an AI task or permission to publish; it is local storage maintenance. A hung vault may keep maintenance waiting rather than allow an unsafe deletion. No timer expires writer ownership. OS process death releases the kernel lock; there is no persisted PID claim to guess or reset. Tests exercise cross-process exclusion and release after a normal close; force-kill and real OS-vault shutdown acceptance remain separate.

## Snapshot and restore policy

lifecycle::snapshot waits for the exclusive owner and creates a new file only. Parent must exist and be protected; an existing file is never overwritten. Paths must be absolute UTF8. Apostrophes and native backslashes are preserved correctly. Snapshot and recognized registry files are 0600 on Unix (including existing DB/WAL/SHM files); Windows inherits the protected host directory ACL and needs separate acceptance. The file and Unix parent are synced before success. A snapshot contains public-profile metadata, references and bounded signed artifacts, so it is sensitive even though vault secrets are not included. The receipt's secrets_included=false means no key/token vault export, not a DLP guarantee about text supplied by the owner.

The SQLite copy is consistent and sanitized: active profile gates become reconcile; all finish token pointers are cleared and states become uncertain. Live registry readiness is unchanged. A deleting profile prevents snapshot creation. This is a dedicated-registry snapshot, not a complete app Store backup. Size is bounded to 64MiB for export/restore.

restore_metadata takes the expected SHA256 and reads an immutable/read-only SQLite source. It validates size, application/schema identity, the complete schema shape against the trusted target (before evaluating arbitrary table content/integrity), integrity/FKs, gate states and disabled finish pointers. Extra views/triggers or schema changes are rejected. Hash is rechecked before the destination transaction commits. Only known table values are copied with bound parameters; source DDL/code is never installed.

Restore merges only absent Community profiles. Existing live/reconcile/deleting/deleted IDs are skipped; it never rewinds live generation/history, deletes live data, or overrides a local deletion tombstone. Imported deleted markers stay deleted. Imported nondeleted metadata remains reconcile and cannot provision, start, complete or expose credentials. Read-only lookups and invalid prepare inputs do not reserve a profile or block a later restore.

reconcile_profile explicitly checks original references and public-key expectations with the local key vault. Missing keys leave the gate closed, never generate replacements. Restored reserved/provisioning intents also stay closed until reconcile_keys explicitly adopts already existing scoped material; no missing entry is created by that API. After successful reconciliation the gate opens, but token pointers remain uncertain: the host must explicitly perform proof recovery before credential access. The active field means an operation gate, not authentication or guaranteed key availability forever. Normal readiness still uses finish.current().

The opt-in app deletion/receipt saga above is implemented. The host must next coordinate both databases' backups/restores, then direction/capability checks and registration IPC. Keep the production host disconnected until that orchestration is complete. A raw copy of the registry is not a sanitized snapshot; a Store-only backup does not include this registry or any vault secrets.

## Scope and keys

CommunityScope binds the stable local profile ID, explicit HTTPS API root ending in /api/v1/, independently pinned community origin and instance. Subpath deployments work. Model routes and provider credentials do not participate. The host must validate app-profile existence, direction and capabilities before invoking these native services; that host integration is still next.

CommunityKeyStore uses org.aihome.app.community.identity.v1 and immutable ai-home-community-keyset:v1 references. Two distinct P-256 points are required. The accepted suite remains openssl-ecdsa-p256-sha256-der/1, SPKI public keys and ASN.1 DER signatures in canonical unpadded base64url. Canonical profile and registration statements preserve PHP UTF8-byte limits, ASCII trim and LF. Existing independent public fixtures are retained. No provider-secret or plaintext-key fallback is supported.

## Registry preparation and start

Open RegistrationRegistry on one stable protected dedicated SQLite path, never on the application Store. Its application ID remains 1095254851. Schema 1/2/3 upgrades atomically to 4; the separate application Store is now v16. Unknown/future databases are refused. A fresh path is not recovery of an existing identity.

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

Registry schema 4 retains finish metadata/journal and adds the profile lifecycle table. Preparation/start/finish/journal DDL remain unchanged. Native Community deletion, snapshot/merge restore, reconciliation and scoped candidate cleanup now exist above. Application profile deletion, full app backup/restore orchestration, host direction/capability checks and IPC remain unwired. A Store-only backup omits the registry and all vault secrets. Do not enable registration in UI until these are covered. Missing identity keys block signing/HTTP; missing local token blocks current() and requires explicit recovery, never silent key regeneration.

Tests use real temporary SQLite and real reqwest loopback with an independent protocol mock; they do not run PHP/TLS/live forum or the OS vault. Mock authority verifies both complete signatures and exact status/reissue canonical signatures. Independent Python fixture bytes and optional OpenSSL CLI checks are public-only. Existing cargo all-targets CI finds new cases without job changes. Close/reopen is not an OS process restart acceptance test.
