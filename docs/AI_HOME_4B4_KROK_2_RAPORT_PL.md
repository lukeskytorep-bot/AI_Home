# AI Home — 4B-4 krok 2: przywracanie starszej wersji pamięci

21 października 2026. **Wykonano lokalnie; do nowego CI i audytu.** Edward potwierdził zielony wynik kroku 1, bez numeru/platform przebiegu i nowego tekstu audytu. Nie ogłaszamy PASS CLOSED całego 4B-4 ani wcześniejszych odbiorów desktopu i rzeczywistych usług.

## Baza

`AI_HOME_4B4_KROK_1_SOURCE.zip`, SHA-256 `819bc1409e1d9a7bd406048b541c9adb97ccb39834770d7399b16adc4692c388`. To dokładna zielona baza kroku 1. Brak usunięć. SOURCE jest porównany bajtowo z odtworzeniem baza + rzeczywisty ZIP nakładki. Plan treść 1.29 i raport są identyczne z kopiami w SOURCE; pełny manifest obejmuje dokumentację i dowody. Poprzednie snapshoty, raporty i dowody pozostają bez zmian.

## Przepływ

1. Użytkownik wybiera starszą wersję w pierwszej kolumnie porównania i jawnie zleca memory.restore. UI wysyła wyłącznie recordId/versionId i CAS rekordu według zamrożonego P2. Nie przekazuje treści, decyzji ani approval.
2. Native rozwiązuje własny L2 i obie wersje; sprawdza hash, profil, recordId, receipt ACCEPT/MODIFY i zakończony wynik wskazanej wersji. Przyjmowany jest starszy zapis w active/outdated; empty/archived i aktualna wersja są odrzucane. Bieżącą wycofaną wersję nadal włącza osobny przełącznik. Równoległe memory.update/restore tego rekordu są blokowane.
3. Kandydatura jest dokładną kopią starszej zatwierdzonej treści. Engine::review_existing nie wykonuje modelu ani nie udaje odpowiedzi propozycji; zaczyna od oczekiwania na nową decyzję bieżącego Primary. Początkowy MemoryProposal ma historyczną trasę Primary zatwierdzającą źródło jako pochodzenie, a memory_runs.restoreProposalId i restoreSource jednoznacznie oznaczają kopię, nie świeże wygenerowanie. UI opisuje ją jako kopię wcześniej zatwierdzonej wersji.
4. Primary otrzymuje historyczny tekst, obecną pamięć, język/claimKind/datę/hash i referencje źródeł. Wszystko zmienne jest JSON user data, nie instrukcjami systemowymi. Nie wczytujemy ponownie oryginalnych wiadomości/plików; system jawnie zabrania twierdzenia, że zostały ponownie przeczytane. Pochodzenie nie stanowi dowodu prawdziwości treści.
5. Primary osobno ACCEPT/MODIFY/REJECT. ACCEPT i MODIFY tworzą **nową** MemoryVersion z nowym ID/datą/hash/pomiarem i receipt. origin=restore, restoredFromVersionId=wersja wskazana, previousVersionId=wersja zastana. MODIFY zapisuje rzeczywisty tekst i claimKind=mixed. Zachowany jest historyczny język źródła, niezależny od EN/PL UI.
6. REJECT kończy restore jako failed/MEMORY_PRIMARY_REJECTED bez zmiany rekordu/wersji; receipt z publicznym uzasadnieniem pozostaje. Zamrożony RestoreOperationResult nie ma wariantu no_change. Nie zmieniamy P2 w celu dodania takiego wariantu.

Pierwotne SourceRef pozostają, a nowy zapis ma dodatkową referencję memory_version do wskazanej wersji z jej hashem. Przywrócona wersja może być później ponownie wskazana jako historyczne źródło. Związek źródłowy musi prowadzić do niższego numeru wersji tego samego rekordu i poprawnego hasha; każdy aktywowany zapis ma własny nowy approval. Nie jest to kryptograficzny podpis całej bazy ani ponowny dowód pierwotnych twierdzeń.

## Budżet i integralność

Restore wykorzystuje tylko bieżący Primary i jego zapisane okno pamięci, niezależnie od trybu/aktualności Scribe. Nie wymaga Scribe ani osobnego płatnego generowania początkowej propozycji. Jeden fizyczny call może wystarczyć do pierwszej decyzji; retry/naprawa nadal wymagają pozostałego budżetu.

