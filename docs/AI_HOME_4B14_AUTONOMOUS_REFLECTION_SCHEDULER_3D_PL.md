# AI Home 4B.14 — Paczka 3D: Produkcyjna refleksja/sen (`Reflect`/`Dream`/`SelfReview`/`SelfWithdrawal`) i natywny wyzwalacz `nextWakeAt` (Autonomous)

Data: 2026-11-26  
Baza repozytorium: `92ab720` (`main`)  
Tryb weryfikacji: Linux headless (`npm test`, `npm run build`, `check_contracts.py`, `verify-*.py`/`mjs`, `cargo test --no-default-features --locked`)

## 1. Zakres zmian (Paczka 3D — kumulatywnie nad 3C)

1. **Produkcyjny `ApiReflectionProvider` i pełny cykl refleksji w `autonomous.wake.check` (Plan 1.99, pkt 6)**
   - W [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs) zaimplementowano `ApiReflectionProvider` (implementacja `experimental_reflection::ReflectionProvider`) wykorzystujący istniejącą warstwę `HttpTransport` + `BoundCredential` z `Vault` oraz dzienny budżet zapytań (`experimental_provider_budgets`).
   - `parse_wake_decision` obsługuje pełny zestaw decyzji autonomicznych: `no_action`, `rest`, `sleep`, `reflect`, `dream`, `review_self`, `withdraw_self`.
   - Gdy model wybierze `reflect` lub `dream`, `ExperimentalRuntime::with_reflection_provider` uruchamia `ApiReflectionProvider` i zapisuje prywatną refleksję w `experimental_private_reflections` (ze ścisłą walidacją `ReflectionOutput` i proweniencją `sourceRefs`).
   - W kolejnym wybudzeniu model otrzymuje prywatny kontekst kandydatów (`privateReviewHints` z `experimental_reflection::context`) wyłącznie w swoim prompcie API i może samodzielnie zatwierdzić, zmodyfikować lub odrzucić kandydata (`review_self`) albo wycofać własny zapis (`withdraw_self`).
   - **Gwarancja prywatności:** odpowiedź `autonomous.instance.status` i UI nadal zawierają wyłącznie beztreściowe liczniki (`privateReflectionCount`, `approvedSelfRecordCount`, `privateContentIncluded: false`) — treść prywatnych refleksji nigdy nie trafia do IPC ani UI. Moduł `experimental_reflection.rs` pozostał bez zależności sieciowych i bez `Vault`.

2. **Natywny jednorazowy wyzwalacz `nextWakeAt` (Plan 1.99, pkt 4 — bez `setInterval` i bez `tokio::time::interval`)**
   - Dodano komendę `autonomous.scheduler.set` w [`contracts/experimental.ipc.json`](../contracts/experimental.ipc.json), [`src-tauri/src/application.rs`](../src-tauri/src/application.rs) i [`src-tauri/src/experimental_pilot.rs`](../src-tauri/src/experimental_pilot.rs) (z pełną idempotencją `receipts`).
   - Domyślnie `autoWakeEnabled: false` (opt-in — aplikacja nigdy nie wykonuje wywołań API w tle bez świadomego włączenia przez użytkownika).
   - `spawn_autonomous_wake_scheduler` (uruchamiany w [`src-tauri/src/community_host/desktop.rs`](../src-tauri/src/community_host/desktop.rs) dla `DesktopRuntime::Standard`) używa wyłącznie jednorazowego `tokio::time::sleep` do najbliższego `nextWakeAt` oraz `tokio::sync::Notify` (zero okresowego pollingu). Gdy żadna instancja nie ma `autoWakeEnabled == true`, task pozostaje w pełni uśpiony na `Notify`. Respektuje pauzę (`inactive`), blokadę `recovery_required`, sen AI oraz dzienny limit wywołań.

3. **UI `AutonomousWorkspace` i lokalizacja EN/PL**
   - W [`src/ui/AutonomousWorkspace.tsx`](../src/ui/AutonomousWorkspace.tsx), [`src/locales/en.json`](../src/locales/en.json) i [`src/locales/pl.json`](../src/locales/pl.json) dodano przełącznik opt-in **Background wake (nextWakeAt): Off / On** oraz licznik `approvedSelfRecordCount` (bez treści prywatnych).

## 2. Testy i weryfikacja

