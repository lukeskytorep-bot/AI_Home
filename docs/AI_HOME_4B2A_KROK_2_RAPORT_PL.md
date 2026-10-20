# AI Home 4B-2A krok 2 — rozmowa o dokumentach

21 października 2026. Kandydat do nowego CI i niezależnego audytu. Edward potwierdził zielone CI kroku 1; nie dostarczono numeru przebiegu, zakresu platform ani nowego PASS audytora. Całe 4B-2A pozostaje otwarte.

## Zachowanie

Po imporcie użytkownik zaznacza gotowe dokumenty i wpisuje pytanie. „Sprawdź kontekst” wykonuje lokalny preflight, bez HTTP i bez zapisania wiadomości. „Wyślij” ponownie wykonuje tę samą walidację, zapisuje wiadomość i źródła atomowo, a dopiero potem uruchamia dostawcę. Do modelu trafia pełny tekst ekstrakcji, nazwa, ID i rewizja materiału, oba hashe, metoda ekstrakcji oraz osobne autorstwo i importer. Oryginał nie jest przesyłany. UI EN/PL opisuje tę granicę przed wysłaniem.

Dokumenty to niezaufane dane w wiadomości użytkownika, oddzielone serializowaną strukturą JSON. Nie stają się system promptem, pamięcią ani zweryfikowanymi zasadami. Instrukcja systemowa wskazuje, że treść dokumentów i deklaracje autorstwa nie mają wyższych uprawnień. To granica kontekstu, nie gwarancja odporności modelu na prompt injection.

## Trwałość i walidacja

chat.send zachowuje zamrożony kontrakt P2. Native wymaga maksymalnie 12 unikalnych, staged i ready dokumentów w dokładnym profilu/rozmowie. Nie przyjmuje ścieżek ani treści pliku z UI. Pending, brakujące, obce, powtórzone lub już związane załączniki są odrzucane przed zapisem/HTTP, kodami istniejącymi w Error P2.

SQLite v6 → v7 dodaje message_documents: uporządkowane snapshoty pełnych metadanych materiału i załącznika z momentu przyjęcia. Wiadomość ma SourceRef(kind=import, materialId, extractedSha256, extractedBlobId). Snapshot materiału zawiera także rewizję, oryginalny hash i autorstwo. FK obejmują profil, wiadomość, załącznik i materiał. W jednej transakcji powstają wiadomość człowieka, powiązania, zmiana staged → bound, snapshoty, rewizja rozmowy, operacja, receipt i zdarzenia. Błąd dowolnego zapisu wycofuje całość, bez HTTP i bez częściowo związanych dokumentów.

Kolejne tury odtwarzają tekst i metadane z przyjętych snapshotów, nie z aktualnych metadanych importu ani pliku na dysku. Sprawdzane są zakres, DTO, zgodność SourceRefs, blobów i SHA-256. Zmiana bieżącego autorstwa nie zmienia odtworzonej wcześniejszej wiadomości. Brak/tampering źródła zatrzymuje kolejne wywołanie. Własna mutacja danych bez uprawnień SQL nie jest oferowana przez IPC.

Bound nie można odłączyć panelem ani powtórnie przypiąć. Dokument jest już dostępny w historii następnych tur. Odłączenie staged zachowuje oryginał. STOP i niepowodzenie generacji zachowują przyjętą wiadomość człowieka ze źródłami. Restart zachowuje wynik i receipts, nie ponawia HTTP. Nie dodano trwałego kasowania ani reattach.

## Kontekst i interfejs

Nowy osobny contracts/document-chat.ipc.json opisuje read-only chat.preflight, ze zgodnym z P2 payloadem chat.send i expectedRevision rozmowy. Wynik identyfikuje ID/rewizję trasy, szacowane wejście, wyjście, rezerwę, zadeklarowany kontekst i liczbę dokumentów. Wspólne prepare() jest używane ponownie przez send; wynik preflight nie jest rezerwacją ani autoryzacją późniejszej zmienionej konfiguracji.

Pomiar uwzględnia pełną historię, tekst i serializowane metadane dokumentów, system instruction, prywatny provider replay, narzut 64/wiadomość, limit wyjścia oraz rezerwę. To jawny szacunek bajtów UTF-8, nie tokenizer ani gwarancja pojemności modelu. Przekroczenie okna lub ochronnego limitu 8 MiB zatrzymuje wysyłkę bez obcinania. Ograniczenia transportu nadal obowiązują. Duże dokumenty mogą wymagać większego zadeklarowanego okna; retrieval/chunking nie jest zaimplementowany.

Pomiar znika po zmianie pytania, języka, wybranych dokumentów lub rewizji rozmowy/trasy. Spóźniony preflight nie nadpisuje nowego szkicu. Próba o nieznanym wyniku zachowuje identyczne requestId/payload/expectedRevision mimo edycji nowego szkicu lub wyboru dokumentów; osobny przycisk rozlicza starą próbę. Accepted rozlicza oryginalne źródła i szkic także po odmontowaniu panelu, przed zawodnym odczytem historii. Selekcje są rozdzielone na profile i rozmowy w rodzicu. Po Accepted odświeżany jest stan załączników. Historia pokazuje ID/hash źródeł przy wiadomości człowieka; nie przypisuje ich jako zweryfikowanych cytowań odpowiedzi AI.

