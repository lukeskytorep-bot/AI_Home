# AI Home — 4B-4 krok 3: warstwy pamięci i wyszukiwanie

22 października 2026. Plan treść 1.30. **Wykonane lokalnie; do nowego CI i audytu.** Edward potwierdził zielony wynik kroku 2, bez numeru/platform przebiegu lub nowego tekstu audytu. Nie oznacza to zamknięcia wcześniejszych bramek desktopu ani całego 4B-4.

## Dokładna baza i przekazanie

Baza: AI_HOME_4B4_KROK_2_SOURCE.zip, SHA-256 `514fc3c97d7f15eda87dbfbeb389b1ec1348e2d29139d07d9321ca635be9e970`. Weryfikator potwierdził 349 plików, 347 hashy manifestu i 348 wpisów SHA256SUMS. Nie użyto innej migawki Harness lub przypadkowej kopii repozytorium.

Nakładka zawiera wyłącznie nowe i zmienione pliki, z płaskimi ścieżkami od głównego katalogu repozytorium. Pełny SOURCE jest dla audytu. Dokładne liczby i hashe ZIP-ów są w zewnętrznym SHA256 oraz OVERLAY_VERIFICATION.log; nie umieszczamy hasha archiwum wewnątrz niego. Manifest obejmuje także ten raport, plan i nowe logi. Niczego nie usuwać. Weryfikator źródeł i poprzednie dowody zachowane bajtowo.

## Zmiana zachowania

- Nowe rodzaje: episodic, relationship, longTerm, intent i workRule. Wiele rekordów na profil; working nadal jeden. Człowiek tworzy pusty rekord, podaje etykietę i opcjonalny subjectRef. Program nadaje ID/daty. Nie ma ręcznego przycisku udającego zatwierdzenie modelu.
- Aktualizacja treści korzysta z istniejącego silnika Scribe/Primary: natywne SourceRefs rozmowy, język źródeł lub jawna polityka tłumaczenia, propozycja i osobna decyzja Primary. LIMIT/MODIFY mierzą rzeczywisty tekst. Dwa podejścia pojemności, naprawa formatu, deadline i wspólny budżet fizycznych wywołań pozostają.
- Każdy rodzaj korzysta ze swojej MemorySettings policy. Własne ustawienia pięciu warstw nie modyfikują working; ustawienia L2 nie modyfikują pozostałych. RetrievalTargetTokens domyślnie 1200, dodatni zgodnie z P2. Nie dodano zera do zamrożonego ContextBudgetPolicy.
- Korekta zapisanej treści wymaga powodu. Nowa wersja wskazuje previousVersionId; osobne niezmienne metadane wersji zawierają powód i ludzkie adnotacje z chwili decyzji. Sprzeczna propozycja nadal może zostać odrzucona przez Primary; wtedy poprzednia treść pozostaje. Powtarzanie nie stanowi dowodu prawdziwości.
- Metadane kontaktów pochodzą z faktycznych messageId/conversationId/createdAt/role/language/sequence. Ostatnia data jest wyliczana według czasu, a nie generowana przez model. subjectRef jest adnotacją człowieka, nie weryfikacją tożsamości. Źródła wskazują zachowane wiadomości.
- Intent używa zamrożonego DTO z importance, trigger, status i expiresAt. Adnotacje są jawnie ludzkie. Open po expiresAt ma pochodny effectiveIntentStatus=expired i nie trafia do retrieval; zapisana historia nie jest nadpisywana. Trigger nie uruchamia harmonogramu ani zadania.
- Historia, porównanie, outdated i przywracanie przez świeżą decyzję Primary działają także dla pozostałych rodzajów. Przywracanie tekstu nie cofa aktualnego statusu zamiaru. Historyczne kontakty i źródła są zachowane, z nowym restoredFromVersionId i approval.

## MemoryRetriever i kontekst rozmowy

Czysta, wymienna granica MemoryRetriever oraz LexicalRetriever. Dokładne tokeny słowne Unicode po lowercase, bez semantyki/odmiany/synonimów. Ranking po liczbie różnych dopasowań, potem ważności zamiaru i ID. Wyszukiwane są etykieta, subjectRef i zatwierdzona treść. Powtarzanie wyrazu nie daje dodatkowych punktów.

Zapytanie to ostatnie 4096 znaków nowej wiadomości. Tylko active, zatwierdzone wersje poza working; intencje dodatkowo open i niewygasłe. Najwyżej 20 całych wpisów mieszczących się w budżecie. Koszt to cały wysyłany blok user/JSON z pochodzeniem i ramką plus 64 na wiadomość, według estymatora UTF-8. Zbyt duży wpis jest pomijany, nie obcinany; dalszy mniejszy może się zmieścić. Wynik podaje matchedCount, omittedCount, inputTokens i pełne wybrane wersje/hash/źródła. Nie twierdzi, że model odczytał resztę magazynu. Świeży pomiar treści wiąże bieżące ID/rewizję Primary, nie modyfikuje historycznego pomiaru wersji.

Chat.preflight i send mają wspólne prepare. Selekcja jest doliczana do pełnej historii, L2, dokumentów/obrazów/replay, odpowiedzi i rezerwy. Unlimited nie omija tej bramki. WorkRule i inne wspomnienia są omylnymi danymi user/JSON po stałej instrukcji systemowej, bez przyznawania uprawnień.

