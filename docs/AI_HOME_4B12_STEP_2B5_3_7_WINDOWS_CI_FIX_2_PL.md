# 2B-5.3.7 — Windows CI Fix 2

Baza: Windows CI Fix 1 SOURCE, SHA-256 81e4af190ef4828053adce9dd601c83cff43d06d57c1a3e78f151e65d11aab31.

## Zgłoszenie i przyczyna

W przesłanym logu Windows biblioteka ma 314 PASS / 1 FAIL. Pozostał nowy test inspection_keeps_live_wal_bytes_and_reads_every_file_after_close, asercja zgodności ścieżki i bajtów WAL. Ten wynik nie dowodzi wykonania dalszych zestawów ani uzyskania desktop candidate.

Gate::open kanonizuje ścieżkę registry. bytes_after_close buduje ścieżkę live WAL na podstawie registry.storage_path(). Test budował oczekiwaną ścieżkę ze zwykłego temp_dir. Na Windows kanonizacja wprowadza składnię extended-length (prefiks \\?\), więc równość PathBuf odrzuca różne reprezentacje tego samego pliku. Usterka jest w przygotowaniu oczekiwań testu. Źródło zachowania systemowego: https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html.

## Poprawka

Po utworzeniu katalogu test kanonizuje root, zanim wyznaczy wszystkie ścieżki i otworzy registry. Dodatkowa asercja wymaga zgodności registry.storage_path() z oczekiwaną ścieżką. Zachowano dokładne porównanie bajtów WAL, sentinel, kontrolę dodatkowego pliku, bazy, owner-lock i każdego pozostałego pliku. Bez wyłączania testów, cfg-wyjątków Windows, obcinania prefiksów, zmiany skanera czy kodu runtime. Jedyny plik Rust zmieniony względem Fix 1: src-tauri/src/community_identity/test_storage.rs (4 dodane linie).

## Dowody wejściowe

Oba native-check-evidence ZIP mają poprawne CRC. ACL main w obu zawiera allow-home-dispatch, allow-community-dispatch i event listen/unlisten, local=true. Cargo.lock Windows i Ubuntu są identyczne po CRLF->LF. ZIP-y są metadanymi builda, nie logiem pełnego PASS i nie aplikacją.

## Walidacja

Własna walidacja Fix 2: cargo test --manifest-path src-tauri/Cargo.toml --lib --no-default-features --locked: Linux Rust 1.99.1, 319/319 PASS, 0 ignored; poprawiony test WAL PASS. To 319 testów biblioteki Linux, nie pełne all-targets ani Windows. Kontrola bajtowa 197 chronionych plików względem Fix 1 PASS; pakiety i rekonstrukcja overlay sprawdzone osobno.

Nie wykonano Windows/MSVC/WebView/OS vault/live forum/TLS. Wcześniejsze wyniki frontend/build/verify z Fix 1 są historyczne, nie powtórzono ich przy tej zmianie testowej. README po angielsku zachowano bajtowo. Runtime, dependencies/lock, fixtures, kontrakty, migracje, workflow, ACL i UI bez zmian.

## Zastosowanie

Nałożyć CHANGED_FILES na Windows CI Fix 1; zawiera tylko różnice względem tamtej bazy. Commit/push i ponownie AI Home checks z build_desktop=true. Wymagane wszystkie frontend/native jobs PASS, następnie desktop-candidate-windows-2022. Bramka Windows pozostaje OPEN do nowego runu. To korekta istniejącego kroku, bez release i bez zamknięcia 2B-5.3/2B/4B-12.