## Weryfikacja

| Kontrola | Wynik |
| --- | --- |
| Rust Linux, bez desktop feature, --all-targets --locked --offline | 153/153 PASS |
| Frontend Vitest, jsdom | 109/109 PASS |
| TypeScript i Vite | PASS |
| Granice modułów | 21/21 PASS |
| Zamrożone P2 | 193/193 PASS |
| Migracja rzeczywistej bazy plikowej v6 → v7 i restart | PASS |
| Nakładka na dokładną bazę daje pełne źródła | sprawdzenie w OVERLAY_VERIFICATION.log |
| Manifest i SHA256SUMS | sprawdzenie w OVERLAY_VERIFICATION.log |

Nowe 7 testów Rust obejmuje: rzeczywisty material.select/import → preflight → send → następną turę, tekst Unicode/znaczniki i niezaufane instrukcje, autorstwo AI/importer człowiek, snapshot mimo zmiany bieżących metadanych, hash/blob w SourceRefs, bound/detach, zakres rozmowy i duplikaty/pending, przepełnienie, rollback błędu receipt, uszkodzenie ekstrakcji, STOP/idempotencję oraz rzeczywistą migrację/restart i identyczny wire przy retry. Dotychczasowe testy profili, providerów, prywatnego replay, receipts i importu pozostają częścią pełnego uruchomienia. Zmiana oczekiwanego user_version na 7 jest jedyną zmianą starszych testów migracji (poza usunięciem nowej tabeli w fixture starszego schematu).

Nowe 6 testów UI sprawdza kontrakt preflight i brak chat.send przy pomiarze, unieważnienie wyniku, dokładną starą próbę mimo zmiany tekstu i wyboru, Accepted po odmontowaniu, odrzucenie spóźnionego pomiaru, zaznaczanie gotowego dokumentu i blokadę pending. Dwa testy zaznaczania w panelu plus cztery w czacie dają 6 nowych testów; liczba wzrosła ze 103 do 109.

Pierwszy Rust run miał 21/22 PASS w zestawie czatu: nowa fixture kopiująca bazę in-memory przez VACUUM INTO dawała pusty stan przy odczycie. Test zmieniono na rzeczywisty plik od jego utworzenia, migrację i dwukrotne otwarcie; runtime nie wymagał obejścia. Zachowano initial-restart-fixture-failure.log i restart-regression.log. Pierwszy UI run miał 105/106 PASS: stara asercja oczekiwała napisu „jeszcze niewysyłane”; zaktualizowano ją do rzeczywistego ostrzeżenia o wysłaniu. Zachowano initial-outdated-label-failure.log. Końcowe pełne logi wskazują 153/153 i 109/109. Nie zmieniano watchdogów, asercji budżetów ani limitów aplikacji w celu uzyskania zielonego wyniku. Ostrzeżenia dead_code i wcześniejsze unused imports pozostają widoczne.

Nie wykonano: desktop feature, Windows, natywnego dialogu/pobierania w WebView, rzeczywistego OS vault, płatnych/live API ani renderowania w przeglądarce. jsdom nie stanowi ich odbioru. Nowy wynik GitHub CI jest wymagany; zielona baza go nie zastępuje. Nie wykonano release ani instalatorów.

## Źródła i zakres

Dokładna baza: AI_HOME_4B2A_KROK_1_SOURCE.zip, SHA-256 `d3d4327971f3af1a7dde232caf3054aae3428fcf2bf4869358677c2c87c81fe8`. Nakładka jest przeznaczona wyłącznie na tę bazę, bez kasowania plików. Wykorzystuje gotowy import i providerów z wcześniejszych kroków; nie kopiuje ich ponownie ani nie zmienia. Nie ma nowych zależności, zmian lockfile, workflow, capabilities lub zamrożonych kontraktów. Wszystkie starsze migawki/dowody zachowano bajtowo. Dodano document_context.rs oraz osobny suplement IPC.

Przejrzano dostarczone dokumenty: przewodnik społeczności Farsight v1.0, architekturę sieci v2.0 i Document 4B v1.3. Istotne tu są niezależność Agent Home, granica niezaufanych wejść oraz pochodzenie źródeł; integracji forum/A2A/Conscious nie uruchamiano. Ich hashe zapisano w status.json; samych dokumentów nie kopiowano do repozytorium. Obowiązujące rozszerzenie 4B-2A i szczegóły etapów są utrzymane w kanonicznym planie treści 1.21.

Następny krok po CI/audycie: obrazy/vision z kontrolą możliwości trasy i właściwym provider payloadem. OCR, RAG, pamięć/Scribe i pozostałe integracje są później.