Snapshot selekcji trafia atomowo do chat_runs wraz z przyjęciem wiadomości. Ponowna selekcja jest porównywana przed każdym fizycznym POST i końcowym commit; zmiana wersji, wyłączenie, status zamiaru lub wygaśnięcie uniemożliwia zapis spóźnionej odpowiedzi. chat.retrieval.get odczytuje dokładny historyczny zestaw bez uruchamiania obecnego wyszukiwania. Stare żądania bez pola mają known=false; dane nie są uzupełniane dzisiejszą pamięcią. Historyczny odczyt sprawdza także hash/receipt/powiązania rekordu, zakres profilu, historyczną trasę pomiaru oraz sumę pełnych bloków i budżet; wykryta niespójność kończy się STORAGE_ERROR. Captured oznacza przyjęty zestaw do żądania, nie dowód odebrania go przez dostawcę.

Ochrona skanu: ponad 10000 aktywnych rekordów lub 32 MiB treści ich aktualnych wersji kończy się jawnym CONTEXT_BUDGET_EXCEEDED. Nie zwracamy arbitralnie pierwszych rekordów jako kompletnego wyniku. To pierwsza implementacja, nie zoptymalizowany indeks dużego magazynu. Pojemność zapisu, retrieval i okno modelu są niezależne.

## Kontrakty, baza i UI

Frozen P2/foundation, zależności/Cargo.lock/package-lock, provider/vault, workflow, capabilities i uprawnienia bez zmian bajtowych. Istniejący własny document-chat.ipc.json dodaje opcjonalny wynik retrieval; native zwraca go w nowych preflight. Nowy własny memory-layers.ipc.json ma 10 poleceń: records.list, record.create/get/metadata.set/update, layers.settings.get/set, search, version.annotations.get, chat.retrieval.get. Każde ma scope profilu, dokładny payload/result i jawne wymaganie CAS. Nie rozszerzono frozen MemoryRecord/MemoryVersion obcymi polami. Błędne payloady, w tym zera budżetu, są odrzucane.

SQLite 9→10 dodaje tylko memory_metadata i memory_version_metadata, z composite FK do scoped rekordów/wersji. Profile, rozmowy, pamięć, receipts i wcześniejsze wersje nie są przepisywane ani usuwane. Stare testy migracji zmieniają wyłącznie oczekiwaną docelową wersję 9→10; kontrole zachowania danych pozostają. Nowa integracja odtwarza bazę v9 z zatwierdzoną pamięcią i sprawdza migrację oraz foreign_key_check.

Panel EN/PL otwierany jawnie w rozmowach: puste rekordy, rodzaj/etykiety/intencje, niezależne limity i unlimited z ostrzeżeniem, wyszukiwanie z pominięciami, aktualizacja z powodem, historia/porównanie/restore. Adnotacje starszych wersji dostępne osobnym scoped odczytem. Angielski pozostaje domyślny. Niepewne zapisy zachowują komendę, target, requestId i CAS po zmianie rozmowy. Accepted rozstrzyga próbę przed następnym odczytem. Spóźniona odpowiedź z poprzednio zamontowanego panelu jest uzgadniana bez tworzenia kolejnej operacji.

## Weryfikacja lokalna

| Kontrola | Wynik |
|---|---:|
| Rust Linux, no-default-features, all-targets, locked/offline po pobraniu zależności | 316/316 PASS |
| Frontend, 12 plików Vitest/jsdom | 181/181 PASS |
| TypeScript + Vite build | PASS |
| Granice modułów | 26/26 PASS |
| Niezmienione kontrole referencyjne P2 | 193/193 PASS |
| Baza + rzeczywista nakładka = rzeczywisty SOURCE | PASS — osobny log przekazania |
| Manifest, SHA256SUMS i brak usunięć | PASS — osobny log przekazania |

27 nowych testów Rust: 2 czystego rankingu +25 integracji rzeczywistego dispatchera/SQLite/loopback. Obejmują wszystkie rodzaje, approval/MODIFY/dwa odrzucenia/STOP, scope, receipts/CAS, niezależność limitów, powody i kontakty, historyczne adnotacje, intencje/wygaśnięcie, dokładny próg pełnego bloku retrieval, kontekst unlimited, przechwycony zestaw i spóźniony wynik, bieżącą trasę, migrację oraz restart. 17 nowych testów UI: 15 bibliotek i niepewnych zapisów/remount +2 kontekstu czatu. Mocki sprawdzają rzeczywiste payload/result według kontraktów, nie tylko liczbę wywołań.

Node 24.19.0, npm 11.9.0, rustc 1.99.1. Wykonano npm ci --ignore-scripts na niezmienionym lockfile; nie aktualizowano zależności. Logi końcowe w docs/evidence/4b4-step-3; zachowano także wybrane logi nieudanych prób rozwojowych. Dotyczyły m.in. oczekiwanej wersji migracji, błędnego testowego wywołania configure/route.save, etykiet/typów testów UI oraz niezgodnego z P2 zera retrieval. Poprawiono przyczyny, bez zmiany frozen kontraktu. Test wcześniejszego CAS ujawnił potrzebę zachowania REVISION_CONFLICT dla obcego rekordu w update/restore; zachowano dotychczasowy błąd i regresję.

Nie wykonano Windows/Tauri/WebView, systemowego magazynu kluczy ani live API. Nie utworzono nowego release/instalatora/workflow. Zestaw headless nie zamyka tych odbiorów. Nie uruchomiono płatnych wywołań ani prawdziwych kluczy.

## Dalej

Nowe CI i audyt bieżącego kroku. Następnie 4B-4 krok 4: pozostałe przeznaczenia importu — wiedza profilu, stałe wskazówki, kandydatury pamięci — z pochodzeniem i rozdzieleniem autora od importera. Import rozmowy działa jak wcześniej. Całe 4B-4 pozostaje otwarte.
