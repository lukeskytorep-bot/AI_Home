> Historyczne przekazanie 4B-11 przed audytem. Bieżąca korekta F1/F2 i wyniki: `AI_HOME_4B11_AUDIT_FIX_1_PL.md`, plan treść 1.72.

# AI Home — 4B-11 Reflection/Dream — przekazanie do CI i audytu

Data: 26.10.2026. Status: IMPLEMENTED / AWAITING CI AND AUDIT.
Dokładna baza: `AI_HOME_4B10_SCHEMA14_FIX_SOURCE.zip` (594 pliki), SHA-256 `24085cdf56b3b66952f9d3fdd252e93dbb7f222362f0efbcbac2d294c333c840`. Edward potwierdził zielony wynik tej poprawki. Źródłem zakresu jest kanon Experimental i najnowszy plan treści 1.70; aktualizacja planu ma treść 1.71.

## Wynik

Dodano prywatną warstwę Reflection/Dream dla osobnej instancji Experimental. Primary wybiera cykl albo pozostaje przy dotychczasowych decyzjach. Generator otrzymuje ograniczony, jawny pakiet; może zwrócić `conclusion:none`. Wariant draft zawiera krótki opis oraz do ośmiu propozycji own_note/value. To jawny wygenerowany wynik, a nie ukryty tok rozumowania modelu. Dream pozostaje `synthetic_hypothesis`, nigdy dowodem zdarzenia.

Wynik cyklu nie zmienia aktywnej pamięci. W kolejnej okazji Primary widzi ograniczony zestaw propozycji i podejmuje nową decyzję accept/modify/reject. Receipt wskazuje decyzję Primary, źródłową refleksję i indeks propozycji. Tekst po modyfikacji jest mierzony ponownie. Zaakceptowana własna notatka/wartość nadal ma etykietę `self_authored_not_verified_fact`. Zastąpienie i wycofanie zachowują poprzedni tekst, pochodzenie oraz decyzję.

Nie ma automatycznej publikacji, wiadomości, narzędzi, webhooków ani dostępu generatora do ActionExecutor. Sleep, refusal i harmonogram okazji nadal podlegają decyzjom AI. Nie wywołujemy generatora podczas zwykłego ticka lub po restarcie. Standard pozostaje odseparowany.

## Kontrakty, zapis i ograniczenia

- Addytywny suplement `experimental.schema.json`: ReflectionOutput, ReflectionRun, PrivateReflection, PrivateSelfReview, PrivateSelfRecord i cztery nowe warianty DecisionRecord. Cztery frozen kontrakty P2 są identyczne z bazą.
- SQLite v15 dodaje cztery tabele powiązane z profilem/instancją. Zapis wyniku i wskaźnika runu oraz domknięcie cyklu są atomowe. Oddzielna transakcja decyzji Primary obejmuje receipt, notatkę, ewentualne zastąpienie starej wersji i zakończenie okazji.
- Przed generacją kontrolowany jest pełny zserializowany input + rezerwa na output + 512 bajtów. Domyślnie: context 32768 bajtów, output 4096 bajtów, deadline 5000 ms. Granice konfiguracji: context 1024–65536, output 1–8192, deadline 1–30000 ms. To limity natywnej usługi, nie deklaracja dokładnego okna tokenowego dostawcy.
- Jedna rezerwacja generatora na cykl, bez automatycznych powtórzeń. `generationCallCount=1` opisuje zarezerwowaną logiczną próbę; nie jest licznikiem fizycznych POST-ów ani potwierdzeniem działania API.
- Kolejny kontekst Primary zawiera całe wpisy, maksymalnie 4096 bajtów, z omittedCount. Propozycje oczekujące na decyzję mają pierwszeństwo. Skan wejść ograniczono do 100 najnowszych propozycji i 100 aktywnych wpisów; pominięcia są liczone, nie blokują dalszej pracy. Prywatne podsumowanie refleksji nie trafia automatycznie do tego kontekstu. Pominiętej propozycji/wersji nie można zaakceptować ani zmienić na podstawie tego pakietu.
- Każdy zapis sprawdza zakres instancji, generację i rewizje. Po zmianie prywatnego kontekstu lub użytych snapshotów podczas generacji spóźniony wynik jest odrzucany. Hashe chronią odczyt treści przed niezgodnością z zapisanymi metadanymi; nie zastępują kryptograficznej ochrony przed administratorem bazy.
- Własne notatki: limit profilu longTerm; wartości: workRule. Mierzymy faktyczne bajty UTF-8 jako konserwatywny szacunek, bez udawania dokładnego tokenizera lub ModelRoute. Limit pojedynczego tekstu 8192 bajty pozostaje także przy ustawieniu unlimited. Nie zmieniono istniejącego, osobnego runtime pamięci Standard ani jego retry. Prywatne self records nie są zapisami MemoryVersion.
- Restart oznacza zarezerwowany run jako interrupted / OPERATION_INTERRUPTED, nie powtarza generacji. Awaria nie tworzy fikcyjnej odmowy AI. Usługa zachowuje dotychczasowy technical backoff.

## Prywatność i cykl życia

