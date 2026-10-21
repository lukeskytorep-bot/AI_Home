# AI Home — 4B-4 krok 1: historia i wyłączenie pamięci

21 października 2026. **Wykonano lokalnie; do nowego CI i audytu.** Edward potwierdził zielony wynik 4B-3 kroku 3, bez identyfikatora/platform przebiegu i nowego tekstu werdyktu audytora. To nie zamyka całego 4B-4 ani wcześniejszych odbiorów desktopu.

## Baza i zakres

Dokładna baza: `AI_HOME_4B3_KROK_3_SOURCE.zip`. SHA-256 `22397ef818c43601e9f86ead61b6f8f3385f86d33fa8981f35a0c2aed96b0e90`. Nakładka nie usuwa plików i nie zmienia CI, zależności, lockfile, uprawnień ani SQLite v9/migracji. Baza + rzeczywisty ZIP nakładki są porównane bajtowo z pełnym SOURCE. Plan treść 1.28, raport i dowody są w pełnym inwentarzu; wcześniejsze raporty/snapshoty/logi pozostają bez zmian.

Ten krok wdraża przeglądanie i porównanie wersji L2 oraz jego wyłączenie z kontekstu. Nie wdraża jeszcze restore wcześniejszej wersji, L3/L4/relacji/zamiarów/zasad pracy, retrieval ani trzech brakujących przeznaczeń importu. Czteroczęściowy podział 4B-4 zapisano w aktualnym planie.

## Native i kontrakty

- `memory.versions.list` implementuje istniejący, zamrożony kontrakt P2: recordId + page{limit,cursor?} → pełne MemoryVersion[] + nextCursor. Lista jest od najnowszego zapisu, zakres obejmuje tylko profil i rekord. Każda wersja przechodzi kontrolę DTO/hash/receipt/committed operation. Limit 1–100 pochodzi z P2; UI używa 10.
- `memory.version.get` jest jedynym nowym poleceniem, w osobnym `contracts/memory-history.ipc.json`. Payload recordId/versionId; wynik pełna wersja i MemoryDecisionReceipt. Dane obejmują datę, język, claimKind, źródła/hashy, poprzednią wersję, historyczny pomiar z ID/rewizją trasy i faktyczną decyzję Primary/uzasadnienie. Odczyt nie zmienia pomiaru na aktualny model.
- `memory.outdated.set` korzysta z istniejącego P2. Wymaga expectedRevision rekordu; stare UI nie wyłączy nowej wersji. active→outdated zachowuje pointer i wszystkie wersje. false ponownie włącza **tę samą uprzednio zatwierdzoną wersję**, po kontroli jej hasha i receipt ACCEPT/MODIFY oraz committed operation. Nie wybiera wersji z porównania ani nie jest przywracaniem starszej treści. Nie tworzy nowego approval, MemoryVersion lub operacji modelowej.
- Empty/archived nie mogą zostać aktywowane tym przełącznikiem. Historia rekordu archived jest czytelna w aktywnym profilu; zarządzania archiwum profili nie wdrażamy. Powtórzenie tego samego stanu z bieżącym CAS zwraca istniejący rekord bez zmiany rewizji/zdarzenia.
- Zmiana rekordu, zdarzenie memory.changed i receipt sukcesu są w istniejącej jednej transakcji dispatchera. Awaria zapisu zdarzenia lub receipt wycofuje całość. Dokładne powtórzenie requestId zwraca zapisany wynik; inne dane pod tym samym ID dają IDEMPOTENCY_CONFLICT.

`memory_history.rs::approved()` przejmuje istniejącą walidację z working_context.rs bez osłabiania warunków: profil/recordId/hash, origin=agent_decision, receipt ACCEPT/MODIFY, zgodność candidateMeasurement/Primary route i zakończone committed operation wskazujące ten rekord/wersję. Czat i ponowne włączenie korzystają z jednego helpera. Bez zmian algorytmu Scribe/Primary lub protokołów HTTP. Origin=restore nadal czeka na osobny krok, nie jest milcząco aktywowany.

### Paginacja

Cursor jest samodzielnym kodowaniem base64url wersjonowanej tablicy: marker, profileId, recordId, page limit, granica rowid początku odczytu, ostatni rowid. Każde żądanie ponownie sprawdza zakres i filtruje SQL po profilu/rekordzie. Cursor nie jest sekretnością, podpisem ani poświadczeniem uprawnień; jego ręczna zmiana nie umożliwia odczytu spoza własnego zakresu. Nie odczytuje vault i nie zapisuje rejestru cursorów do bazy. Błędny format, obcy profil/rekord lub inny limit są odrzucane. Granica początku przejścia wyłącza dopisywane w międzyczasie wersje; po odświeżeniu widać nowe. Cursor działa po zwykłym restarcie; nie deklarujemy ważności po przyszłym restore całej bazy, VACUUM lub usuwaniu/renumeracji wersji.

## Kontekst i UI

Wycofana pamięć znika z kolejnego preflight/send, ale źródłowe rozmowy i historyczny kontekst żądań pozostają. Dotychczasowy snapshot/CAS czatu i koordynatora pamięci wykrywa zmianę active/outdated podczas pracy: nie zapisuje spóźnionej odpowiedzi lub kandydatury. To rozstrzygnięcie konfliktu, a nie anulowanie już wysłanego żądania dostawcy lub obietnica zwrotu kosztów.

