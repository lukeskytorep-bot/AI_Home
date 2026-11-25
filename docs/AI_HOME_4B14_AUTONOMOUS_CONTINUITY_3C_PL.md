# AI Home 4B.14 — Paczka 3C: Ciągłość pracy, archiwizacja instancji i wybudzanie API (Autonomous)

Data: 2026-11-25  
Baza repozytorium: `92ab720` (`main`)  
Tryb weryfikacji: Linux headless (`npm test`, `npm run build`, `check_contracts.py`, `verify-*.py`/`mjs`, `cargo test --no-default-features --locked`)

## 1. Zakres zmian (Paczka 3C — kumulatywnie nad 3B)

1. **Wybudzanie z użyciem skonfigurowanego providera API (`autonomous.wake.check` — domknięcie uwagi audytu 2C `P3-2`)**
   - W [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs) dodano obsługę wywołania modelu API (`wake_check_via_provider`, `parse_wake_decision`, `is_wake_due`) w `autonomous.wake.check`, gdy:
     - instancja jest w stanie `ready`,
     - termin `nextWakeAt` z aktywnego `WakePlan` już upłynął (gdy AI śpi / `nextWakeAt` jest w przyszłości, zwracany jest `due: false`, `reason: "sleeping"` **bez** żadnego wywołania API),
     - skonfigurowany jest provider API + poświadczenie w Vault i dostępny jest dzienny budżet zapytań (`experimental_provider_budgets`).
   - W przypadku wyczerpania dziennego budżetu zwracany jest jawny status `reason: "budget_exhausted"` (dodany w [`contracts/experimental.ipc.json`](../contracts/experimental.ipc.json)).
   - Odpowiedź AI może ustawić nowy `WakePlan` (np. `sleep` z własnym `nextWakeAt`, `interval` lub `idle`) z `origin: "ai_decision"` — człowiek nadal nie może skrócić snu AI.

2. **Beztreściowe podsumowanie ciągłości pracy i prywatności w `autonomous.instance.status` + kontekst techniczny dla modelu**
   - W [`contracts/experimental.ipc.json`](../contracts/experimental.ipc.json) i [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs) dodano obiekt `continuitySummary` (wyłącznie liczniki i flagi techniczne: `activeEpisodeCount`, `hasResumePacket`, `privateReflectionCount`, `approvedSelfRecordCount`, `privateContentIncluded: false`).
   - Treść prywatnych refleksji (`experimental_private_reflections`) nigdy nie opuszcza bazy ani nie trafia do UI.
   - Adapter decyzji API (`consider_via_provider`) może dołączyć techniczny nagłówek aktywnego pakietu wznowienia pracy (`activeWorkEpisode` z `WorkContinuityService::latest_resume_packet`), bez włączania prywatnych refleksji.

3. **Archiwizacja instancji (`autonomous.instance.lifecycle` z `action: "archive"`)**
   - Dodano akcję `"archive"` w [`contracts/experimental.ipc.json`](../contracts/experimental.ipc.json) i [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs).
   - Archiwizacja wymaga wcześniejszego zatrzymania instancji (`status == "inactive"`) — nie można zarchiwizować aktywnej instancji.
   - Operacja archiwizuje powiązany wiersz w `profiles` (`status = 'archived'`), zwiększa `revision` i zwraca `profileRevision`, umożliwiając późniejsze trwałe usunięcie przez standardowy mechanizm `profile.delete` (wraz z tombstoningiem `receipts` i wyczyszczeniem wszystkich 20 tabel `experimental_*`).

4. **UI `AutonomousWorkspace` i lokalizacja EN/PL**
   - W [`src/ui/AutonomousWorkspace.tsx`](../src/ui/AutonomousWorkspace.tsx) dodano:
     - kartę `continuitySummary` (bez treści prywatnych),
     - przycisk `Archive instance` / `Archiwizuj instancję` (widoczny wyłącznie, gdy instancja ma status `inactive`),
     - rozwijaną listę ostatnich decyzji (`recentDecisions`) oraz lokalizację w [`src/locales/en.json`](../src/locales/en.json) i [`src/locales/pl.json`](../src/locales/pl.json).

## 2. Testy i weryfikacja

- Rust (`src-tauri/tests/experimental_pilot.rs`): 12/12 testów PASS, w tym 2 nowe testy 3C:
  - `wake_check_uses_configured_api_provider_and_respects_ai_sleep_without_extra_calls`
  - `archive_lifecycle_requires_inactive_and_enables_permanent_delete`
- Frontend (`tests/autonomous-switcher.test.tsx`): zaktualizowany mock i asercje dla `continuitySummary` oraz przycisku `Archive instance` w stanie `inactive`.
- Kontrakty i granice: `check_contracts.py`, `verify-boundaries.mjs`, `verify-command-parity.py`, wszystkie skrypty `verify-*` i `scripts/verify-source.py`.
- Zamrożone kontrakty P2 (weryfikowane przez `contracts/p2-frozen.sha256.json`) oraz `src-tauri/capabilities/main.json` pozostały **nienaruszone**.

## 3. Otwarte bramki zewnętrzne (OPEN)

- **Windows CI (`build_desktop=true`)** i ręczne próby na Windows (WebView2, OS Vault, prawdziwy klucz API) — wymagają uruchomienia na hoście Windows.
- **Przegląd MCP OSS (`docs/AI_HOME_4B12_MCP_CLIENT_OSS_REVIEW_PL.md`)** — brak nowych zależności MCP w `Cargo.toml`/`Cargo.lock` do czasu akceptacji przeglądu przez użytkownika/audytora.
- **Community 4A live forum/TLS** — akceptacja wobec zewnętrznego forum pozostaje OPEN.
