# AI Home — korekta CI i zależności Tauri

11 października 2026 · kandydat do sprawdzenia w GitHub Actions

Edward słusznie wskazał, że zwykłe CI może sprawdzać Rust, SQLite i Tauri bez wydania aplikacji. Poprzednie zielone zadanie frontend potwierdzało wyłącznie frontend; nie oznaczało braku możliwości takich kontroli w Actions. Dotychczasowe native wykonywało testy bez desktop features, a potem pełny zoptymalizowany build na każdym push. Nie publikowało wydania, lecz było niepotrzebnie ciężkie do codziennej pracy.

## Nowy podział

| Zadanie | Kiedy | Zakres |
|---|---|---|
| frontend | push, PR, ręcznie | Granice modułów, 53 testy, TypeScript i build Vite |
| native | push, PR, ręcznie; Windows 2022 i Ubuntu 24.04 | `cargo test --all-targets --locked` z domyślnym desktop feature: testy Rust/SQLite oraz kompilacja kodu Tauri, konfiguracji i uprawnień. Bez budowania zoptymalizowanej aplikacji przez tauri build, instalatorów i publikacji. |
| desktop_candidate | Tylko ręcznie, po zaznaczeniu build_desktop i przejściu obu poprzednich zadań | Plik wykonywalny do sprawdzenia na komputerze; `--no-bundle`, bez instalatora i publikacji |
| Prepare Cargo lockfile | Jednorazowo ręcznie; ponownie tylko przy uzasadnionej zmianie zależności | Generacja Cargo.lock, test Rust/SQLite bez WebView, artefakt lockfile do przeglądu i osobnego commitu |

Zwykłe testy budują własne programy testowe; nie są wydaniem produktu. Żaden workflow w tym pakiecie nie tworzy GitHub Release, taga ani instalatora. Ręczne wykonanie AI Home checks domyślnie również nie buduje desktop candidate.

## Błąd wskazany przez załączone logi

Ubuntu: błędy `Option<Monitor>` kontra `Result<_, Error>` w tauri 2.11.5. Windows: różne typy HWND oraz błędy wygenerowanych menu; log wprost wskazuje tauri-runtime 2.12.1. Wniosek: rozwiązanie zależności wybrało nowszą rodzinę runtime/makr do starszego Tauri. Ostrzeżenia Node i Ubuntu są odrębne od tego błędu kompilacji.

Zestaw sprawdzony w źródłowym Cargo.lock migawki Harness: tauri 2.11.5, tauri-build 2.6.3, tauri-runtime 2.11.3, tauri-runtime-wry 2.11.4, tauri-utils 2.9.3, tauri-codegen 2.6.3, tauri-macros 2.6.3. Przypięto ten zestaw w Cargo.toml. Cargo.lock nie jest ręcznie zmyślony ani przeniesiony z obcej domeny: generuje go osobny workflow i trzeba zachować go w repozytorium AI Home. Poprawka zależności jest kandydatem: nie ma jeszcze wyniku ponownej kompilacji potwierdzającego usunięcie błędów.

## Ostrzeżenia Actions

Zastąpiono checkout v4 przez v5, setup-node v4 przez v6 oraz upload-artifact v4 przez v6. Wersja Node programu pozostaje 24; to odrębna rzecz od runtime samej akcji. Przypięto ubuntu-24.04 zamiast ubuntu-latest oraz windows-2022 zamiast windows-latest. Nie polegamy na zmianie etykiety Ubuntu w październiku. Dodano anulowanie starszych przebiegów tej samej gałęzi oraz cache zależności i wyników kompilacji Rust, rozdzielony według systemu, zadania, wersji kompilatora i hashy Cargo.lock/Cargo.toml. Cache nie zastępuje testów ani --locked.

## Jak zastosować

1. Rozpakować AI_HOME_CI_CHANGED_FILES.zip **do głównego katalogu repozytorium AI_Home**, z zachowaniem podkatalogów. Pakiet zawiera wyłącznie zmienione i nowe pliki; nie nakładać go na Harness. Nie usuwa istniejącego Cargo.lock.
2. Commit i push zmian. W Actions uruchomić **Prepare Cargo lockfile** na tej samej gałęzi. Jeśli job jest czerwony, zachować log — artefakt Cargo.lock jest oferowany również po niepowodzeniu testów, jeżeli generacja się powiodła; sam artefakt nie oznacza PASS.
3. Z artefaktu AI-Home-Cargo-lock-review pobrać Cargo.lock i dodać jako **src-tauri/Cargo.lock**. Zatwierdzić go osobnym commitem. Jeśli w repo był już lock z niezgodnym runtime, zastąpić go wygenerowanym po korekcie manifestu.
4. Nowy push uruchomi **AI Home checks**. Zadanie native wymaga istniejącego lockfile; bez niego ma zakończyć się czytelnym błędem, zamiast rozwiązywać wersje po cichu przy każdym etapie. Po zachowaniu lockfile wszystkie testy korzystają z --locked.
5. Do sprawdzania działania okna uruchomić AI Home checks ręcznie z **build_desktop=true**. To przygotowuje kandydat do uruchomienia, bez publikacji. Pełne wydania i instalatory pozostają przyszłym osobnym procesem.

## Co rzeczywiście sprawdzono tutaj

Przeczytano oba załączone logi, dostępną migawkę Harness ci.yml/prepare-cargo-lock.yml i jej Cargo.lock. Próba odczytu bieżącego repozytorium Harness przez przeglądanie nie powiodła się; nie deklarujemy wglądu w aktualne przebiegi Harness ani wyniku nowego AI Home CI. Wcześniejszy lokalny frontend 53/53 pozostaje bez zmian. Dla tej korekty sprawdzono składnię YAML/TOML i reguły workflow; bez uruchomienia GitHub Actions i bez lokalnego Rust.

Aktualne źródła dokumentacji akcji: https://github.com/actions/checkout, https://github.com/actions/setup-node, https://github.com/actions/upload-artifact. Przekazanie --locked do Cargo sprawdzono w pomocy lokalnego tauri CLI: `npm run tauri build -- --no-bundle -- --locked`.

4B-0 pozostaje otwarty. Zielone testy Rust/SQLite i kompilacja Tauri nadal wymagają dowodu z nowego CI. Rzeczywisty start/restart WebView, zapis, motywy i kontrola błędów pozostają osobnym odbiorem. 4B-1 nie rozpoczęto.
