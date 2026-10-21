# AI Home — 4B-3 krok 3: zatwierdzony Working Context w czacie

21 października 2026. **Wykonano lokalnie; do nowego CI i audytu.** Edward potwierdził zielony wynik poprawki kroku 2. Nie dostarczono numeru/platform przebiegu ani nowego tekstu werdyktu. Nie ogłaszamy PASS CLOSED całego 4B-3 lub wcześniejszych bramek desktopowych.

## Dokładna baza

`AI_HOME_4B3_KROK_2_AUDIT_FIX_SOURCE.zip` — SHA-256 `738270c6c170e47371113fd5ae02f662d95ae268318e4cae93f4798151e16716`. Nakładka nie usuwa plików. Pełne źródła i odtworzenie baza + rzeczywisty ZIP nakładki są porównane bajtowo. Osobny plan i raport są identyczne z kopiami w SOURCE. Hashe obejmują całe aktualne drzewo; wcześniejsze snapshoty i dowody pozostają bez zmian.

## Zachowanie

- Do każdego nowego żądania trafia jedna aktywna, zatwierdzona wersja Working Context wybranego profilu. Oddzielny blok user zawiera JSON treści i pochodzenia (record/revision, version/number/hash, język, claimKind, origin, receipt i SourceRef). Stała instrukcja systemowa określa, że są to omylne dane referencyjne. Treść pamięci nie staje się system promptem ani rolą rozmowy. To granica reprezentacji/interpretacji, nie gwarancja odporności modelu na prompt injection.
- Pełna historia, dokumenty, obrazy i natywne replay pozostają. Nie odtwarzamy dawnych bloków pamięci jako kolejnych wiadomości historii. Pusta, outdated i archived pamięć jest wyłączona z nowych żądań; nie usuwamy wersji. Nie korzystamy z niezatwierdzonych propozycji lub decyzji REJECT.
- Native sprawdza powiązania profilu/rekordu/wersji, hash treści, receipt ACCEPT/MODIFY z właściwą historyczną trasą/pomiarem oraz zakończoną operację committed wskazującą tę wersję. Obecnie obsługiwany origin to agent_decision; restore pozostaje do 4B-4.
- chat.preflight i chat.send korzystają z tego samego prepare(). Pomiar obejmuje pełny JSON pamięci, metadane i narzut, wszystkie wiadomości, replay, obrazy, limit wyjścia i rezerwę. Nie ma cichego skracania. Nadal jest to szacunek bajtowy, a nie tokenizer dostawcy ani gwarancja jego akceptacji.
- measurement pamięci jest ponawiany dla aktualnego ID i revision Primary. Historyczne MemoryVersion/TokenMeasurement nie są przepisywane po zmianie modelu. inputTokens obejmuje cały blok; ten składnik jest już zawarty w estimatedInputTokens.
- Unlimited ogranicza wyłącznie politykę zapisu, nie okno modelu. Zmniejszenie pojemności nie obcina starej wersji. Zbyt duże pełne żądanie jest odrzucane przed Operation/receipt/wiadomością/HTTP. Nowa trasa nadal wymaga swojej deklaracji okna czatu.

## Snapshot, CAS i trwały odczyt

W jednej transakcji z przyjęciem chat.send zapisujemy w istniejącym JSON chat_runs pełny workingContext: schemaVersion=1, MemoryRecord, MemoryVersion i receipt albo jawny brak wersji/receipt. Nie jest potrzebna migracja; SQLite pozostaje v9. Snapshot nie zawiera klucza API.

Istniejący RecordAttempt ponownie sprawdza cały przechwycony rekord i wersję przed każdym fizycznym POST, również retry/recovery. Ten sam warunek jest sprawdzany przed końcowym commit. Zmiana rekordu, aktywnej wersji lub empty→active odrzuca spóźniony wynik z REVISION_CONFLICT. Hash wykrywa także zmianę treści bez revision. Dotychczasowe kontrole profilu, Primary, połączenia/klucza/endpointu i vision nadal obowiązują. STOP nie tworzy odpowiedzi asystenta; receipt retry zachowuje tożsamość wcześniejszego żądania.

W teście retry wykryto osobny błąd licznika: po zablokowaniu drugiej próby przed HTTP końcowy chat.run przepisywał wartość z rezerwacji w pamięci. Teraz zachowuje trwały physicalCallCount z RecordAttempt, zapisany dopiero po kontroli snapshotu. Limit transportu pozostaje bez zmian; odrzucona próba nie wraca do wykonania. Licznik oznacza zatwierdzone przed dispatch próby, nie dowód odbioru przez dostawcę.

Nowy własny suplement `memory-chat.ipc.json` dodaje tylko odczyt `chat.context.get` z payload `{"operationId":"…"}`. Wynik known:true zawiera descriptor przechwyconej pamięci; known:false oznacza starszy run bez snapshotu. Odczyt jest profile-scoped i dostępny także po restarcie, błędzie lub STOP. Nie dodano pól do zamrożonego Operation.result/Message. Nowy runtime nie kontynuuje runów bez snapshotu; otwarte operacje przy starcie pozostają interrupted według istniejących zasad.