- Rust (`src-tauri/tests/experimental_pilot.rs`): **15/15 PASS**, w tym testy 3D i regresyjne Fix 1:
  - `wake_check_runs_api_reflection_and_self_review_without_leaking_private_content`
  - `autonomous_scheduler_opt_in_fires_only_when_enabled_and_due_and_respects_pause_and_sleep` (weryfikuje bezpieczne minimum 60 s dla schedulera tła)
  - `retry_not_before_blocks_http_and_budget_and_second_step_reflection_failure_returns_resource_unavailable` (`F2/P2` + `P3-1`)
  - `archive_lifecycle_requires_inactive_and_enables_permanent_delete` (`F1/P2`: brak zarchiwizowanego w `items`, obecność w `archivedItems`, odmowa `resume`, czyste `profile.delete.permanent`)
- Frontend (`tests/autonomous-switcher.test.tsx`): asercje dla `Approved self-records`, przełącznika `autonomous.scheduler.set` oraz pełnej ścieżki UI `Pause -> Archive -> Delete permanently`.
- Kontrakty i granice: `check_contracts.py` (193/193), `verify-boundaries.mjs` (41/41 + 14 asercji v17), `verify-command-parity.py` (75/75), wszystkie skrypty `verify-*` (w tym `verify-standard-closure.py`: brak `setInterval` i brak `tokio::time::interval`) oraz `scripts/verify-source.py`.
- Zamrożone kontrakty P2 (`contracts/p2-frozen.sha256.json`), `src-tauri/capabilities/main.json` oraz `Cargo.toml`/`Cargo.lock` pozostały **nienaruszone**.

## 3. Poprawki po audycie Paczki 3D (Fix 1)

- **`F1 / P2` (spójność archiwizacji i trwałego usuwania w Autonomous):**
  - `list_instances()` odfiltrowuje zarchiwizowane profile z `items` i zwraca je osobno w `archivedItems` (z `profileRevision`).
  - `require_instance()` odrzuca operacje na zarchiwizowanym profilu kodem `POLICY_DENIED` (nie można wznowić zarchiwizowanej instancji).
  - `AutonomousWorkspace.tsx` weryfikuje `preferredProfileId` wobec bieżących `items` (nie wywołuje `status` na zarchiwizowanym profilu) i udostępnia sekcję **Archived Autonomous instances** z przyciskiem **Delete permanently** (`profile.delete.permanent`), zachowując pełną izolację od Standard (`connections.rs` bez zmian).
- **`F2 / P2` (bezpieczny interwał schedulera tła, `retryNotBefore` i backoff błędów):**
  - `AutonomousWorkspace.tsx` tworzy instancje z `initialIntervalMs: 60000` (60 s), a `compute_background_wake_due_at` w backendzie dodatkowo egzekwuje twarde minimum `MIN_BACKGROUND_WAKE_INTERVAL_MS = 60_000` dla planów `interval`, co uniemożliwia spalanie budżetu po decyzjach innych niż `sleep`.
  - `precheck_consider_guards`, `wake_check_via_provider` i `next_scheduled_wake` sprawdzają `instance["retryNotBefore"]` przed pobraniem budżetu i wywołaniem HTTP.
  - `spawn_autonomous_wake_scheduler` uruchamia 60 s backoff również wtedy, gdy `outcome.is_none()` (np. błąd/timeout).
- **`P3-1` (spójność błędów drugiego kroku refleksji):**
  - `wake_check_with_reflection` mapuje `Err("RESOURCE_UNAVAILABLE" | "PROVIDER_TIMEOUT")` z `run_due` na `RunResult::ResourceUnavailable` (`ok: true, result: "resource_unavailable"`).

## 4. Otwarte bramki zewnętrzne (OPEN)

- **Próby na realnym Windows** (WebView2, OS Vault, żywy klucz API) — odłożone na koniec zgodnie z ustaleniem użytkownika (GitHub CI i build Windows przeszły na zielono na bazie 3C).
- **Akceptacja przeglądu MCP OSS** (`docs/AI_HOME_4B12_MCP_CLIENT_OSS_REVIEW_PL.md`) — brak nowych zależności MCP w `Cargo.toml`/`Cargo.lock` do czasu decyzji użytkownika/audytora.
- **Community 4A live forum/TLS** — akceptacja wobec zewnętrznego forum pozostaje OPEN.
