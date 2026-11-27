# AI Home 4B-12 — Paczka 4C: Społeczności (Maintenance UI + Podpisane Zapisy ECDSA P-256 + Publiczny Feed)

**Data:** 2026-11-27  
**Zakres:** Paczka 4C (krok 2 z planu 4B-12/13) — połączenia z dowolną kompatybilną społecznością, odczyt publicznego feedu, podpisane publikacje ECDSA P-256, trwałe uzgadnianie niepewnych zapisów (maintenance UI) oraz zamknięcie uwagi `P3-A` z Audytu 4B Fix 1.

## 1. Zrealizowane zmiany

1. **Kontrakt IPC i parytet komend (`85/85`)**:
   - Dodano [`contracts/community.social.ipc.json`](../contracts/community.social.ipc.json) z 5 komendami:
     - `community.account.list` (read-only, `onlineStatusKnown: false`)
     - `community.account.save` (mutacja, kanonizacja URL, klucz P-256 + opcjonalny token w `Vault`, `reconcileReceiptRef`)
     - `community.account.delete` (mutacja, czyszczenie + symetryczne `maybe_clear_autonomous_recovery`)
     - `community.feed.read` (read-only, zawsze `untrustedExternalContent: true` i `signatureValidation: "not_performed"`)
     - `community.post.publish` (mutacja, podpisany wpis ECDSA P-256 + `bodySha256` + `Idempotency-Key`)
   - Podłączono `community.social.ipc.json` do [`scripts/verify-command-parity.py`](../scripts/verify-command-parity.py) oraz routing i strażnik `restore_pending_for_store` w [`src-tauri/src/application.rs`](../src-tauri/src/application.rs).

2. **Persystencja SQLite v19 i sprzątanie profilu**:
   - Podniesiono schemat SQLite do `CURRENT_SCHEMA = 19` w [`src-tauri/src/persistence.rs`](../src-tauri/src/persistence.rs) z tabelą `community_accounts(profile_id, id, revision, body)`.
   - Dodano `"community_accounts"` do [`src-tauri/src/profile_deletion.rs`](../src-tauri/src/profile_deletion.rs) (`profile.delete.permanent` usuwa konta i tombstonuje `receipts`).
   - Zaktualizowano skrypty `verify-experimental-*.py` oraz asercje wersji schematu w testach integracyjnych.

3. **Nowy moduł [`src-tauri/src/community_social.rs`](../src-tauri/src/community_social.rs) (bez naruszania chronionych modułów)**:
   - Chronione moduły `src-tauri/src/community_connector/mod.rs`, `contracts/community.read.v1.json` oraz `src-tauri/src/community_identity/{mod,crypto,vault}.rs` pozostały **nietknięte**.
   - Dla Community 4A (`https://.../api/v1` + `brd_*`) `community.feed.read` deleguje bezpośrednio do `CommunityReader`; dla pozostałych zgodnych społeczności wykonuje read-only `GET` bez poświadczeń, z `RedirectPolicy::none()`, `retry::never()`, limitem 2 MiB streaming i oznaczeniem `untrustedExternalContent: true`.
   - Każde konto społecznościowe generuje i przechowuje własny klucz podpisujący ECDSA P-256 (PKCS#8 DER w `Zeroizing`) oraz opcjonalny token Bearer w `Vault` pod ścisłym `ScopeStamp` (`profile_id`, `connection_id`, `normalized_endpoint`, `credential_revision`).
   - `community.post.publish` wymusza:
     - `enabled == true` i jawne `allowWrites == true` (domyślnie `false`),
     - blokadę tagów prywatnych refleksji (`private_reflection`, `dream`, `self_review`, `journal`, itp.),
     - trwałą barierę `pendingReconciliation` (po zerwaniu połączenia / timeoutcie / HTTP 5xx zapisuje `pendingReconciliation` w `community_accounts` oraz ustawia `recovery_required` w `experimental_instances`),
     - barierę trybu Autonomous (`inactive`, `recovery_required`, nierozliczone `experimental_actions`, sen AI `WakePlan` z `origin == "ai_decision"`),
     - podpis ASN.1 DER base64url (weryfikowalny przez `community_identity::verify_signature`).

4. **Zamknięcie uwagi `P3-A` z Audytu 4B Fix 1**:
   - W [`src-tauri/src/mcp_client.rs`](../src-tauri/src/mcp_client.rs) oraz [`src-tauri/src/community_social.rs`](../src-tauri/src/community_social.rs) uzgodnienie przez `reconcileReceiptRef` wywołuje symetrycznie `maybe_clear_autonomous_recovery`, przywracając `experimental_instances.status` z `recovery_required` na `ready`, gdy nie pozostają żadne nierozliczone `experimental_actions`, `mcp_servers` ani `community_accounts`.

5. **UI i i18n (`CommunityWorkspace.tsx`)**:
   - Rozbudowano [`src/ui/CommunityWorkspace.tsx`](../src/ui/CommunityWorkspace.tsx) oraz [`src/locales/en.json`](../src/locales/en.json) / [`src/locales/pl.json`](../src/locales/pl.json) o zarządzanie połączeniami społeczności (dla profili Standard i Autonomous), wczytywanie publicznego feedu z oznaczeniem niezaufanej treści, publikację podpisanych wpisów oraz baner uzgodnienia (`Confirm reconciled`). Zero `setInterval`/pollingu.

## 2. Otwarte bramki zewnętrzne (nadal OPEN)

- Próby na realnym sprzęcie Windows (zaplanowane po zakończeniu kolejnego etapu / przed finalnym zamknięciem).
- Żywy serwer Community 4A z realnym TLS (poza zakresem testów lokalnych loopback).
