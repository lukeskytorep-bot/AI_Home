# AI Home 4B-6 krok 4 — AUDIT FIX D1/D2

Data: 23 października 2026.

## Zakres

Ograniczona poprawka dwóch P1 z głównego audytu permanent deletion. Nie rozpoczyna 4B-7, nie zmienia frozen P2 i nie dodaje migracji. SQLite pozostaje v12.

## D1 — logiczne usunięcie treści rozmowy z aktywnej bazy

- Dodano wspólny moduł `src-tauri/src/deletion.rs`.
- `conversation.delete.permanent` redaguje `profile_events`, których payload strukturalnie wskazuje usuwaną rozmowę.
- Zachowane zostają sequence/event metadata, ale payload jest zastępowany beztreściowym tombstone `{"tombstone":true,"reason":"permanent_deletion"}`.
- Pełna treść wiadomości, title i draft nie pozostają w logicznie dostępnych event payloads ani w powiązanych historycznych receipts.
- Nie deklaruje się fizycznego secure erase sektorów, WAL, backupów ani wcześniej wyeksportowanych ZIP-ów.

## D2 — receipts i idempotency bez substringowego purge

- Usunięto oba `DELETE FROM receipts WHERE instr(response,?)>0`.
- Nie ma `LIKE`/substringowego dopasowania target ID w receipt response.
- Historyczne receipts są kwalifikowane strukturalnie przez semantyczne pola `profileId`, `conversationId` lub właściwy DTO `id`.
- Powiązany receipt zachowuje `request_id` i `fingerprint`, a jego response staje się terminalnym `PERMANENTLY_DELETED` bez starej treści.
- Dispatcher rozpoznaje taki tombstone: exact requestId+payload nie wykonuje starej mutacji ponownie; inny payload nadal kończy się `IDEMPOTENCY_CONFLICT`.
- Receipt profilu B, którego zwykły title zawiera ID profilu A, pozostaje nienaruszony i jego replay zwraca poprzedni wynik bez duplikacji.
- Bieżący receipt komendy permanent delete jest nadal normalnym success receipt, ponieważ zostaje zapisany dopiero po wykonaniu purge; exact replay samego delete zwraca poprzedni success.

## Regresje

Do `src-tauri/tests/lifecycle.rs` dodano przypadki:
- message/title/draft marker nie pozostają po permanent conversation delete;
- event sequence pozostaje, payload jest tombstone;
- historyczny conversation.create po purge zwraca `PERMANENTLY_DELETED` zamiast odtwarzać rozmowę;
- zmieniony payload z tym samym requestId daje `IDEMPOTENCY_CONFLICT`;
- foreign receipt profilu B z tekstem zawierającym ID profilu A nie jest zmieniany;
- replay B nie tworzy drugiej rozmowy;
- historyczny profile.create A po purge jest terminalnie zablokowany.

## Kontrole lokalne

- P2: 193/193 PASS.
- boundaries: 29/29 PASS.
- contract/handler/router parity: 51/51 PASS.
- Rust source-text normalization: PASS.
- substring receipt purge sweep: PASS, zero `instr(response...)` / `LIKE` cleanup.
- schema expectation sweep: storage target pozostaje v12.
- Cargo/native runtime: wymaga CI, lokalny toolchain Rust niedostępny.

Po regeneracji manifestów wymagane są `verify-source.py`, overlay reconstruction i pełne GitHub Actions + re-audyt.
