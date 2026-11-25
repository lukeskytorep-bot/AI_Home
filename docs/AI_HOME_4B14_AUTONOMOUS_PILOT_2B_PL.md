# 4B-14 Paczka 2B — lokalny pilotaż trybu Autonomous (Autonomiczny) bez forum + poprawki P3 z audytu 2A

Data: 25.11.2026. Baza repozytorium: `92ab720` (`main`, „4b14”). Zakres obejmuje skumulowane zmiany Paczki 2A + poprawek P3 z niezależnego audytu 2A + Paczki 2B (lokalny pilotaż bez forum zgodnie z sekcją 4 planu `AI_HOME_4B_ETAPY_PRACY_v1.0_PL_1.98.md`).

## 1. Zamknięte uwagi nieblokujące z niezależnego audytu Paczki 2A (P3)

1. **P3-1 (`src/ui/FirstConversation.tsx`):** uszczelniono walidację adresu Custom OpenAI o odrzucenie znaków kontrolnych (`[\u0000-\u001f\u007f]` sprawdzanych przed `.trim()`) oraz pustego `?` / `#` (`parsed.href.includes('?') || parsed.href.includes('#')`). Pokryte testem regresyjnym w `tests/first-conversation.test.tsx`.
2. **P3-2 (`src/ui/FirstConversation.tsx`):** przycisk `Back` (`onCancel`) czyści `localStorage.removeItem(setupStorageKey)` przed wyjściem z kreatora, dzięki czemu po nieudanym teście połączenia i restarcie kreator nie otwiera się ponownie automatycznie. Pokryte testem regresyjnym w `tests/first-conversation.test.tsx`.
3. **P3-3 (`src/ui/App.tsx`):** przejście z lewego menu do `Activity` (należącego do przestrzeni Standard) przełącza stan `workspace` z powrotem na `'standard'`, zachowując spójność górnego przełącznika `Standard / Autonomous`. Pokryte testem w `tests/autonomous-switcher.test.tsx`.

## 2. Zakres Paczki 2B — lokalny pilotaż Autonomous bez forum

- **Migracja SQLite v16 → v17 (`src-tauri/src/persistence.rs`, `src-tauri/src/profile_deletion.rs`):**
  - Dodano tabele `experimental_proposals` oraz `experimental_proposal_rounds` z kluczami obcymi na `experimental_instances` i pełnym pokryciem w `profile_deletion::delete_rows`.
  - Zaktualizowano `CURRENT_SCHEMA = 17`, `PRAGMA user_version=17`, asercje migracji w testach Rust oraz strażniki wersji schematu w `scripts/verify-experimental-*.py`.
- **Suplementarny kontrakt IPC (`contracts/experimental.ipc.json`) i routing przez `home_dispatch` (`src-tauri/src/application.rs`):**
  - Dodano 7 komend bez rozszerzania uprawnień Tauri ACL (`src-tauri/capabilities/main.json` bez zmian) i bez modyfikacji zamrożonych kontraktów P2:
    `autonomous.instance.list`, `autonomous.instance.create`, `autonomous.instance.status`, `autonomous.proposal.submit`, `autonomous.proposal.negotiate`, `autonomous.proposal.consider`, `autonomous.wake.check`.
  - Zarejestrowano `contracts/experimental.ipc.json` w `scripts/verify-command-parity.py` (71/71).
- **Natywny serwis pilotażu (`src-tauri/src/experimental_pilot.rs`):**
  - Korzysta bezpośrednio z `ExperimentalRuntime` (`DecisionRecord`, `RefusalRecord`, `WakePlan`, `ActionRecord`, `WorkEpisode`, `runtimeGeneration`, fence na `sent`/`uncertain`).
  - Egzekwuje natywnie wszystkie inwarianty sekcji 4 planu 1.98:
    - trwała tożsamość i numer wersji propozycji (`v1`, `v2`, …),
    - aktywna odmowa (`RefusalRecord`) dla danej wersji przetrwa restart i blokuje ponowne wykonanie oraz nadpisanie poleceniem człowieka,
    - zmienione warunki tworzą nową wersję (`v+1`) wymagającą nowej, zapisanej decyzji agenta (bez kasowania historycznych odmów),
    - brak podwójnego wykonania tej samej wersji (również po restarcie),
    - milczenie (`no_action`, `rest`, `sleep`, `RESOURCE_UNAVAILABLE`) nie jest zgodą,
    - sen / `WakePlan` wybrany przez agenta nie jest skracany przez ludzką propozycję,
    - awaria wykonania (`execution_failed`) oraz niepewny wynik (`execution_uncertain`) są oddzielone od odmowy agenta (nie tworzą `RefusalRecord`; `uncertain` aktywuje blokadę `reconciliation_required` także po restarcie),
    - izolacja profili Standard (`POLICY_DENIED` dla profili spoza `experimental_instances`).
- **Interfejs lokalnego pilotażu (`src/ui/AutonomousWorkspace.tsx`, `src/locales/en.json`, `src/locales/pl.json`, `src/ui/style.css`):**
  - W trybie desktop: tworzenie odizolowanych instancji, podgląd aktywnego `WakePlan`, aktywnych odmów i nierozliczonych akcji, składanie propozycji, rundy negocjacji, zmiana warunków (nowa wersja) oraz jawnie oznaczona ścieżka decyzji pilotażowej (symulacja lub skonfigurowany provider), z wyraźnym oznaczeniem `Local pilot · No forum connected`.
  - W trybie przeglądarkowym: nadal brak wywołań `autonomous.*` i uczciwy komunikat o wymaganiu środowiska desktopowego.

## 3. Granice potwierdzenia (czego NIE wykonano w tym środowisku)

- Wszystkie testy wykonano w środowisku **Linux headless** (bez Windows, MSVC, WebView2, systemowego magazynu haseł OS Vault i bez forum na żywo).
- Bramki **Windows CI (`build_desktop=true`)**, **Windows próby użytkowe**, **Community 4A acceptance**, **MCP (po przeglądzie OSS)** oraz **przeglądarka (odłożona)** pozostają **OPEN**.