Panel EN/PL pokazuje jawne znaczenie przełącznika i ostrzega o wpływie na trwające operacje. Przycisk włączenia nie dotyczy wersji wybranej do porównania. Nowa treść nadal wymaga decyzji Primary; przywrócenie starszej wersji będzie miało osobny przepływ.

Historia jest otwierana na żądanie. Pełne treści są obok siebie, na małym ekranie jedna pod drugą; długie treści mają przewijany, dostępny z klawiatury panel, bez obcięcia danych. Porównanie pokazuje identyczne/różne; nie jest algorytmem różnic linia po linii. Karty pokazują komplet źródeł i metadanych decyzji. Tekst React nie wykonuje HTML-a z pamięci.

Odczyty stron i kart ignorują spóźnione odpowiedzi po zmianie aktywnej wersji/wyboru lub unmount. Podwójne szybkie „starsze wersje” daje jeden odczyt; powtarzający się cursor/duplikaty są błędem zamiast zapętlenia. Zmiana aktywnej wersji odświeża przejście po historii. Samo wyłączenie zachowuje wybrane wersje do porównania.

Po sukcesie memory.outdated.set UI stosuje zwrócony rekord i powiadamia czat **przed** kolejnym odczytem. Awaria tego odczytu nie przywraca starego stanu UI. Konflikt odświeża stan bez automatycznego ponawiania mutacji i nie usuwa szkicu ustawień. Przy niepewnej odpowiedzi ponowienie ze starą rewizją nie może niechcący wykonać przeciwnej operacji; CAS wymusza odświeżenie.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all targets, locked/offline | **264/264 PASS** |
| Nowy memory_history | **14/14 PASS**, wliczone wyżej |
| Frontend, 11 plików | **154/154 PASS** |
| Nowe przypadki frontendowe | **17 PASS**: 12 przeglądarki, 5 integracji panelu |
| TypeScript/Vite build | **PASS** |
| Granice modułów | **24/24 PASS** |
| Referencje P2 | **193/193 PASS** |
| Frozen P2/foundation, SQLite/migracje, zależności/lockfile, provider/vault, koordynator i czysty silnik pamięci, CI/capabilities | **Bez zmian bajtowych** |
| Pełny inwentarz, manifesty, zgodność nakładki i SOURCE | **PASS**, osobny log |

Rust: 79 unit/provider/vault, 31 chat, 19 chat_memory, 24 connections, 10 conversations, 8 foundation, 19 imports, 30 memory, 14 memory_history, 30 memory_runtime. Nowe testy używają rzeczywistego dispatchera, SQLite i loopback z testowym vault: pusta historia bez modelu/przy awarii vault; pełna historia i approval bez zapisów/HTTP; stabilna paginacja przy nowej wersji i scope/limit; stary CAS i receipt replay; rzeczywiste wyłączenie z czatu/włączenie bez nowej wersji; zmiana w czasie chat i memory update; izolacja oraz odrzucenie REJECT przy włączeniu; uszkodzony hash historyczny; rollback po awarii receipt/zdarzenia; plikowy restart i cursor; archived; no-op i dwa równoczesne CAS; MODIFY i niezmienność historycznego pomiaru po zmianie trasy.

UI: pełne treści/pochodzenie/decyzje i brak wywołań modeli, bezpieczny tekst HTML, paging, świeży odczyt, porównanie identycznych treści, PL/empty, późne odpowiedzi kart i listy, bezpieczne błędy, zapętlony cursor, zachowanie wyboru po wyłączeniu, szybkie podwójne kliknięcie; dodatkowo integracja przełącznika z CAS, zachowaniem szkicu, powiadomieniem czatu przed nieudanym reload, niepewną odpowiedzią i leniwym otwarciem. Mocki sprawdzają payload/result względem kontraktów.

Logi: docs/evidence/4b4-step-1. Użyto istniejących node_modules i Cargo cache z niezmienionymi lockfile; nie wykonano nowego npm ci ani aktualizacji zależności. Rust 1.99.1, no-default-features. Zachowano istniejące ostrzeżenia dead_code/unused imports; nie ukrywano ich. Nie wykonano kompilacji desktop feature/Windows/Tauri/WebView, rzeczywistego vault lub live API. Responsywny CSS i testy DOM nie zastępują odbioru wizualnego/klawiatury w natywnym WebView.

## Nałożenie i dalszy zakres

Rozpakować CHANGED_FILES.zip w głównym katalogu AI_Home, zastąpić pliki, commit i push. Niczego nie usuwać. Bez cleanup, tworzenia nowych lockfile, migracji, nowych workflow, release lub instalatorów. Do audytu przekazać SOURCE, raport, plan 1.28, SHA256 i OVERLAY_VERIFICATION.log.

Po CI/audycie kroku 1 następny krok to przywracanie starszej wersji z ponowną decyzją Primary, snapshotem i atomowym zapisem nowej wersji. Następnie pozostałe warstwy/retrieval i przeznaczenia importu według planu. **Całe 4B-4 pozostaje otwarte.**