Własny suplement `document-chat.ipc.json` wymaga workingContext w wyniku preflight. Przykład braku: `{"included":false,"recordId":"working-…","recordRevision":1,"state":"empty"}`. Wersja aktywna ma included:true, state:active, versionId/number, hash, language, decisionReceiptId, measurement (bieżąca trasa) i inputTokens (pełny blok). Reszta preflight i frozen chat.send nie zmieniają wejścia.

## UI

EN pozostaje domyślny; PL ma te same klucze. Preflight pokazuje wersję/hash/język i koszt wejścia pamięci. Historia pobiera przechwycony kontekst po operationId, w paczkach do 8 równoległych odczytów. Nie zastępuje historycznej pamięci dzisiejszą wersją. Pokazuje jawne unknown dla starych żądań. Etykieta „Captured” oznacza snapshot przy Accepted, a nie dowód wysłania do dostawcy.

MemoryPanel powiadamia czat o zmianie odczytanego rekordu/wersji; unieważnia to preflight oraz spóźnione wyniki tego odczytu, zachowując szkic wiadomości i niezapisane ustawienia pamięci. Odczyt/polling panelu nie wykonuje modeli. Zmiana z innego procesu jest rozstrzygana przez świeży native prepare/CAS; preflight pozostaje doradczy. Nie dodano automatycznego Scribe po każdej rozmowie. Nieaktywna zapisana wersja ma w panelu ostrzeżenie o wyłączeniu z nowych żądań.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all targets, locked/offline | **250/250 PASS** |
| Nowe integracje chat_memory | **19/19 PASS**, wliczone wyżej |
| Frontend, 10 plików | **137/137 PASS** |
| TypeScript + Vite build | **PASS** |
| Granice modułów | **23/23 PASS** |
| Referencje P2 | **193/193 PASS** |
| P2/foundation, provider/vault, dependencies/lockfiles, workflow/capabilities, SQLite/migracje, czysty silnik i koordynator pamięci | **Bez zmian bajtowych** |
| Baza + nakładka = SOURCE; manifesty i pełny inwentarz | **PASS**, osobny log |

Rust obejmuje 79 unit/provider/vault, 31 chat, 19 chat_memory, 24 connections, 10 conversations, 8 foundation, 19 imports, 30 czystej pamięci i 30 memory_runtime. Nowe przypadki: brak pamięci; zatwierdzona wersja/JSON/role/pełna historia; świeży pomiar po zmianie revision i ID Primary; mniejsza pojemność; outdated/archived; unlimited przepełniające czat; zmiana przed pierwszym POST, przed retry i przed commit; nowa lub zastąpiona wersja podczas POST; STOP/idempotencja; izolacja profili; uszkodzony hash i REJECT; legacy unknown; rzeczywisty restart plikowej bazy; atomowość po błędzie receipt; Scribe automatic→czat Primary bez dodatkowego Scribe; pełny dokument+obraz w historii; dokładny przyrost estymaty o jeden blok pamięci z metadanymi.

Siedem nowych testów UI obejmuje descriptor preflight, odczyt historyczny, legacy/wyłączenie PL, unieważnienie po zmianie pamięci i spóźniony preflight, powiadomienie bez kasowania szkicu oraz ostrzeżenie o nieaktywnej wersji. Mocki sprawdzają kształty nowego IPC; native waliduje odpowiedzi względem suplementów. Zmieniono wcześniejsze oczekiwanie tekstu „not yet added to chat”, bo integracja jest teraz wykonana.

Logi: `docs/evidence/4b3-step-3/`. Pozostałe istniejące ostrzeżenia dead_code/unused imports oraz lokalne ostrzeżenie npm o http-proxy pozostają jawne. Nie wykonywano nowego npm ci: wykorzystano lokalne node_modules zgodne z niezmienionym lockfile, pochodzące z poprzedniej bazy. Cargo użył lokalnego cache/toolchain 1.99.1 i niezmienionego Cargo.lock.

Nie uruchomiono desktop feature, kompilacji Windows, natywnego Tauri/WebView, rzeczywistego systemowego vault ani live API. Loopback używa Custom OpenAI i wstrzykniętego testowego vault; wspólne testy protokołów nie są odbiorem połączeń z dostawcami. Zachowanie layoutu i podglądu/CSP w realnym WebView nadal wymaga osobnego odbioru.

## Nałożenie i kolejny etap

Rozpakować CHANGED_FILES.zip w głównym katalogu repozytorium, zastąpić pliki, commit/push. **Bez usuwania plików, cleanup, instalowania zależności lub zmiany CI/release.** Zachowany workflow wykona zwykłe kontrole. SOURCE służy do audytu/odtworzenia dokładnego drzewa. Plan zaktualizowano do treści 1.27.

Po zielonym CI i audycie kroku 3 można przejść do 4B-4: pozostałe rodzaje pamięci, historia/wycofanie/porównanie/restore oraz przeznaczenia importu według planu. Nie zamykamy wcześniejszych odbiorów realnych usług i desktopu samym wynikiem headless.