Przed Operation/receipt/seed/snapshot/HTTP ten sam build() sprawdza pełne **rzeczywiste** pierwsze żądanie decyzji, z oboma tekstami, metadanymi, wyjściem i rezerwą. Nie jest to minimalny szablon z etapu aktualizacji L2. Znana niewykonalność daje zero POST i brak nowych rekordów/zdarzeń. CostLimit inny niż unconfigured jest odrzucany, bo nadal brak estymatora kosztu. ReasoningValue i nieznane okno są odrzucane jak wcześniej.

Jeśli historyczna treść przekracza nową pojemność zapisu, nie jest obcinana: Primary może w pierwszej decyzji MODIFY utworzyć krótszą treść. ACCEPT/MODIFY jest mierzony faktycznie dla bieżącej trasy i rewizji. Po pierwszym przekroczeniu działa druga i ostatnia próba: propozycja **Primary**, następnie osobna decyzja Primary. Komunikat zawiera porażkę pierwszej próby i ostatnią szansę. Druga porażka zachowuje stan poprzedni. Naprawa formatu ma jeden osobny retry; transport retry dzieli najwyżej 4 fizyczne wywołania i 180 s z całym przebiegiem. Pojemność unlimited nie omija okna/output/deadline. Szacunek UTF-8 nadal nie jest tokenizerem dostawcy.

Snapshot w istniejącym memory_runs zawiera profil, rekord, bieżącą wersję, wybraną wersję i jej receipt, trasy/połączenie/credential revision, pojemność i język. Kontrola bieżącego zakresu działa przed każdym fizycznym POST i finalnym commit. Zmiana wskazanego źródła nawet bez revision jest wykrywana przez porównanie pełnego snapshotu; zmiana rekordu, profilu, Primary lub klucza nie pozwala zapisać spóźnionej odpowiedzi. Scribe nie bierze udziału w kontroli trasy restore, bo nie jest wywoływany.

Koordynator pamięci zapisuje proposal/decision/attempts i atomowo tworzy końcową wersję, pointer, wynik i zdarzenia. Restore physicalCallCount zachowuje licznik trwałego RecordAttempt, nie nadpisuje go rezerwacją odrzuconą przed dispatch; ta reguła obejmuje fail i STOP. Licznik nadal oznacza próby zatwierdzone do dispatch, nie dowód odbioru dostawcy.

STOP zamyka pending proposal i zachowuje poprzednią pamięć. Restart korzysta z istniejącej procedury SQLite: pending operacja → interrupted, pending proposal → failed, bez automatycznego wznowienia POST. Awaria receipt przy Accepted wycofuje Operation/seed/snapshot; awaria końcowego zapisu nie aktywuje pustej lub częściowej pamięci. Exact replay requestId zwraca poprzedni Accepted bez ponownej pracy.

## UI i kontrakty

Wykorzystano frozen memory.restore, Accepted, Operation(kind=restore), RestoreOperationResult, MemoryVersion i receipt. **Żaden kontrakt nie został zmieniony.** Nie dodano nowego IPC ani migracji. SQLite pozostaje v9; dependency/lockfile/workflow/capabilities/provider/vault bez zmian. Wspólny helper historyczny dopuszcza teraz origin=restore z kontrolą własnej nowej decyzji, typu wyniku i bezpośredniego źródła. Historia, toggle i czat mogą korzystać z przywróconego zapisu. Stare origin=agent_decision zachowują dotychczasowe warunki.

EN pozostaje domyślny; PL ma komplet nowych etykiet. Panel pokazuje cel, wymóg nowej decyzji, bieżącą operację restore, publiczny wynik/błąd oraz restoredFrom obok previousVersion. Przycisk jest zablokowany dla bieżącej wersji, przy trwającej/niepewnej operacji lub nieweryfikowanym/niezadeklarowanym Primary. Zlecenie używa zapisanych limitów, nie niezapisanych edycji pól.

