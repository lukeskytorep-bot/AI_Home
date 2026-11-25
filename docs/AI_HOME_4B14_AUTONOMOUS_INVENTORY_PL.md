# 4B-9–11 → 4B-14 — mapa komponentów natywnych i braków dla trybu Autonomous (Autonomiczny)

Data: 25.11.2026. Zakres: inwentaryzacja wymagana przez sekcję 4 planu `AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (treść 1.98) przed uruchomieniem lokalnego pilotażu bez forum.

## 1. Nazewnictwo (UI vs identyfikatory techniczne)

- **Nazwa produktowa / UI:** **Autonomous** (EN) / **Autonomiczny** (PL).
- **Identyfikatory techniczne (bez zmian w tej paczce):** tabele SQLite `experimental_*`, moduły Rust `src-tauri/src/experimental_*.rs`, schemat `contracts/experimental.schema.json` oraz skrypty `scripts/verify-experimental-*.py` zachowują dotychczasowe nazwy techniczne, aby nie naruszać istniejących migracji (SQLite v16), zamrożonych kontraktów P2 (gdzie historycznie występuje pole `mode: "conscious"`) ani 100+ statycznych asercji weryfikacyjnych.

## 2. Co już istnieje natywnie (zweryfikowane na `92ab720`)

| Warstwa | Istniejące artefakty | Zweryfikowany zakres |
| --- | --- | --- |
| Storage SQLite (v14–v16) | `src-tauri/src/persistence.rs` (17 tabel `experimental_*`) | Instancje (`experimental_instances`), plany wybudzeń (`experimental_wake_plans`), decyzje (`experimental_decisions`), odmowy (`experimental_refusals`), epizody pracy (`experimental_work_episodes`), techniczne i semantyczne checkpointy, ledger akcji (`experimental_actions`), kursory zdarzeń, 4 tabele stanu wewnętrznego (`internal_state_*`), 4 tabele refleksji/snu (`reflection_runs`, `private_reflections`, `self_reviews`, `self_records`). Odzyskiwanie po restarcie (`running -> interrupted`, `sent -> uncertain`). |
| Kontrakty DTO | `contracts/experimental.schema.json`, `src/domain/schema.ts`, `src-tauri/src/domain/schema.rs` | Walidacja DTO dla wszystkich rekordów Experimental po stronie Rust i TypeScript. |
| Silnik pętli i decyzji | `src-tauri/src/experimental_runtime.rs` | `ExperimentalRuntime`: `create_instance`, `run_due` (`claim`, ocena `WakePlan` `interval`/`deadline`/`event_only`, blokada `fence` przy nieuzgodnionych akcjach `sent`/`uncertain`, ochrona `runtimeGeneration`), `DecisionChoice` (`Action`, `NoAction`, `Rest`, `Sleep`, `WaitEvent`, `Refusal`, `Defer`, `Abandon`, `Continue`, `Revise`, `Reflect`, `Dream`, `ReviewSelf`, `WithdrawSelf`). |
| Ciągłość pracy | `src-tauri/src/experimental_continuity.rs` | `WorkContinuityService`: `preflight_context`, `create_episode`, checkpointy techniczne i semantyczne, `build_resume_packet` / `latest_resume_packet`. |
| Stan wewnętrzny | `src-tauri/src/experimental_internal_state.rs` | `InternalStateService`, `NullStateProvider`, `ConsciousCoreAdapter`, neutralne zdarzenia i `WakeCandidate`. |
| Refleksja i sen | `src-tauri/src/experimental_reflection.rs` | `ReflectionProvider`, prywatne notatki, `SelfReview`, `SelfWithdrawal`, redakcja i usuwanie w `deletion.rs` oraz `profile_deletion.rs`. |
| Ochrona izolacji | `src-tauri/src/connections.rs`, `src-tauri/src/community_host/local.rs`, `src-tauri/src/community_backup.rs` | Profile powiązane z `experimental_instances` są wykluczane ze ścieżek `Standard` i z obecnej rejestracji Community 4A; aktywne instancje blokują destrukcyjne operacje. |
| Testy | 62 testy integracyjne Rust (`experimental_*`) + 7 testów TS + 5 skryptów `verify-experimental-*` | Wszystkie przechodzą na Linux headless (662/662 łącznie w Rust). |

## 3. Braki w produkcyjnym podłączeniu (host / IPC / UI)

1. **Brak suplementarnego kontraktu i routingu IPC (`contracts/experimental.ipc.json`):**
   - `src-tauri/src/application.rs` i `src-tauri/src/connections.rs` nie udostępniają jeszcze komend IPC dla tworzenia/listowania instancji Autonomous, składania i wersjonowania propozycji, rund negocjacji, odczytu trwałych decyzji/odmów/planu wybudzenia ani kontrolowanego uruchomienia impulsu.
   - Zgodnie z `src-tauri/capabilities/main.json` nowe komendy powinny iść przez istniejące `home_dispatch` (bez rozszerzania uprawnień Tauri ACL).
2. **Brak trwałego modelu propozycji i negocjacji (wymaganego do lokalnego pilotażu):**
   - Obecne `OpportunityEvent` i `RefusalRecord` (`scopeFingerprint`, `reconsiderationConditions`) pokrywają zdarzenia i odmowy, ale nie przechowują jeszcze pełnej historii propozycji z jawną tożsamością, numerem wersji (`version`), warunkami i rundami negocjacji (dopytanie, odłożenie, zmiana warunków, ponowienie tej samej wersji po restarcie).
   - Wymaga to dedykowanych tabel w kolejnej migracji SQLite (**v17**) wraz z pokryciem w `deletion.rs`, `profile_deletion.rs`, `diagnostics.rs` (content-free) oraz aktualizacją asercji migracji sprawdzanych przez `scripts/verify-boundaries.mjs`.
3. **Brak produkcyjnego `DecisionProvider` (i oddzielenia błędów wykonania od odmowy):**
   - `experimental_runtime.rs` udostępnia obecnie wyłącznie zaślepki `DisabledDecisionProvider` oraz `DisabledActionExecutor`.
   - Lokalny pilotaż wymaga adaptera `DecisionProvider` korzystającego z istniejącego `src-tauri/src/providers/` i `Vault` (dla przypisanej trasy Primary instancji Autonomous) oraz ścisłego zachowania semantyki: brak uprawnień, awaria sieci/providera lub niepewny wynik narzędzia są stanami wykonania, nigdy odmową agenta; przy aktywnej odmowie tej samej wersji propozycji lub podczas wybranego snu (`WakePlan` `deadline`/`event_only`) model nie jest wywoływany bez kwalifikującej się zmiany warunków.
4. **Brak interfejsu użytkownika (częściowo rozpoczęte w Paczce 2A):**
   - Paczka 2A dodaje górny przełącznik przestrzeni roboczej **Standard / Autonomous** (`Standard / Autonomiczny`) oraz uczciwy ekran statusu niedostępności (bez ingerencji w profile Standard).
   - Paczka 2B podłączy ekran lokalnego pilotażu bez forum (propozycja → rozpatrzenie → opcjonalna negocjacja → decyzja → restart → ponowiona lub zmieniona propozycja + podgląd aktywnego `WakePlan` i trwałych odmów).

## 4. Plan kolejnego kroku (Paczka 2B — lokalny pilotaż bez forum)

1. Zamrozić suplementarny kontrakt `contracts/experimental.ipc.json` i dodać migrację SQLite v17 dla propozycji/wersji/rund negocjacji (`experimental_proposals`, `experimental_proposal_rounds` lub równoważne, w pełni zintegrowane z `deletion.rs` i `profile_deletion.rs`).
2. Dodać natywny serwis pilotażu w Rust spinający propozycje z `ExperimentalRuntime` (`DecisionRecord`, `RefusalRecord`, `WakePlan`) — z obsługą zarówno deterministycznego testowego `DecisionProvider`, jak i podpięcia pod skonfigurowaną trasę Primary.
3. Dodać ekran lokalnego pilotażu w `src/ui/AutonomousWorkspace.tsx` (z zachowaniem twardego rozdziału: człowiek proponuje lub zmienia warunki, nigdy nie nadpisuje odmowy poleceniem ani nie skraca wybranego snu).
4. Zweryfikować testami Rust + Vitest: odmowa tej samej wersji po restarcie, zmiana warunków jako nowa wersja wymagająca nowej decyzji, brak zgody przez milczenie, brak podwójnego wykonania po restarcie, poszanowanie `WakePlan` i oddzielenie awarii wykonania od odmowy agenta.
