# AI Home 4B-6 krok 2 — raport wykonawczy

**Data:** 22.10.2026  
**Zakres:** backup + restore z preflight i safety copy.  
**Baza:** `AI_HOME_4B6_KROK_1_AUDIT_FIX_4_SOURCE.zip`.  
**Status:** IMPLEMENTED — REQUIRES CI AND INDEPENDENT AUDIT.

## Zaimplementowane

- `contracts/backup.ipc.json`: `backup.create`, `backup.list`, `backup.restore.preflight`, `backup.restore.request`, `backup.restore.status`.
- Backup jest lokalnym snapshotem SQLite wykonanym przez `VACUUM INTO`; plik otrzymuje manifest z SHA-256, rozmiarem, datą i schema version.
- Preflight jest read-only i sprawdza checksum, `PRAGMA user_version`, `PRAGMA integrity_check` i `PRAGMA foreign_key_check`. Odrzuca backup nowszy od wspieranej wersji 12.
- Restore nie podmienia otwartej bazy. Request tworzy safety backup bieżącego stanu i atomowy sidecar `restore-pending.json`; przy kolejnym starcie pending jest walidowany i staging copy zastępuje bazę przed `Store::open`.
- Jeśli pending restore jest uszkodzony lub nie przechodzi walidacji, bieżąca baza pozostaje na miejscu, pending jest rozliczony jako failure, a aplikacja może się uruchomić.
- Restore starszego, wspieranego schematu jest następnie normalnie migrowany przez `Store::open` do v12.
- Backup nie odczytuje OS vault. Raw API keys nie są w SQLite, więc nie są kopiowane.
- Mutacje zachowują requestId przy niejednoznacznym wyniku; backup ID/safety ID są deterministycznie związane z requestId.
- Panel EN/PL pokazuje listę kopii, preflight, wymaganie safety copy oraz stan „restart required”.

## Testy dodane

- backup → zmiana preferencji → restore po restarcie, z weryfikacją safety backupu,
- read-only preflight,
- uszkodzony backup odrzucony przed restore,
- uszkodzony pending restore zachowuje bieżącą bazę i zapisuje failure status,
- UI: create/list/preflight/restore request i restart-required.

## Stałe bramki

- schema expectation sweep: brak asercji docelowej wersji 11 w testach; historyczne `PRAGMA user_version=N` tworzące stare fixture pozostają celowo,
- contract/handler/router parity: wszystkie 44 komendy suplementarne obecnego drzewa mają ścieżkę w native application/connections,
- frozen P2 bez zmian.

## Nie w tym kroku

Portable export, permanent deletion, 4B-7, Windows/WebView smoke, real OS-vault/live API.
