# 4B-14 Paczka 2C — idempotencja `receipts`, podpięcie API (`providers/` + `Vault`) z budżetem, uzgadnianie `uncertain` i pauza/wznowienie instancji

Data: 25.11.2026. Baza repozytorium: `92ab720` (`main`, „4b14”). Paczka skumulowana (1 + 2A + P3 z audytu 2A + 2B + 2C), realizująca plan `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL_1.99.md`.

## 1. Zakres zmian Paczki 2C

1. **Idempotencja `receipts` dla mutujących komend `autonomous.*` (zamknięcie `P3-A` z audytu 2B):**
   - W `src-tauri/src/experimental_pilot.rs` mutujące komendy zapisują wynik w tabeli `receipts` po `requestId` + deterministycznym `fingerprint` (dla `autonomous.provider.configure` pole `apiKey` jest hashowane przed wyliczeniem fingerprintu — surowy sekret nigdy nie trafia do `receipts`).
   - Powtórka tego samego `requestId` i `fingerprint` odtwarza zapisany wynik bez powtarzania zapisu ani wywołania sieciowego; konflikt payloadu zwraca `IDEMPOTENCY_CONFLICT`.
2. **Podpięcie trybu Autonomous do warstwy API (`providers/` + `Vault`) z dziennym budżetem wywołań (zamknięcie `P3-B` z audytu 2B):**
   - Nowa komenda `autonomous.provider.configure` w `contracts/experimental.ipc.json` i `src-tauri/src/application.rs`: zapisuje metadane połączenia i dziennego limitu wywołań (`dailyCallLimit`, `maxOutputTokens`) w tabeli `connections` (izolacja po `profile_id`), a klucz API wyłącznie w `Vault::store(scope, credentialRef, secret)`.
   - Ścieżka `autonomous.proposal.consider` bez `simulationOutcome` pobiera klucz z `Vault::secret(...)`, sprawdza i rezerwuje dzienny budżet PRZED wywołaniem sieciowym, wysyła ustrukturyzowane zapytanie przez `HttpTransport::execute_chat(...)` (przekazując agentowi propozycję, rundy, aktywne odmowy i pozostały budżet), a wynik zapisuje przez kanoniczny `ExperimentalRuntime::run_due`.
   - Błąd sieci, timeout lub niepoprawny JSON odpowiedzi zwraca `resource_unavailable` (nigdy fałszywa zgoda ani fałszywa odmowa); przekroczenie limitu zwraca `budget_exhausted` (`policy_guard`, zero wywołań HTTP).
3. **Jawna ścieżka uzgadniania akcji `uncertain` (`autonomous.action.reconcile`):**
   - Pozwala rozliczyć akcję `sent`/`uncertain` jako `confirmed_accepted` lub `confirmed_not_executed` z jawnym dowodem `receiptRef` (walidowane przez zamrożony schemat `ActionRecord`).
   - Aktualizuje powiązaną propozycję, dopisuje rundę `reconciliation` i — gdy brak innych nierozliczonych akcji — zdejmuje blokadę `recovery_required` na instancji.
4. **Techniczna pauza i wznowienie instancji (`autonomous.instance.lifecycle`):**
   - Akcje `pause` (`inactive`) i `resume` (`ready`) bez nadpisywania aktywnych odmów i bez ukrywania nierozliczonych akcji (`POLICY_DENIED` przy aktywnym `recovery_required` lub `sent`/`uncertain`).
   - Zatrzymana instancja zwraca `status: "inactive"` (`policy_guard`) bez wywołania API.
5. **Aktualizacja UI i planu:**
   - `src/ui/AutonomousWorkspace.tsx` + `src/locales/en.json` / `pl.json`: sekcja konfiguracji API i budżetu, przyciski `Pause / Resume`, panel uzgadniania akcji `uncertain`.
   - Zsynchronizowano plan `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` oraz `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL_1.99.md` (z zachowaniem pełnej historii 1.98 i starszej co do bajtu).

## 2. Audit Fix 1 — zamknięcie ustaleń z niezależnego audytu 2C (`F1/P2` i `P3-1`)

- **`F1/P2` (czystość DTO w `connections` / `routes`):** `autonomous.provider.configure` zapisuje w tabeli `connections` wyłącznie walidowany `schema::dto("ProviderConnection", ...)` oraz w `routes` wyłącznie walidowany `schema::dto("ModelRoute", ...)` (w pełni kompatybilne z `export.rs`). Dzienny budżet wywołań (`dailyCallLimit`, `callsUsedToday`, `budgetDay`) jest przechowywany w dedykowanej tabeli `experimental_provider_budgets` (objętej przez `profile_deletion::delete_rows` i `PRAGMA foreign_key_check`).
- **`F1/P2` (pełna koperta w `receipts` i tombstoning przy `profile.delete.permanent`):** `save_receipt` zapisuje pełną kopertę IPC `{"contractVersion":1,"requestId":...,"ok":true,"result":...}` bez cichego `INSERT OR IGNORE`, a każdy mutujący wynik `autonomous.*` zawiera `profileId`. Dzięki temu `deletion::tombstone_profile_receipts` automatycznie zamazuje wszystkie paragony `autonomous.*` usuniętego profilu, a ponowne odtworzenie zamazanego `requestId` zwraca `POLICY_DENIED`. Pokryte testem regresyjnym `permanent_profile_deletion_tombstones_autonomous_receipts_and_connections_keep_strict_dtos`.
- **`P3-1` (walidacja URL Custom OpenAI w UI):** dodano tę samą walidację adresu `custom_openai` w `src/ui/AutonomousWorkspace.tsx` przed wysłaniem IPC, co w `FirstConversation.tsx`.

## 3. Granice potwierdzenia

- Wszystkie testy (w tym test adaptera API z lokalnym serwerem HTTP `127.0.0.1` i `Vault` w pamięci) wykonano na **Linux headless**.
- Nadal **OPEN**: Windows CI (`build_desktop=true`), próby użytkowe na Windows / WebView2 / OS Vault / rzeczywistym kluczu API, odbiór live forum Community 4A / TLS, obowiązkowy przegląd OSS przed MCP oraz przeglądarka.

