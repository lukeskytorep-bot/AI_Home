# AI Home 4B-0 — fundament po Final Normalization

Wersja treści 1.3 · 12 października 2026

**4B-0 otwarty; 4B-1 nierozpoczęty.** Normalizacja źródeł i dowodów zakończona lokalnie. Edward potwierdził udane CI po poprawce async; nie dostarczono identyfikatora przebiegu, commitu ani zakresu platform, dlatego nie nadano niezależnego PASS Windows/WebView.

## Baza i zakres

Baza: załączony AI_Home-main.zip; jego SHA-256 zapisano w docs/evidence/source-status.json. Pracowano na tej migawce GitHub, nie na wcześniejszym lokalnym szkielecie. Funkcjonalny kod aplikacji, cztery kontrakty P2, katalog IPC, workflow, Cargo.toml i Cargo.lock pozostają bajtowo zgodne z wejściem. Zmieniono organizację materiałów, checkery referencyjne, dokumentację i identyfikację przekazania. Usunięto wyłącznie nieużywane kopie lub zastąpiono je kanonicznymi wejściami.

## Wyjaśnienie P2

| Liczba | Znaczenie |
|---|---|
| 119 | Zapis historycznego zestawu P2 przed korektami audytowymi |
| 145 | Rzeczywisty wynik starszego check_contracts.py z aktualnymi kontraktami/przykładami w wejściowym ZIP-ie — stan mieszany, nie pełny zestaw po audycie |
| 193 | Pełny checker odebranego P2 1.1, zawierający dodatkowe kontrole wyniku operacji, błędu, modelRouteId i gotowego obrazu bez OCR |

Cztery kontrakty z GitHub były już zgodne z P2 1.1. Nie przebudowywano ani nie zmieniano schematów. Przywrócono kompletny snapshot snapshots/p2-v1.1 z brakującym build_audit_fixtures.py, właściwym checkerem i 23 poprawnymi wpisami jego oryginalnego SHA256SUMS. Główny check_contracts.py jest prostym wejściem: sprawdza zgodność bieżących kontraktów ze snapshotem i uruchamia jego pełny checker.

Pliki starego przekazania przeniesiono do snapshots/p2-legacy-mixed, z wyraźnym opisem niespójności. Jego pierwotnego SHA nie „naprawiono”: historyczny manifest nie potwierdza aktualnej aplikacji. Historyczne źródła i logi zachowano bez udawania, że stanowiły spójne wydanie. Generatory P2 są materiałem snapshotu, nie skryptami do uruchamiania na runtime aplikacji.

## Porządek źródeł i manifestów

- Usunięto src-tauri/src-tauri/src/lib.rs i manifest poprawki z src-tauri/. Natywny właściwy src-tauri/src/lib.rs zachowano.
- Manifesty jednorazowych poprawek i starych przekazań przeniesiono do historii, aby nie wyglądały jak sumy całego projektu.
- Poprzednie docs/evidence przeniesiono do snapshots/4b0-evidence-before-normalization. Nowe lokalne logi znajdują się w docs/evidence.
- Nowy manifest opisuje komplet aktualnych plików w przekazaniu. Wyklucza tylko siebie i główny SHA256SUMS, by uniknąć wzajemnego hashowania. SHA256SUMS obejmuje również manifest, poza samym sobą.
- scripts/verify-source.py sprawdza kompletność, oba zestawy hashy oraz katalog lub ZIP niezależnie od nazwy pobrania. scripts/verify-handoff.py jest wejściem do tego samego weryfikatora.
- scripts/apply-normalization-cleanup.py usuwa jedynie wymienione pliki wejściowe i sprawdza ich hashe przed jakimkolwiek usunięciem. Jeśli któryś plik został zmieniony, zatrzymuje się i zachowuje wszystkie pliki. Może zostać uruchomiony ponownie.

## Wyniki wykonane tutaj

| Kontrola | Wynik |
|---|---|
| Instalacja npm z lockfile | PASS |
| Vitest/jsdom | 53/53 PASS |
| TypeScript i build Vite | PASS |
| Kontrole granic / zamrożone P2 | 15/15 PASS |
| Pełny zestaw referencyjny P2 1.1 | 193/193 PASS |
| Oryginalne hashe kanonicznego snapshotu P2 | 23/23 PASS |
| Zachowanie właściwych plików runtime/kontraktów/workflow/lock | 32/32 identyczne bajtowo |
| Kontrole aktualnego drzewa i pełnego ZIP-a | PASS; log końcowej weryfikacji przekazany razem z pakietem |
| Natywne testy Rust/SQLite w lokalnym środowisku | NOT RUN — brak cargo/rustc |
| Nowy przebieg CI dla samej normalizacji | Nie uruchomiono tutaj |
| Poprzednie udane CI po poprawce async | Potwierdzone przez Edwarda, bez niezależnej identyfikacji przebiegu |
| Windows, wygenerowane uprawnienia, rzeczywisty WebView | Nadal brak kompletnego dowodu odbioru |

## Aktualny stan aplikacji

Baza zawiera kernel Standard/NullState, ustawienia, SQLite, idempotencję/CAS i zdarzenia aplikacji. EN domyślnie, PL, Orbital/Aurora light/dark/system. Kod chroni szkic po konflikcie i opóźnionym zdarzeniu. Cargo.lock istnieje; async komenda zwraca Result. Codzienne CI sprawdza Ubuntu bez artefaktów; Windows oraz pliki wykonywalne są opcjami ręcznymi. Nie trzeba ponownie generować lockfile ani zmieniać workflow dla tej normalizacji.

Nie ma jeszcze providerów, profili, rozmów ani pamięci runtime; to kolejne etapy. Plan 1.9, README, Acceptance i CI_PL opisują obecny stan. Plan wykorzystania providerów i innych gotowych modułów Harness pozostaje aktualny.

## Status 4A i następne czynności

Odbiór forum 4A pozostaje potwierdzeniem Edwarda. Starszy indeks z otwartą bramką produkcyjną nie został tutaj zmieniony ani uznany za niezależny dowód zamknięcia. Przed 4B-12 trzeba uzgodnić aktualny formalny status integracji 4A na podstawie bieżących dokumentów/dowodów. Ta rozbieżność nie blokuje niezależnej aplikacji Standard.

Następne: zastosować pakiet, zwykłe CI, zachować link i commit udanego przebiegu; zaliczyć Windows, kontrolę uprawnień oraz start/restart i zapis w WebView. Potem audyt i decyzja o zamknięciu 4B-0. Nie rozpoczęto 4B-1.
