# AI Home — audyt główny 4B-11 Reflection/Dream

Data: 26.10.2026. Podstawa: SOURCE, CHANGED_FILES, instrukcja, VERIFY_DIRECTORY, OVERLAY_PROOF, raport i plan 1.71. Werdykt: **DO KOREKTY**. Audyt nie zmienia implementacji.

## F1 — P2: zaakceptowana notatka może być trwale niewidoczna i niewycofywalna

Plik `src-tauri/src/experimental_reflection.rs`: `review()`, `context()`, `withdraw()`.

`review()` dopuszcza modified_content do 8192 bajtów, o ile polityka pamięci pozwala (np. unlimited). `context()` przekazuje natomiast wyłącznie całe rekordy mieszczące się w 4000 bajtach całego serializowanego pakietu. Nie istnieje ścieżka pobrania konkretnego self record do nowej decyzji Primary. `withdraw()` wymaga obecności pełnego wpisu z daną rewizją w packet.private_context.records; zastąpienie wpisu w `review()` ma ten sam warunek.

Scenariusz: mała, widoczna propozycja own_note → Primary wybiera modify z tekstem 5000 bajtów przy unlimited → zapis aktywnego self record jest dozwolony. Od następnej okazji rekord nigdy nie mieści się w kontekście (sam content przekracza 4000, przed metadanymi). Próba WithdrawSelf lub zastąpienia kończy się CONTEXT_BUDGET_EXCEEDED. Wpis nadal istnieje jako active. Nie jest to przejściowe pominięcie wynikające z wielu rekordów: nawet pusty pozostały kontekst go nie pomieści.

Analogicznie zbyt duża propozycja wygenerowana przy output_bytes=8192 może stale pozostawać pending, bo nawet reject wymaga jej wcześniejszej obecności w ograniczonym pakiecie.

Dowód: analiza warunków akceptacji/pakowania/wycofania oraz niezależny pomiar JSON: sam obiekt z content o 5000 bajtach ma ponad 5000 bajtów i przekracza próg pakowania. Nie uruchomiono tej ścieżki w natywnym runtime Rust.

Poprawka: zapewnić bounded, jawne pobranie wybranego wpisu/propozycji do decyzji Primary albo spójny limit pełnego serializowanego elementu przed zapisem. Nie obchodzić wymogu świadomego przeglądu Primary. Regresja natywna: mała propozycja → modify 5000 bajtów przy unlimited → kolejna okazja umożliwia przegląd/wycofanie/zastąpienie, albo pierwotny zapis jest odrzucony atomowo. Sprawdzić też duży pending candidate i reject.

## F2 — P2: generowanie refleksji gubi pierwotne source refs

Plik `src-tauri/src/experimental_reflection.rs`: `input()`, `source_refs()`, `complete()`, `review()`; `src-tauri/src/deletion.rs::redact_experimental_source_refs()`.

Generator otrzymuje technical i semantic checkpoint z ResumePacket, wraz z ich sourceRefs i semantyczną treścią. Funkcja budująca pochodzenie zapisanej refleksji dodaje jednak tylko odnośnik `other` do ID technical checkpointu. Nie przenosi jego sourceRefs ani sourceRefs semantic checkpointu. Dla wcześniejszych notatek i kandydatów także dodaje jedynie odnośnik do bezpośredniego rekordu, bez dziedziczenia jego źródeł.

Przykład: checkpoint ma sourceRefs conversation:c1; refleksja wygenerowana na jego podstawie dostaje wyłącznie other:<technical-id>. Po permanent deletion c1 istniejący kod usuwa dostępność c1 w checkpointach, ale nie w refleksji ani zatwierdzonej z niej notatce — deletion skanuje bezpośrednie sourceKind/sourceId i nie przechodzi przez graf pochodzenia. Ich provenance nie zawiera c1, więc nie może zostać oznaczone permanently_deleted. Późniejsze generacje powielają ten brak.

Nie jest to żądanie usuwania semantycznej treści własnych notatek po usunięciu źródła. Raport słusznie zachowuje je jako odrębne rekordy. Problem dotyczy utraty deklarowanego pochodzenia i informacji o niedostępności źródła.

