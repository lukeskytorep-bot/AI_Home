# AI Home 4B-7 AUDIT FIX — raport PL

Data: 23.10.2026

## Baza

Baza poprawki: `AI_HOME_4B7_SOURCE.zip`, SHA-256 `0c24a9b3dbc734b77e556cebdc09da998c34e5370d894e7bc36888c02f41d932`. Zakres ograniczony do A1/A2 z audytu głównego. Nie rozpoczęto 4B-8.

## Poprawki

### A1 — Activity po permanent deletion

`src-tauri/src/activity.rs::project()` rozpoznaje legalny tombstone `permanent_deletion` przed próbą odczytu pól `Operation`. Tombstone jest pomijany, ale `list()` nadal aktualizuje `lastSequence`, więc paging zachowuje postęp. Wszystkie inne malformed `operation.changed` nadal kończą się `STORAGE_ERROR`; nie wprowadzono ogólnego ignore-on-error.

Dodano regresję native w `src-tauri/tests/standard_closure.rs`: legalny successful chat operation event powiązany z rozmową jest redagowany przez prawdziwe `conversation.delete.permanent`; `activity.list` z `limit:1` pomija tombstone i zwraca późniejszy wpis, zachowuje profil B i działa identycznie po ponownym otwarciu Store.

### A2 — odświeżanie profili w Activity

`ActivityPanel` ma osobną generację dla `profile.list`. Refresh jest dostępny bez wyboru profilu, ponawia listę profili, usuwa nieaktualny wybór i dopiero dla nadal istniejącego profilu odświeża Activity. Stale profile responses nie mogą zastąpić nowszej listy.

Dodano testy UI dla: nowego profilu po montowaniu, usunięcia/archiwizacji wybranego profilu z aktywnej listy oraz retry po początkowym błędzie `profile.list`.

## Lokalne bramki

- `python3 check_contracts.py`: 193/193 PASS.
- `node scripts/verify-boundaries.mjs`: 30/30 PASS.
- `python3 scripts/verify-command-parity.py`: 52/52 PASS.
- `python3 scripts/verify-rust-source-text.py`: PASS.
- `python3 scripts/verify-standard-closure.py`: 8/8 PASS.
- schema expectation sweep: PASS; storage target v12.
- Cargo: REQUIRES CI, brak lokalnego toolchainu.
- Vitest/typecheck/Vite: REQUIRES CI, brak lokalnego `node_modules`.

## Werdykt wykonawczy

IMPLEMENTED — REQUIRES FULL CI AND REAUDIT.
