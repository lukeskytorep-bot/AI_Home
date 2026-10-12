# AI Home — 4B-2 krok 1: rozmowy i trwałe szkice

Wersja 1.1 — korekta po audycie, 12 października 2026. Kandydat do CI/audytu, 4B-2 otwarte.

## Korekta audytu: żądania UI a kontrakt

Pierwotny loadProfiles wysyłał {page:{limit:100}}, podczas gdy frozen profile.list wymaga płaskiego {limit:100}. Poprawiono jedyne wspólne wywołanie w pętli, więc kolejne strony wysyłają {limit:100,cursor:...}. Kontrakt pozostaje bez zmian.

Mock Conversations waliduje teraz każde żądanie: CommandEnvelope, payload według katalogu P2 lub jawnego dodatku rozmów, profilowy zakres i obecność expectedRevision tam, gdzie jest wymagana. Nowy stały test wymusza dwie strony, porównuje dokładne payloady, sprawdza zgodność P2 i obie opcje profili w UI. Przed poprawką wykonano testy z tą walidacją: 9 FAIL, 1 PASS (preview bez native); po poprawce cały frontend ma 81/81 PASS.

Ta korekta zmienia kod produkcyjny wyłącznie w src/ui/Conversations.tsx. Rust, SQL/migracje, wszystkie kontrakty, zależności, Cargo.lock i workflow są identyczne z audytowanym SOURCE kroku 1. Audytor potwierdził jego 107/107 Rust/Linux; nie uruchamiano nowego native przebiegu lokalnie w tej korekcie. Aktualne frontend/build/boundaries/P2 są ponownie wykonane i zapisane w docs/evidence/4b2-step-1-post-audit. Wcześniejsze logi są historyczne.

Paczka POST_AUDIT_CHANGED_FILES jest skumulowanym poprawionym krokiem 1 na oryginalnej bazie 4B-1 + deadline, zastępuje poprzednią nakładkę. Można ją nałożyć także na już zastosowany pierwotny krok 1; oba warianty są sprawdzone w logu przekazania. Nowe CI i ponowny audyt są nadal wymagane.

## Baza i zakres

Edward potwierdził zielone CI po synchronizacji kroku 4 i naprawie testu deadline, bez identyfikatora przebiegu/platform. Baza jest zrekonstruowanym pełnym SOURCE kroku 4 po audycie + dokładną poprawką deadline. Jej archiwum zachowane do odtworzenia: SHA-256 `0b7c424b3fa2b07aa221ecd80d922472f5c85ee2a816f11e53418bd8a703c2c4`. Nie jest to nowy ZIP pobrany z GitHub ani niezależnie sprawdzony commit. Nakładka zawiera zmiany do tej spójnej bazy, nie wymaga kolejnych wcześniejszych paczek.

Pierwsza część 4B-2 dodaje organizację rozmów i szkice. Wysyłanie do modelu, chat.send, ContextAssembler, trwałe wiadomości generowane i pamięć mają dalsze części. Wyślij jest jawnie niedostępne. Nie ma automatycznych ani płatnych wywołań. Podstawowa konfiguracja profili i kluczy z 4B-1 pozostaje dostępna. Otwarte odbiory wcześniejszych etapów nie zostały zamknięte.

## Zachowanie i granice

