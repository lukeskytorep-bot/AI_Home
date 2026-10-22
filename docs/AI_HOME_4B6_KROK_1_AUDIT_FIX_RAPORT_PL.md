# AI Home 4B-6 krok 1 AUDIT FIX — raport

Data: 22.10.2026
Baza: AI_HOME_4B6_KROK_1_SOURCE.zip
Zakres: wyłącznie A1–A3 z audytu Archive Lifecycle. Bez 4B-6 krok 2.

## A1 — late result po archive → restore
- conversation.archive(true) przed zmianą statusu wyszukuje queued/running operacje powiązane przez chat_runs lub import_materials z daną rozmową.
- Operacje są atomowo oznaczane cancelled; import pending jest domykany; po commit cancellation jest przekazywane do aktywnego runtime.
- chat_runs zapisuje conversationRevision po Accepted.
- chat.ensure_current wymaga zgodności tej rewizji przed finalnym commit. Archive i restore zwiększają revision, więc stary wynik nie może zostać zapisany po ponownym otwarciu.
- Dodano test kontrolowanego POST: archive, natychmiast restore, release odpowiedzi, brak late assistant i fresh send po restore.

## A2 — idempotency lifecycle UI
LifecyclePanel korzysta z mapy LifecycleWrite przechowywanej w Conversations. Przy niejednoznacznym wyniku zachowuje dokładnie ten sam command, scope, payload, expectedRevision i requestId. Response rozlicza pending write; transport exception pozostawia go do retry.

## A3 — Accepted przed readback
Restore profilu i rozmowy wywołuje callback rodzica natychmiast po Accepted, przed list refresh. Błąd odświeżenia jest nadal widoczny jako błąd UI, ale nie blokuje propagacji nowego stanu/revision.

## Weryfikacja lokalna
- check_contracts.py: 193/193 PASS
- scripts/verify-boundaries.mjs: 27/27 PASS
- SQLite: v12, bez migracji
- Rust/Vitest/typecheck/build: wymagają CI
