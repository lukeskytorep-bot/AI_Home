# AI Home 4B-12/13 — Paczka 4D: Warstwa Skills per profil (No Self-Grant) + Zamknięcie P3-1 / P3-2

**Data:** 2026-11-27  
**Zakres:** Paczka 4D (krok 3 z planu 4B-12/13) — deklaratywne skille per profil ze ścisłą polityką no-self-grant, SQLite v20 (`profile_skills`), kontrakt IPC `skills.ipc.json`, panel UI `SkillsPanel` w Ustawieniach oraz zamknięcie obu obserwacji P3 (`P3-1` i `P3-2`) z audytu Paczki 4C.

## 1. Zrealizowane zmiany

1. **Zamknięcie uwag P3 z Audytu 4C:**
   - **`P3-1`:** W [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs) metoda `reconcile_action` przed zdjęciem statusu `recovery_required` z `experimental_instances` sprawdza teraz kompletną trójkę tabel (`experimental_actions`, `mcp_servers`, `community_accounts`) — pełna symetria z `McpService` i `CommunitySocialService`.
   - **`P3-2`:** W [`src-tauri/src/community_social.rs`](../src-tauri/src/community_social.rs) metoda `require_active_profile` odrzuca profile o statusie `"archived"` kodem `POLICY_DENIED` (identycznie jak `McpService`).

2. **Kontrakt IPC [`contracts/skills.ipc.json`](../contracts/skills.ipc.json) i parytet komend (`89/89`):**
   - `skills.list` (read-only)
   - `skills.save` (mutacja, pełna idempotencja `receipts`)
   - `skills.delete` (mutacja, pełna idempotencja `receipts`)
   - `skills.policy.check` (read-only, deterministyczna ocena uprawnień)
   - Routing i strażnik `restore_pending_for_store` w [`src-tauri/src/application.rs`](../src-tauri/src/application.rs), eksport w [`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs).

3. **Moduł [`src-tauri/src/skills.rs`](../src-tauri/src/skills.rs) — twarde inwarianty bezpieczeństwa:**
   - **No self-grant:** zaimportowany skill (`source: "imported"`) nigdy nie może zostać zapisany jako `enabled: true` ani z niepustym `grantedCapabilities` (`POLICY_DENIED`). Pole `requestedCapabilities` jest wyłącznie informacyjne i nigdy nie nadaje dostępu; dostęp daje wyłącznie jawne `grantedCapabilities` zatwierdzone przez użytkownika (`source: "user"`) ze ścisłego katalogu `ALLOWED_CAPABILITIES`.
   - **Zero bezpośredniej sieci / Vault / shella:** skill jest deklaratywnym pakietem instrukcji per profil.
   - **Ochrona prywatnej refleksji:** `skills.policy.check` odrzuca tagi prywatnych refleksji (`private_reflection`, `dream`, `self_review`, `journal` itd.) niezależnie od nadanych uprawnień.
   - **Bariera trybu Autonomous:** mutujące uprawnienia (`mcp_mutating`, `community_publish`) są blokowane podczas pauzy (`inactive`), `recovery_required`, nierozliczonych akcji (`experimental_actions` / `mcp_servers` / `community_accounts`) oraz snu AI (`WakePlan` z `origin == "ai_decision"`).

4. **SQLite v20 i sprzątanie profilu:**
   - Podniesiono schemat SQLite do `CURRENT_SCHEMA = 20` w [`src-tauri/src/persistence.rs`](../src-tauri/src/persistence.rs) z tabelą `profile_skills(profile_id, id, revision, body)`.
   - Dodano `"profile_skills"` do [`src-tauri/src/profile_deletion.rs`](../src-tauri/src/profile_deletion.rs) (`profile.delete.permanent` czyści skille i tombstonuje `receipts`).

5. **UI i i18n:**
   - Utworzono [`src/ui/SkillsPanel.tsx`](../src/ui/SkillsPanel.tsx) i osadzono w Ustawieniach w [`src/ui/App.tsx`](../src/ui/App.tsx) (obsługa profili Standard i Autonomous, rozróżnienie `Requested (untrusted)` vs `Granted`, jawne włączanie/wyłączanie/usuwanie, zero `setInterval`).
   - Pełny parytet kluczy w [`src/locales/en.json`](../src/locales/en.json) i [`src/locales/pl.json`](../src/locales/pl.json).

## 2. Otwarte bramki zewnętrzne (OPEN)

- Próby na realnym sprzęcie Windows (zaplanowane po Paczce 4E — synchronizacja planu `2.00` + checklista testów Windows).
- Żywy serwer Community 4A z realnym TLS.
