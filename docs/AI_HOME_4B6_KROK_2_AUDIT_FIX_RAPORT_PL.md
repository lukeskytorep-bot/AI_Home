# AI Home 4B-6 krok 2 — AUDIT FIX — raport wykonawczy

Data: 22 października 2026

## Zakres
Poprawka wykonana na `AI_HOME_4B6_KROK_2_SOURCE.zip`. Nie rozpoczyna 4B-6 kroku 3. Obejmuje R1–R4 oraz U1–U2 z audytu backup/restore.

## Zmiany
- Restore ma trwały, zewnętrzny wobec przywracanej bazy ledger `requestId + backupId + wynik`. Replay po restarcie zwraca ten sam receipt; inny backup pod tym samym requestId daje konflikt.
- Metadata restore są publikowane jako create-once temp+fsync+rename; istniejących request/result sidecarów nie usuwa się przed replace.
- Po Accepted cała aplikacja przechodzi w `restart-required` write freeze. Mutujące komendy są odrzucane, read-only pozostają dostępne. Queued/running operations są oznaczane cancelled przed końcowym safety snapshotem, a aktywne transporty otrzymują cancellation.
- Safety backup jest tworzony przez SQLite `VACUUM INTO` po zamrożeniu operacji, więc obejmuje logiczny stan także przy WAL.
- Startup restore jest fail-closed. Błędu nie połyka się i nie usuwa pending. `Store::open(create_if_missing=true)` nie jest wykonywane po błędzie restore.
- WAL/SHM nie są ręcznie kasowane. Po zweryfikowanym safety SQLite wykonuje checkpoint przez API przed switchem.
- Swap używa staging i zachowuje `previous`; przy brakującym/uszkodzonym current najpierw odtwarza się verified safety. Jeżeli proces padnie po instalacji nowej bazy, kolejny start rozpoznaje target po SHA i tylko domyka receipt.
- `BackupPanel` zachowuje requestId create/restore w app-scoped stanie modułu przez remount i pokazuje write-freeze/restart-required.

## Testy dodane/zaktualizowane
- Accepted restore blokuje późniejsze mutacje.
- Restore + restart + replay tego samego requestId nie tworzy drugiego pending.
- Ten sam requestId z innym backupId daje `IDEMPOTENCY_CONFLICT`.
- Brak `foundation.sqlite3` przy pending restore jest odtwarzany z safety przed kontynuacją.
- Symulacja przerwania po `current -> previous` odzyskuje safety i kończy restore.
- Uszkodzone źródło restore powoduje fail-closed i zachowuje bieżącą bazę.
- Frontend zachowuje requestId restore przez remount po niejednoznacznym transporcie.

## Kontrole lokalne
- Frozen P2: 193/193 PASS.
- Boundaries: 28/28 PASS.
- Schema expectation sweep: PASS, brak docelowych oczekiwań `user_version == 11`.
- Contract/handler/router parity: PASS.
- Zakaz ręcznego kasowania `foundation.sqlite3-wal/-shm`: PASS.
- Cargo/Rust: NOT RUN, brak `cargo` w środowisku.
- TypeScript/Vite/Vitest: NOT RUN do końca, brak lokalnych typów `vite/client` i `vitest/globals`.

## Status
IMPLEMENTED / REQUIRES FULL CI AND RE-AUDIT. Nie nadawać 4B-6 krok 2 PASS CLOSED przed pełnym Cargo, frontend CI, fault-injection/restart smoke i ponownym audytem.