- Conversation.create, conversation.draft.set i conversation.messages.list wykonują istniejące kontrakty P2. Odczyty conversation.get/list są osobnym dodatkiem contracts/conversations.ipc.json; frozen P2 nie zmieniono.
- Każda rozmowa ma UUID, profileId, rewizję, język i dokładny draftText. Dwa profile nie widzą swoich rozmów, niezależnie od kluczy i modeli. Tytuł ma operacyjny limit 512 bajtów UTF-8; IPC zachowuje dotychczasowy limit 128 KiB całego żądania. Nie są to limity tokenów pamięci; nic nie jest cicho obcinane.
- Zapis szkicu używa CAS i trwałego receipt. Ten sam requestId nie powtarza zapisu; inne polecenie/treść/profil z tym ID daje konflikt. Szkic, receipt i zdarzenie snapshot.required są commitowane w jednej transakcji. Błąd zachowuje poprzedni rekord.
- P2 nie definiuje conversation.changed, dlatego profil otrzymuje istniejące snapshot.required z nową sekwencją. Po utracie powiadomienia UI może odczytać listę/rekord ponownie; obecny panel nie nasłuchuje jeszcze zdarzeń profilu i ma jawne odświeżanie.
- Paginacja rozmów jest keyset po UUID; wiadomości po sequence. Opaque cursory są związane z profilem, rozmową, rodzajem odczytu i rozmiarem strony. Nie wolno użyć kursora innego profilu/rozmowy. SQL i vault pozostają native, UI używa Client/IPC.
- Tabela messages i czytnik oryginalnej historii są gotowe dla dalszego runtime; ten krok nie zapisuje sztucznych wiadomości i nie ma jeszcze widoku historii wypowiedzi. Testy historii wprowadzają wiadomości bezpośrednio jako fixture do SQLite.
- UI EN domyślnie, PL dostępne. Osobny wybór profilu rozmowy umożliwia tworzenie/wybór rozmów bez klucza API. Po utworzeniu profilu w konfiguratorze można użyć Odśwież profile. Lista rozmów ładuje dalsze strony jawnym przyciskiem.
- Niezapisane szkice są lokalne i osobne dla profilu/rozmowy w czasie otwarcia ekranu. Zapisz szkic utrwala je po restarcie. Nie deklarujemy automatycznego zapisu ani trwałości niezapisanych zmian po zamknięciu aplikacji. Zmiana rozmowy/języka/motywu i odświeżenie nie zastępują dirty text zapisanym rekordem.
- Konflikt odświeża rewizję, zachowując tekst do ponowienia. Niejednoznaczny błąd transportu zachowuje requestId dla tej samej próby. Nowe zmiany wprowadzone podczas zapisu pozostają w polu i otrzymują informację, że jeszcze wymagają zapisu. Sekrety/localStorage nie są używane przez ten panel.

Baza v3 → v4 dodaje conversations, messages i conversation_cursors. FK wiadomości zawiera profil i rozmowę; wcześniejsze profile, połączenia, HMAC, ustawienia i receipts zachowano. Migracja odbywa się transakcyjnie przy starcie. Dotychczasowe testy v1/v2 oczekują teraz v4, zachowując kontrolę oryginalnej zawartości. Nowy test tworzy faktyczny stan v3 bez trzech nowych tabel, migruje i sprawdza profil oraz FK/integrity. Restart zapisanych szkiców i receipts ma osobny test.

Rozwiązanie korzysta z istniejącej warstwy native AI Home (walidacja, transakcje, CAS, receipts i zdarzenia). Nie przenosi historii sesji RV, nie dodaje nowej bazy ani zależności. Infrastruktura Harness z 4B-1 pozostaje niezmieniona; test deadline z zielonej bazy zachowano.

## Dowody

| Kontrola | Wynik |
|---|---|
| Rust Linux headless — audyt poprzedniej paczki, kod niezmieniony | 107/107 PASS: 65 provider/vault, 24 connections, 10 conversations, 8 foundation |
| Frontend — ponownie wykonany | 81/81 PASS, w tym 10 testów panelu rozmów; jsdom |
| TypeScript/Vite | PASS |
| Granice | 19/19 PASS |
| P2 | 193/193 PASS, frozen bajtowo bez zmian |
| Nakładka i inwentarz | Odtworzenie na dokładnej bazie i katalog/ZIP sprawdzone; log dołączony |

Rust 1.99.0: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --locked --offline`. Pierwotne logi: docs/evidence/4b2-step-1. Aktualne dowody korekty: docs/evidence/4b2-step-1-post-audit. Nowe testy obejmują izolację, dokładny Unicode, CAS, idempotencję, rollback receipt/event, walidację, cursory, kolejność i zakres historii, restart, faktyczną migrację v3 i FK/corrupt record. UI obejmuje przestawianie profili/rozmów, język/odświeżenie, save, konflikt, niejasny błąd transportu, tworzenie bez modelu i nową edycję podczas save.

Cargo.toml, Cargo.lock, package.json/lock, workflow, uprawnienia, frozen P2 i snapshoty są identyczne z bazą. Nie uruchamiano live API, OS vault, Windows/Tauri/WebView ani oceny renderowania prawdziwej przeglądarki. Lokalny headless test nie zamyka tych bramek. Nowe CI i audyt dokładnej nakładki są potrzebne przed przyjęciem tej części.

## Następny krok

4B-2 krok 2: wykonywalny chat.send z poprawnym kontekstem, trwałym użytkownikiem/odpowiedzią, operacją, anulowaniem i historią w UI. Budżety, preflight i replay muszą zachować izolację i ograniczenia wcześniejszego wykonawcy. Import 4B-2A oraz pamięć 4B-3/4 pozostają późniejsze.
