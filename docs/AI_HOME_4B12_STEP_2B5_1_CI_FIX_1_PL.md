# AI Home — 2B-5.1 — CI fix 1: brakujące rlib

Data: 27.10.2026. Status **AWAITING COLD NATIVE CI**. Ta poprawka nie rozpoczyna 2B-5.2 ani nie zamyka 2B-5.1.

Baza: AI_HOME_4B12_STEP_2B5_1_SOURCE.zip, SHA-256 `6e1c4052b6ba062d0fbf8375a13538a4063d364ef47fc3fda8c515bf3fe6dc5f`.

## Zgłoszenie i diagnoza

GitHub zgłosił błąd kompilacji skryptu budowania ai-home: writeable i base64 są wymagane jako rlib, lecz takiej postaci nie znaleziono. To nie jest komunikat o braku tych zależności w Cargo.toml. Z krótkiego logu nie można definitywnie ustalić przyczyny.

Najbardziej prawdopodobną przyczyną są niespójne lub niekompletne przywrócone artefakty target. Poprzedni workflow cache'ował src-tauri/target razem z pobranymi zależnościami. W repo nie ma RUSTFLAGS/prefer-dynamic ani dodatkowego wrappera cache, który tłumaczyłby ten komunikat. Nie odtworzono błędu na świeżym GitHub runnerze; diagnoza pozostaje hipotezą do sprawdzenia przez cold native CI.

## Poprawka

W dwóch sekcjach foundation.yml (native i ręczny desktop_candidate) cache przechowuje odtąd tylko ~/.cargo/registry i ~/.cargo/git. src-tauri/target nie jest zapisywany ani przywracany. Klucz otrzymuje nowy prefiks cargo-downloads-v2, więc nie trafia w poprzedni cache. Nie dodano restore-keys kierujących do starej rodziny.

Każdy standardowy GitHub-hosted job zaczyna z nowym target i kompiluje zależności oraz build scripts. Registry/git ograniczają ponowne pobieranie. Koszt: kompilacja Rust może trwać dłużej przy kolejnych uruchomieniach. To świadomy kompromis dla spójności obecnego CI, a nie obejście testów. Nie dodano automatycznego retry po błędzie, || true ani wyciszenia kodów wyjścia.

Nie zmieniono Rust, Cargo.toml, Cargo.lock, build.rs, funkcji aplikacji ani uprawnień. Native nadal wykonuje `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`. Windows nadal jest opcjonalny, desktop candidates ręczne, release/instalatory nie są dodawane. Nie trzeba ręcznie usuwać cache na GitHubie.

## Kontrola lokalna

- YAML obu workflowów parsuje się poprawnie; natywny workflow zmienia wyłącznie nazwy/ścieżki/klucze dwóch istniejących sekcji cache.
- W obu cache paths pozostają wyłącznie registry/git; nowy prefiks i brak fallback do starego target zostały sprawdzone.
- Polecenia, job matrix, warunki, permissions, triggers i katalog testów pozostają zgodne z bazą.
- Frozen P2: 193/193, granice: 34/34; kompletny manifest i odtworzenie BASE + overlay = SOURCE przechodzą.
- Całe drzewo Rust, kontrakty, zależności, frontend i wcześniejsze testy są bajtowo zgodne z bazą.

Nie uruchamiano ponownie Rust headless ani frontendu, bo nie zmienił się ich kod. Wcześniejsze 563/563 i 219/219 to historyczny wynik bazy 2B-5.1, nie dowód poprawy tego błędu. Nie wykonano lokalnego desktop/Tauri ani zdalnego cold CI. Nie deklaruje się jeszcze naprawionego GitHub build.

## Użycie i dalsza decyzja

Nałóż nakładkę w głównym katalogu bazy 2B-5.1, commit i push. Nie wybieraj wyłącznie „Re-run” starego run: trzeba uruchomić nowy commit ze zmienionym workflow. Nowy job ma nie przywracać target. Po pomyślnej kompilacji i testach można wrócić do odbioru 2B-5.1, następnie 2B-5.2 (wspólny backup/restore).

Jeśli ten sam błąd wystąpi na zimnej kompilacji, hipoteza cache nie wystarcza. Następny krok to pełny log build script/rustc z `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked -vv`; nie należy wtedy dodawać losowych zależności ani usuwać Cargo.lock.

Referencja: https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching — istniejący cache jest niezmienny; nowy klucz tworzy odrębną rodzinę cache.