Pełna kopia SQLite zawiera również prywatne notatki i wartości. Backup/restore zostały sprawdzone na rzeczywistych nowych rekordach. Zwykły portable export oraz diagnostics nie zawierają ich treści. Permanent profile deletion usuwa nowe rekordy w kolejności zgodnej z FK, tylko dla wybranego profilu. Source refs przy usunięciu źródła stają się permanently_deleted; nie odtwarzamy z nich usuniętej treści. Własna, jawnie wygenerowana notatka pozostaje odrębnym rekordem aż do jej usunięcia/wycofania lub skasowania profilu; nie obiecujemy kasowania wszystkich semantycznych pochodnych źródła.

## Walidacja

| Kontrola | Wynik |
|---|---|
| Rust Linux, `cargo test --no-default-features --all-targets --locked --offline` | **409/409**, 22 zestawy |
| Nowe testy Reflection, ponowione po przyspieszeniu fixture testu >100 propozycji | **22/22** |
| Frontend Vitest | **219/219**, 21 plików |
| TypeScript i Vite build | **PASS** |
| Granice modułów + schema expectation sweep | **34/34**, 13 asercji migracji do v15 |
| Frozen P2 reference checks | **193/193** |
| Contract/handler/router parity | **54/54** |
| Experimental foundation / runtime / continuity / internal-state | **12/12 / 24/24 / 23/23 / 23/23** |
| Reflection static scope guard | **12/12** (osobno od testów natywnych) |
| Standard closure / release guards, Rust source-text sweep | **8/8 / 8/8 / PASS** |

Pełny przebieg zakończył się bez błędów. Po nim przyspieszono wyłącznie przygotowanie fixture regresji >100 propozycji: jeden test generatora + poprawne rekordy SQLite zamiast 105 powtarzanych generacji. Finalny plik nowych testów uruchomiono ponownie (22/22); kod runtime pozostał ten sam. Wcześniejszy pełny przebieg także przeszedł (408 testów, przed dodaniem tej jednej regresji). Logi wskazują rzeczywiste polecenia i wyniki; nie są dowodem Windows/Tauri/OS vault/live API. Pozostało wcześniejsze ostrzeżenie unused import w teście internal-state.


Nowe testy obejmują: brak wniosku; spekulację dream; osobną decyzję Primary; modify/reject; pomiar UTF-8; duplikaty decyzji; zmianę/wycofanie i historię; izolację zakresu; preflight bez generatora; błędny output; timeout; zmianę generacji/kontekstu podczas oczekiwania; rollback; bounded context i ponad 100 propozycji; sleep/refusal; migrację v14→15; restart przerwanego cyklu; backup/restore, prywatność eksportu/diagnostyki i usunięcie profilu; uszkodzenie hashy. Fixture migracji v14 powstaje przez usunięcie czterech nowych tabel z bieżącego schematu i ustawienie PRAGMA 14, z zachowaniem starszych danych; nie nazywamy go niezależnym historycznym dumpem produkcyjnym.

Podczas prac wykryto i usunięto błąd kolejności pakowania kontekstu: aktywne notatki mogły wypchnąć nową propozycję, uniemożliwiając jej zatwierdzenie. Propozycje mają teraz pierwszeństwo, a regresja z ośmioma kolejnymi notatkami potwierdza możliwość kontynuacji. Pierwsze robocze wykonania nie stanowią wyników finalnych.

## Granice odbioru

Brak testu desktopu Windows/Tauri/WebView, rzeczywistego OS vault, live API oraz renderowania nowego interfejsu — interfejsu Experimental jeszcze nie dodano. ReflectionProvider jest wstrzykiwanym, asynchronicznym interfejsem Rust, domyślnie DisabledReflectionProvider. Testy używają jawnych generatorów demonstracyjnych. Produkcyjny adapter LLM, walidacja budżetu tokenowego konkretnej trasy i host schedulera wymagają integracji przed udostępnieniem Experimental użytkownikowi. Nie jest to jeszcze działający autonomiczny tryb desktopowy.

Nie zmieniono zależności, Cargo.lock, package-lock.json, workflow, uprawnień, adapterów providerów, vault ani frozen P2. Zmiany w starych testach dotyczą docelowej wersji storage (15), liczby tabel Experimental (17), bieżącej wersji w mocku backup UI oraz rozszerzenia regresji source deletion. Historyczne wersje fixture pozostają zachowane. Nie wykonano publikacji GitHub ani release; nowy wynik CI i audyt są nadal wymagane.

## Zastosowanie

Nakładka CHANGED_FILES zawiera tylko zmienione i nowe pliki, bez usunięć. Rozpakować w głównym katalogu repozytorium, nadpisując pliki. Nie rozpakowywać wewnątrz src-tauri. Uruchomić `py -3 scripts/verify-source.py .` (Windows) albo `python3 scripts/verify-source.py .`. Potem commit/push i dotychczasowy CI. Pełny SOURCE jest alternatywą do audytu i odtwarzania, nie instalatorem. OVERLAY_PROOF sprawdza zewnętrzne ZIP-y bajtowo; manifest i SHA256SUMS obejmują komplet źródeł.
