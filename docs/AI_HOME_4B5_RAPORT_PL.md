# AI Home — 4B-5 REBASE AUDIT FIX S1/S2 — raport PL

**Data:** 22 października 2026

## Status

Korekta dwóch problemów wskazanych w audycie `AUDYT_AI_HOME_4B5_REBASE_PL.md`. Zakres nie rozpoczyna 4B-6. 4B-5 pozostaje otwarty do pełnego CI i ponownego audytu.

## S1 — checkpoint przy małej liczbie dużych wiadomości

Usunięto sztuczne wymaganie co najmniej 6 wiadomości przed automatycznym seal. Decyzja opiera się teraz na faktycznym obciążeniu aktywnego segmentu względem zadeklarowanego context window, output reserve i input reserve. Dzięki temu 2–4 duże wiadomości mogą utworzyć checkpoint po udanej turze i nie blokują następnej wiadomości wyłącznie z powodu liczby rekordów.

Dodano także ścieżkę legacy dla rozmów istniejących przed v12 i niemających `activeSegmentId` ani wcześniejszego sealed segmentu. Jeżeli ich surowa historia przekracza próg segmentacji dla aktualnego modelu, tworzony jest bounded legacy checkpoint; surowe wiadomości pozostają niezmienione w `messages`.

## S2 — najnowsze tury mają pierwszeństwo

`bounded_excerpt` wybiera teraz materiał od najnowszej wiadomości wstecz, dopiero potem układa zachowane elementy chronologicznie. Pojedyncza starsza długa wiadomość nie może wyprzeć najnowszej wypowiedzi. Jeśli część starszego materiału nie mieści się w limicie, summary zawiera jawny marker pominięcia. Obcinanie działa po znakach Unicode.

## Testy dodane

- 2 duże wymiany przy małym context window -> sealed segment przed 6 wiadomościami -> następna tura nadal możliwa;
- bardzo długa starsza wiadomość + nowsze znaczniki -> checkpoint zachowuje najnowszą wypowiedź i oznacza pominięcia;
- dotychczasowe testy wielosegmentowej ciągłości pozostają w zestawie.

## Kontrole wykonane lokalnie

- `python3 check_contracts.py`: **193/193 PASS**;
- `node scripts/verify-boundaries.mjs`: **26/26 PASS**;
- Rust/Cargo: **NOT RUN**, brak toolchainu w środowisku;
- Vitest/TypeScript/Vite: bez zmian funkcjonalnych frontend w tej korekcie, pełne CI nadal obowiązkowe.

## Bramy nadal otwarte

Pełne Cargo/Rust all-targets locked, Vitest/TypeScript/Vite, realna migracja 11→12 + restart, CAS w trakcie fizycznego POST, Windows/Tauri/WebView, OS Vault i live API.
