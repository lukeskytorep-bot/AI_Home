# AI Home — 4B-10: korekta oczekiwań SQLite v14

Data: 26.10.2026. Baza: załączony `AI_Home-main.zip`, commit archiwum GitHub `cf108496fb48db9e648b151232807602ae6981b7`, SHA-256 `8c594ed2d7c0b95e68f611d5e389fb3e3082b259ab57ac8bf5baf643e7790c88`.

## Przyczyna i poprawka

Runtime 4B-10 prawidłowo kończy migracje na SQLite `user_version=14`. Starsze testy oczekiwały nadal 13. Trzy zgłoszone awarie nie są dowodem utraty danych ani błędnej migracji: porównanie końcowej wersji bazy było nieaktualne.

Pełny przegląd żywych testów znalazł 11 takich asercji w sześciu plikach:

| Plik | Poprawione oczekiwania |
|---|---:|
| src-tauri/tests/chat.rs | 3 |
| src-tauri/tests/connections.rs | 2 |
| src-tauri/tests/conversations.rs | 2 |
| src-tauri/tests/imports.rs | 1 |
| src-tauri/tests/memory_runtime.rs | 2 |
| src-tauri/tests/memory_layers.rs | 1 |

Każda zmiana w tych plikach to wyłącznie 13→14 przy końcowym `assert_eq!(...PRAGMA user_version...)`. Nie usunięto testu, nie wyłączono asercji i nie osłabiono kontroli zachowania danych, izolacji ani foreign keys. Historyczne przypisania `PRAGMA user_version=1/2/3/4/5/6/7/8/9/13` pozostają jako punkty startowe migracji. Inne znaczenia liczby 13, np. liczba tabel lub sekwencja wiadomości, pozostają bez zmian.

## Ochrona przed ponownym dryfem

Istniejący `scripts/verify-boundaries.mjs`, już wywoływany przez obecny frontend CI, odczytuje docelową wersję z `persistence.rs` i porównuje ją z końcowymi asercjami wersji w testach. Rozpoznaje dwa stosowane układy źródła Rust: jedno- i wielowierszowy. Historyczne przypisania fixture są pomijane. Wbudowana próba parsera kontroluje te dwa układy oraz ignorowanie przypisania fixture. Nie wymaga nowej zależności, polecenia package.json ani workflow.

Regresja przed poprawką wykryła dokładnie 11 starych oczekiwań. Po poprawce sprawdzonych jest 13 asercji: 11 skorygowanych oraz dwie już prawidłowe w Experimental. Bramka granic ma teraz 34 kontrole zamiast 31. Kontrola tekstowa jest wczesnym wykrywaniem tych stosowanych wzorców, a nie substytutem testów Rust.

## Faktycznie wykonane kontrole

| Zestaw Rust/Linux headless | PASS |
|---|---:|
| chat — w tym trzy zgłoszone regresje | 36/36 |
| connections | 24/24 |
| conversations | 10/10 |
| imports | 23/23 |
| memory_runtime | 30/30 |
| memory_layers | 25/25 |
| experimental_contracts — m.in. migracja v13→v14 | 3/3 |
| experimental_internal_state — także poprzednia poprawka wyboru kandydata | 5/5 |
| **Razem** | **156/156** |

Dodatkowo: granice 34/34; zamrożone P2 193/193; internal-state invariants 23/23; Rust source-text PASS. Komenda Cargo i pełne logi są w `docs/evidence/4b10-schema14-fix`. Użyto rustc 1.99.1 i niezmienionego Cargo.lock. Wybrano sześć zmienionych zestawów testów oraz dwa zestawy dotyczące nowej wersji 14; nie deklarujemy pełnego all-targets CI.

Nie powtarzano Vitest/TypeScript/build, ponieważ UI, jego testy i zależności nie zmieniły się. Nie wykonano Windows, desktop feature/Tauri/WebView, live API ani systemowego vault. Zielony natywny GitHub Actions dla tej paczki nadal jest bramką odbioru 4B-10.

## Integralność przekazania

W bazowym ZIP-ie wszystkie istniejące pliki ujęte w starym manifeście mają poprawne hashe. Sam inwentarz jest jednak starszy: wymaga 122 nieobecnych ścieżek i pomija dwie obecne. Szczegóły zachowano w `base-inventory.json`. Nie przywracano brakujących historycznych kopii dokumentów ani niczego nie usuwano. Odnowiono manifest i SHA256SUMS dla faktycznego drzewa załączonego przez Edwarda, uwzględniając niniejszą poprawkę i dowody.

Wszystkie produkcyjne źródła Rust, migracje, kontrakty, zależności, lockfile, workflow, uprawnienia oraz dotychczasowe dokumenty pozostają identyczne bajtowo względem załączonego ZIP-a. Plan i zakres 4B-10 nie zostały rozszerzone; nie rozpoczynano 4B-11. Pełny SOURCE i nakładka zostały porównane przez odtworzenie: dokładna baza + nakładka = pełny SOURCE. Szczegóły i sumy są w osobnym logu oraz SHA256.

## Zastosowanie

Rozpakuj `AI_HOME_4B10_SCHEMA14_FIX_CHANGED_FILES.zip` w głównym katalogu repozytorium obok `package.json` i `src-tauri`, nadpisz pliki, zrób commit i push. Niczego nie usuwaj. Nie potrzeba release ani instalatora. SOURCE jest pełną migawką dla audytora; nie jest drugą nakładką.