Dodana regresja deletion ręcznie wstawia sourceRef conversation do private reflection. Dowodzi działania tombstonowania na przygotowanym rekordzie, ale nie sprawdza, że normalne Reflection/Dream w ogóle utworzy taki sourceRef.

Dowód: analiza kompletnej funkcji source_refs i bezpośredniego dopasowania w deletion; nie wykonano end-to-end w Rust.

Poprawka: zachować pierwotne refs (z deduplikacją i jawnie kontrolowanym rozmiarem) lub zapewnić rozwiązywanie grafu pochodzenia oraz propagację niedostępności przy odczycie/usunięciu. Dodać regresję używającą rzeczywistego runtime: checkpoint z c1 → Dream/Reflection → decyzja Primary → permanent deletion c1 → właściwe oznaczenie pochodzenia refleksji i własnej notatki, bez usuwania jej autonomicznie zatwierdzonego tekstu.

## Ocena rozwiązania

Rozdział generowania od późniejszego accept/modify/reject jest zgodny z uzgodnieniami. Dream zachowuje status synthetic_hypothesis; przyjęte wpisy pozostają self_authored_not_verified_fact. Provider nie otrzymuje executora, nie publikuje i nie zmienia bezpośrednio pamięci. Kontrole generacji, hashy, snapshotów i bounded input/output oraz atomowe transakcje stanowią właściwą podstawę. Granice produktu są opisane uczciwie: nie jest to jeszcze autonomiczny tryb desktopowy z live modelem i schedulerem.

F1 dotyczy możliwości dalszego zarządzania własnymi wpisami, a F2 pochodzenia danych. Oba wymagają poprawki przed zamknięciem 4B-11.

## Integralność i wykonane kontrole

- CRC oraz unikalność nazw obu ZIP: PASS.
- SOURCE SHA256: `488a76fcbf1e36133be16b5da2b3adeac1f5508d8cecc12ad238d87189279aef`.
- CHANGED_FILES SHA256: `3c50185d2ecfb0b5743a2a27b54ccf30a4f5b5b8b0dd0c834fc0893bc6da77f1`.
- Sumy zgodne z OVERLAY_PROOF. Wszystkie 49 plików overlay odpowiadają bajtowo plikom SOURCE po uwzględnieniu prefiksu AI_Home.
- verify-source przed instalacją/buildem: PASS, 616 manifest hashes / 617 SHA256SUMS entries.
- npm ci: PASS; Vitest 219/219, 21 plików PASS; TypeScript i Vite build PASS.
- Statyczne: Reflection 12/12, internal state 23/23, continuity 23/23, runtime 24/24, foundation 12/12, parity 54/54, boundaries 34/34 wraz z 13 asercjami oczekiwanej wersji schematu, Standard closure/release 8/8 i 8/8, Rust source-text PASS.
- Cargo/rustc nadal niedostępne lokalnie: natywne 409 testów, migracje, backup/restore i lifecycle nie zostały niezależnie uruchomione. Dołączone logi autora stanowią osobne dowody; nie przypisuję ich własnemu wykonaniu.
- Konkretnej paczki bazowej SCHEMA14_FIX nie było w bieżących załącznikach. Nie potwierdzam niezależnie pełnego BASE+OVERLAY=SOURCE ani 72 protected files; potwierdzam integralność SOURCE i zgodność wszystkich wpisów overlay. Wcześniejszy AUDIT_FIX nie jest tożsamy z deklarowaną późniejszą bazą SCHEMA14_FIX.
- Brak testu desktopu Windows/Tauri/WebView, OS vault i live API. Frozen P2 193/193 jest deklaracją dołączonego raportu, nie ponowionym przeze mnie wynikiem tej bramki.

Do dalszego odbioru: poprawki F1/F2, natywne regresje end-to-end, pełny zielony CI dla wynikowej paczki i ponowny audyt. Nie rekomenduję jeszcze zamknięcia 4B-11.