Niepewna operacja przechowuje command=memory.restore, pierwotny payload/target, requestId i CAS w istniejącej mapie prób profilu, także po przełączeniu rozmowy/remount. „Resolve previous memory request” rozlicza poprzednie restore, nie zamienia go w aktualizację rozmowy. Po Accepted próba jest usuwana przed odczytem statusu, więc błąd reload nie powoduje nowego płatnego requestu pod nowym ID. Ustawienia wpisane przez użytkownika pozostają przy konflikcie. Wynik/STOP/polling jest trwały przez memory.working.get/operation.get. Otwieranie historii nie wywołuje modeli.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust Linux headless, all targets, locked/offline | **289/289 PASS** |
| Nowe memory_restore | **22/22 PASS**, wliczone powyżej |
| Nowe testy czystego review_existing | **3/3 PASS**, wliczone powyżej |
| Frontend, 11 plików | **164/164 PASS** |
| Nowe przypadki UI | **10 PASS** |
| TypeScript/Vite | **PASS** |
| Granice modułów | **24/24 PASS** |
| Referencje P2 | **193/193 PASS** |
| Kontrakty/SQLite/migracje/dependency/lockfile/CI/capabilities/provider/vault | **Bez zmian bajtowych** |
| Manifesty i odtworzenie nakładki do SOURCE | **PASS**, osobny log |

Rust: 79 unit/provider/vault, 31 chat, 19 chat_memory, 24 connections, 10 conversations, 8 foundation, 19 imports, 33 czystej pamięci, 14 historii, 22 restore, 30 memory_runtime.

Nowe integracje sprawdzają: ACCEPT i nowe approval/proweniencję/wersję, MODIFY i faktyczny pomiar po zmianie Primary revision, REJECT active/outdated, reaktywację outdated, automatic Scribe z nieaktualnym własnym oknem przy poprawnym Primary bez wywołania Scribe, obniżony limit i krótkie MODIFY, drugą próbę pojemności, dwie porażki długości/formatu, jedną naprawę JSON, niemożliwy preflight bez POST/zapisów, jeden fizyczny call, stale CAS/obcy profil/aktualna wersja/uszkodzony hash, STOP, wyłączenie w trakcie, zmianę źródła bez revision, retry ze zmianą snapshotu/licznikiem, awarię receipt/finalnego zapisu, wzajemne wykluczenie operacji, restore wcześniej przywróconej wersji, zmianę klucza, retry 503 z tym samym wire, plikowy restart z trwałą nową wersją i przerwaniem pending bez sieci. Czysty silnik wymaga źródła memory_version zgodnego z treścią i nowej decyzji; odrzuca Scribe/niepowiązaną kandydaturę.

UI sprawdza kontrakt/CAS i jawny wybór, niepewną tożsamość po remount, Accepted przed nieudanym reload, niezależność od Scribe, blokadę bieżącej wersji i nieweryfikowanego Primary, wynik bez brakującego outcome, odrzucenie, konflikt bez resubmit/skasowania szkicu, restoredFrom oraz jawne oznaczenie początkowej kopii historycznej.

Logi: docs/evidence/4b4-step-2. Użyto istniejących node_modules i cache Cargo, bez nowego npm ci. Rust 1.99.1; bez desktop feature. Istniejące dead_code/unused import warnings pozostają widoczne. Eksploracyjny test początkowo próbował niewdrożonego profile.update; finalny fixture używa rzeczywistych route.save/assign i memory.settings.set, a kontrolę przeliczono na aktualnym pliku. Finalne logi dotyczą pełnych aktualnych źródeł.

Nie wykonano Windows/Tauri/WebView, rzeczywistego systemowego vault lub live API. Loopback Custom OpenAI i testowy vault nie są odbiorem rzeczywistych dostawców. Testy DOM/CSS nie zastępują realnego WebView. Wcześniejsze bramki pozostają otwarte.

## Nałożenie i dalej

Rozpakować CHANGED_FILES.zip w głównym katalogu AI_Home, zastąpić pliki, commit/push. Niczego nie usuwać; bez cleanup, nowych zależności, migracji, workflow, release i instalatorów. Do audytu SOURCE, raport, plan 1.29, SHA256 i OVERLAY_VERIFICATION.

Po CI/audycie: 4B-4 krok 3 — pozostałe rodzaje pamięci i podstawowe wyszukiwanie/retrieval według planu. Następnie pozostałe przeznaczenia importu. **Całe 4B-4 nadal otwarte.**
