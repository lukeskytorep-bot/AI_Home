# AI Home 4B-6 krok 3 — raport wdrożenia

## Zakres

Wykonano portable export profilu na bazie `AI_HOME_4B6_KROK_2_AUDIT_FIX_4_SOURCE.zip`. Krok nie dodaje migracji SQLite i nie rozpoczyna permanent deletion ani 4B-7.

## Implementacja

Dodano suplement `contracts/export.ipc.json` z komendami `profile.export.create`, `profile.export.list` i `profile.export.read`. Eksport jest profile-scoped i powstaje jako ZIP w katalogu zarządzanym przez AI Home. UI nie otrzymuje ścieżki systemowej. ZIP może być pobrany przez chunked `profile.export.read`.

Eksport jest budowany z jednej transakcji read-snapshot. Zawiera:

- `manifest.json` z listą plików, rozmiarem i SHA-256 każdego wpisu,
- `README.txt`,
- `conversations.txt`,
- osobne pliki Markdown dla rozmów w kolejności wiadomości,
- strukturalne JSON dla profilu, rozmów, raw messages, Context Segments, pamięci i version links, annotations/metadata, importów, attachments, knowledge i persistent guidance,
- oryginalne importowane źródła jako pliki binarne z zachowaniem SHA-256 i powiązania przez metadata JSON.

Jawnie wykluczono vault credentials, provider private replay (`chat_replay`), receipts/request fingerprints, raw provider payloads i prywatne refleksje, gdy te zostaną kiedyś zaimplementowane.

`profile.export.create` jest idempotentne dla `requestId`. Export innego profilu nie jest widoczny przez `profile.export.list/read` dla bieżącego scope.

## Testy dodane

- native: kolejność wiadomości, Markdown, manifest, brak provider private replay, idempotency i izolacja profilu,
- UI: create/list dla wybranego profilu bez ujawnienia ścieżki filesystem,
- stałe release gates: schema expectation sweep, contract/handler/router parity, Rust source-text normalization, P2 i boundaries.

## Weryfikacja lokalna

- P2: 193/193 PASS,
- boundaries: 29/29 PASS,
- command parity: 49/49 PASS,
- schema expectation sweep: PASS,
- Rust source-text sweep: PASS,
- SQLite: v12, bez migracji.

Cargo/Rust oraz pełny Vitest/typecheck/Vite nie zostały uruchomione lokalnie z powodu brakującego toolchainu/zależności. Wymagane pełne GitHub Actions i re-audyt przed kolejnym krokiem.
