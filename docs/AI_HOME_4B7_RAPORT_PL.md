# AI Home 4B-7 — Domknięcie Standard — raport PL

Data: 23.10.2026

## Baza

Baza implementacji: `AI_HOME_4B6_KROK_4_AUDIT_FIX_4_SOURCE.zip`, po potwierdzonym przez Edwarda zielonym GitHub Actions. SQLite pozostaje w wersji 12. Frozen P2 pozostaje bez zmian.

## Zakres wykonany

1. Wdrożono istniejący frozen `activity.list` jako profile-scoped odczyt historii trwałych `operation.changed`.
2. Dodano `src-tauri/src/activity.rs`; projekcja do DTO `Activity` nie zawiera message content, credentialRef, sekretów ani raw provider response.
3. Dodano panel Activity EN/PL z wyborem profilu, odświeżaniem, pagingiem i generation/scope guardem na spóźnione odpowiedzi.
4. Activity jest dostępne z głównej nawigacji Standard.
5. Dodano native acceptance test: historia Activity dwóch profili nie miesza się i przeżywa restart.
6. Dodano UI acceptance test: samo otwarcie Standard i 60 s bezczynności nie inicjuje Primary/Scribe, importu, memory update/restore ani connection test.
7. Dodano `scripts/verify-standard-closure.py`: Activity route/UI, NullState, brak interval schedulers i brak runtime package dependency od forum/community.
8. Command parity rozszerzono o frozen `ipc.catalog.json::activity.list`.

## Lokalne bramki

- `python3 check_contracts.py`: 193/193 PASS.
- `node scripts/verify-boundaries.mjs`: 30/30 PASS.
- `python3 scripts/verify-command-parity.py`: 52/52 PASS.
- `python3 scripts/verify-rust-source-text.py`: PASS.
- `python3 scripts/verify-standard-closure.py`: 6/6 PASS.
- schema expectation sweep: 0 starych docelowych asercji `user_version == 11`; SQLite v12.
- Cargo: REQUIRES CI, brak lokalnego toolchainu Rust.
- Vitest/typecheck/Vite: REQUIRES CI, brak lokalnego `node_modules`; globalny tsc nie ma `vite/client` i `vitest/globals`.

## Kryteria 4B-7

- Rozmowy, pliki, pamięć i wiele profili pozostają niezależne od forum: zachowane z wcześniejszych etapów, bez dodania zależności forum/community.
- Brak okresowych wywołań głównego modelu tylko z powodu otwartej aplikacji: nowy test acceptance + static guard.
- Widoczna historia aktywności: zaimplementowana profile-scoped i trwała po restarcie.

## Poza zakresem

4B-8 diagnostics/About/Privacy, Community/A2A, ConsciousCore/Life Loop, release smoke Windows/WebView oraz live provider/vault acceptance.

## Werdykt wykonawczy

IMPLEMENTED — REQUIRES FULL CI AND AUDIT.
