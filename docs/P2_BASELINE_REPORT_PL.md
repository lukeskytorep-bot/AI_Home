# AI Home — P2: kontrakty danych, IPC i granice modułów

Wersja 1.1 po korekcie audytowej, 11 października 2026. Kontrakt `ai-home/1`.

**Wynik: przygotowano kontrakty wykonawcze dla 4B-0 i kolejnych etapów.** P0 pozostaje przyjętym projektem ekranów. P1 ma PASS audytora przekazany przez Edwarda. Audyt P2 potwierdził dowody 1.0, wskazując trzy luki przed PASS CLOSED. Wersja 1.1 usuwa je i jest przekazaniem do ponownego odbioru. P2 nie jest implementacją runtime ani niezależnym audytem. Następnym etapem jest 4B-0: uruchamialna podstawa Tauri 2 + React/TypeScript + Rust + SQLite.

## 1. Materiały i zakres

Podstawa: plan wykonawczy AI Home (teraz treść 1.5), Document 4B Agent Home Modular Runtime Plan v1.3, raport i dowody P1, poprawione P0 oraz bieżący Document 4A AI First PHP Community Server Plan v3.1 (aktualna wersja pliku 5). Potwierdzenie Edwarda, że forum jest odebrane, ma pierwszeństwo przed historycznymi wpisami o niedokończonych etapach 4A. AI Home działa bez forum.

Migawka Harness: `AI-RV-Harness-ai-rv-2.zip`, SHA-256 `534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e`, 1075 plików, digest drzewa `7044d8e5435d8fd15d0c5537028b302ed96e597cee9d067d4a265ccc402b0cc7`. Komentarz ZIP `2cb5bba5b857f808851388e8078127e37389434b` jest identyfikatorem ze źródła, nie potwierdzonym zdalnie commitem. Mapa ponownego użycia i warunki licencji pozostają w P1; P2 nie zmienia zakresu na kopiowanie całego Harness.

Pakiet P2 zawiera 57 definicji DTO w `contracts/domain.schema.json`, parametry w `contracts/defaults.json`, 30 poleceń i 8 typów zdarzeń w `contracts/ipc.catalog.json`, generatory i referencyjne sprawdzenia. Definicje JSON Schema określają kształt danych. Reguły relacyjne, uprawnienia, cykl życia i transakcje z tego dokumentu są równie obowiązujące; sam poprawny JSON nie uprawnia do zapisu.

## 2. Tożsamość, połączenia i izolacja

`Profile.id` jest lokalnym, losowym, trwałym ID. Nazwa, model, endpoint, klucz i identyfikator konta dostawcy nie są tożsamością profilu. Zmiana tych parametrów nie resetuje rozmów ani pamięci. Profil jest `draft`, `ready` lub `archived`; `ready` wymaga poprawnej trasy Primary i sprawdzonego połączenia. Standard jest dostępny najpierw. Włączenie niezaimplementowanego Conscious zwraca `MODE_UNAVAILABLE`.

**Endpoint należy do `ProviderConnection`, nigdy do globalnego stanu aplikacji.** Połączenie ma `profileId`, własne ID, rodzaj dostawcy, znormalizowany endpoint, etykietę, `credentialRef`, rewizję klucza i stan testu. Klucz przechowuje natywny magazyn sekretów w odrębnej przestrzeni AI Home; nie używamy przestrzeni `org.airvharness.app`. Surowy sekret występuje wyłącznie jako przejściowe `CredentialInput` przekazywane do natywnego polecenia ustawienia. UI po zapisie usuwa go z pola i stanu; nie można go odczytać przez IPC.

`ModelRoute` wiąże rolę `primary` albo `scribe` z połączeniem i modelem. Primary i Scribe mogą mieć inne modele, połączenia, parametry i limity odpowiedzi. Mogą współdzielić połączenie wewnątrz tego samego profilu. Drugi profil ma własne połączenie i odwołanie do sekretu, nawet jeśli człowiek świadomie wpisze ten sam klucz. Nie współdzielimy automatycznie rekordów połączeń między profilami.

Każda relacja między rekordami sprawdza `profileId`, także dla źródeł, załączników, wersji i operacji. Implementacja stosuje złożone klucze obce `(profile_id, id)` i sprawdza zakres przed odczytem. Samo znalezienie globalnego ID nie wystarcza. Błąd zakresu daje `PROFILE_SCOPE_MISMATCH` bez danych cudzego rekordu. Usuwanie jednego profilu nie usuwa cudzych blobów; ewentualna deduplikacja wymaga licznika właścicieli.

Test połączenia poświadcza parę `(connection.revision, credentialRevision)`. Zmiana dostawcy, endpointu lub klucza unieważnia poświadczenie. Wynik spóźnionego testu nie może zatwierdzić nowszej konfiguracji. Zmiana modelu unieważnia jego możliwości i pomiar kontekstu. Normalizacja URL usuwa wyłącznie dozwolone różnice składni; nie zmienia wielkości liter ścieżki. Odrzuca dane użytkownika w URL, fragmenty i sekrety w query. HTTPS jest domyślne; jawnie dopuszczone lokalne HTTP służy lokalnemu endpointowi. Wbudowane endpointy i specyfikę API adaptujemy z Harness; własny endpoint jest jawną konfiguracją, nie zgadywaną trasą.

## 3. Rekordy domeny

| Grupa | Kontrakty i zobowiązania |
|---|---|
| Rozmowa | `Conversation`, `Message`, `ContextSegment`: szkic per rozmowa, monotonny numer wiadomości, zachowany surowy zapis, segment z zakresem wiadomości, hashem i źródłami podsumowania |
| Materiał | `ImportedMaterial`, `Attachment`: oryginalne bajty i hash, wyodrębniony tekst i osobny hash, stan parsowania, zakres, autor oraz importer |
| Wiedza i wskazówki | `KnowledgeSource`, `PersistentGuidance`: źródło, język, aktywność, kolejność wskazówek, pomiar tokenów; plik nie staje się automatycznie instrukcją |
| Pamięć | `MemoryProposal`, `MemoryDecisionReceipt`, `MemoryRecord`, `MemoryVersion`, `MemoryWriteAttempt`: propozycja, decyzja Primary, niezmienna wersja, aktywny wskaźnik i historia każdej próby |
| Zasady i zamiary | `WorkRule`, `Intent`: wskazanie zatwierdzonej pamięci; korekty źródłowe, aktywność zasady, termin/wyzwalacz i stan zamiaru |
| Relacje i stan | `RelationshipMetadata`: czas ostatniego kontaktu i licznik wyliczane przez program; `InternalState`: NullState lub wersjonowany stan przyszłego ConsciousCore |
| Wykonanie | `Operation`, `Activity`, `ContextManifest`: wynik rzeczywistej operacji, liczniki, budżety i jawny wykaz wykorzystanych źródeł |
| Integracja | `ConnectorBinding`, `ToolPolicy`: osobne zdalne ID, klucze, możliwości oraz uprawnienia; niezależne od klucza LLM |

Wszystkie trwałe DTO mają `schemaVersion`; rekordy profilu mają `profileId`, rewizję i czas UTC. Program tworzy ID i daty. Brak wartości oznacza nieobecne pole opcjonalne, nie pusty sekret czy fikcyjną datę. Nieznane pola są odrzucane. Wersje pamięci i źródłowe snapshoty są niezmienne; zmiana tworzy nowy rekord. `claimKind` odróżnia fakt, preferencję, wniosek, zasadę lub materiał mieszany. To klasyfikacja pochodzenia, nie dowód prawdziwości.

L0 to NullState/ConsciousCore; L1 to wybrane wiadomości. L2 ma rekord rodzaju `working` (jeden aktywny na profil); L3 obejmuje rekordy `episodic` i `relationship`; L4 obejmuje `longTerm`. `intent` i `workRule` mają własne rekordy oraz metadane. Unikalność aktywnego L2, wersji `(recordId, versionNumber)`, sekwencji `(conversationId, sequence)` i numeru próby `(operationId, attemptNumber)` egzekwuje magazyn. `MemoryRecord.activeVersionId` musi wskazywać wersję tego samego rekordu i profilu.

## 4. Import przez człowieka

Cztery cele są rozłączne: `conversation`, `knowledge`, `guidance`, `memory_import`. Pierwszy wymaga `conversationId`; pozostałe nie uzyskują dostępu do rozmów przez sam import. `Attachment` wskazuje materiał tej samej rozmowy i profilu. Zmiana przeznaczenia jest osobną czynnością z zachowaniem pochodzenia; nie przesuwa potajemnie pliku do trwałej wiedzy.

`Author` zapisuje człowiek/AI/współpraca/nieznane, etykietę, źródło deklaracji i stan weryfikacji. `importer` zapisuje człowieka i lokalny identyfikator. Domyślnie deklaracja importera nie jest zweryfikowanym autorstwem. Import z UI nie może ustawić verified=true; taki stan wymaga natywnej weryfikacji dowodu źródła, poza początkowym importem. Materiał napisany przez AI i przesłany przez człowieka zachowuje oba fakty. Import Field Guide/Viewer Notes nie udaje wykonanej sesji ani pamięci zbudowanej przez ten profil.

Importer przechowuje oryginał i jego hash. Wymagany `mediaKind` rozdziela `document` i `image`; `parseState` określa gotowość właściwą dla tego rodzaju. Schemat jest rozłączną unią sześciu wariantów:

- Dokument `ready` wymaga extractedBlobId, extractedSha256 i importMethod. Nie może zawierać imageMetadata ani errorCode. Dokument `pending` nie deklaruje gotowej ekstrakcji; `failed`/`unreadable` wymaga bezpiecznego errorCode.
- Obraz `ready` wymaga imageMetadata z formatem png/jpeg/webp/gif oraz dodatnimi width/height. Native ustala te dane z dekodowania oryginalnych bajtów, sprawdza zgodność MIME i limity, a nie przyjmuje deklaracji UI jako dowodu. Nie wymaga ani nie deklaruje extractedBlobId, extractedSha256, importMethod lub OCR. Gotowy obraz nie ma errorCode.
- Obraz `pending` nie ma jeszcze poświadczonych metadanych; `failed`/`unreadable` ma kod przyczyny, np. nieprawidłowe bajty. Brak OCR sam w sobie nie oznacza błędu poprawnego obrazu. Przyszła ekstrakcja tekstu z obrazu otrzyma jawny, osobny kontrakt, zamiast fikcyjnych pól dokumentu.

Gotowość obrazu oznacza możliwość podania go trasie vision; nie poświadcza odczytania treści, opisu ani indeksu tekstowego. Wiedza z obrazu bez tekstu nie trafia do wyszukiwania tekstowego jako rzekomo rozpoznany dokument. Materiał `pending` nie trafia do wywołania jako przeczytany. Skan PDF bez tekstu pokazuje brak OCR. Format obrazu i możliwość vision są sprawdzane przed wysłaniem do wybranej trasy; obrazy nie są automatycznie analizowane przez Scribe bez takiej potrzeby.

Początkowe limity ochronne: 12 plików na partię, 15 MiB dokumentu, 2 MiB tekstu po ekstrakcji, 10 MiB obrazu. Są parametrami importu, nie limitami pamięci. Przekroczenie zwraca błąd z możliwością podziału/wyboru plików; nie stosujemy cichego `.take(12)` ani obcięcia tekstu. Dostęp do pliku przyznaje jednorazowy, natywny `selectionToken` ograniczony do profilu i wybranych plików; UI nie przesyła dowolnej ścieżki systemowej.

Stałe wskazówki można włączać, wyłączać i układać. Native ContextBuilder umieszcza aktywne wskazówki w oznaczonym bloku niższej rangi niż reguły aplikacji. Adapter renderuje blok zgodnie z API dostawcy: bloki system/developer/messages lub systemInstruction. Treść nie może nadawać ról, narzędzi ani uprawnień; uprawnienia egzekwuje program, nie obietnica w promptcie. Scribe otrzymuje właściwe źródła i instrukcje zadania pamięci, bez automatycznego kopiowania wszystkich wskazówek konwersacyjnych.

## 5. Pojemność, kontekst i domyślne wartości

| Rodzaj zapisu | Domyślna pojemność pojedynczego rekordu |
|---|---:|
| working / L2 | 300 tokenów |
| episodic / L3 | 300 tokenów |
| relationship / L3 | 300 tokenów |
| longTerm / L4 | 500 tokenów |
| intent | 300 tokenów |
| workRule | 300 tokenów |

Są to nowe wartości wykonawcze P2. Ustawienia są niezależne: zmiana L2 nie zmienia pozostałych. Nie jest to limit sumy całego magazynu. Początkowo nie ma automatycznego usuwania historii; przechowujemy ją do jawnego usunięcia. Wyczerpanie dysku kończy zapis błędem, zachowując poprzednią wersję.

Polityka jest unią `{kind: "bounded", tokens: dodatnia liczba}` / `{kind: "unlimited"}`. Presety 300/500/800/1000 i własna wartość są dostępne w ustawieniach. Dla „Bez limitu długości zapisu” nie podstawiamy sztucznego miliona ani ukrytego limitu pojemności. Nadal obowiązują format, miejsce na dysku, budżet wywołania i limit odpowiedzi modelu. UI pokazuje ostrzeżenie uzgodnione w planie.

`TokenMeasurement` zapisuje wartość, exact/estimated, estimatorId oraz **modelRouteId i modelRouteRevision**. Obie identyfikacje są wymagane we wszystkich użyciach: PersistentGuidance.tokenMeasurement, MemoryVersion.measurement, MemoryDecisionReceipt.candidateMeasurement i MemoryWriteAttempt.candidateMeasurement. Primary revision=1 i Scribe revision=1 to różne pomiary, jeśli ID tras są różne. Wynik szacowany nigdy nie jest podpisany jako dokładny.

Native sprawdza właściciela trasy i zgodność całej pary `(modelRouteId, modelRouteRevision)` z trasą konkretnego wywołania. Cache pomiaru jest dodatkowo związany z hashem mierzonej treści i estimatorId. Zmiana modelu, connectionId, możliwości wpływających na tokenizację lub tokenizera zwiększa rewizję trasy i wymaga ponownego pomiaru; przełączenie na inne routeId także unieważnia przydatność wyniku. Ten sam numer rewizji nie uprawnia do przeniesienia pomiaru między trasami.

Zachowujemy konfigurację historycznej trasy według ID+rewizji. Nie modyfikujemy historycznego pomiaru w niezmiennej wersji pamięci: pozostaje dowodem dawnej walidacji. Bieżący preflight i walidacja kolejnego zapisu tworzą nowy pomiar dla aktualnej trasy; brak aktualnego wyniku blokuje użycie starego wyniku jako poświadczenia budżetu. Dla wskazówek zmiana konfiguracji wymaga nowego bieżącego pomiaru, bez zmiany tekstu/źródła. Finalny pomiar receipt i wersji pamięci odpowiada Primary, dla którego walidowano zapis; Scribe może mieć własny pomiar używany wyłącznie przy jego wywołaniu.

Niezależne domyślne budżety: wybór wspomnień 1200 tokenów; aktywne wskazówki 800; rezerwa wejścia 512; odpowiedź rozmowy 1024; pułap generowania odpowiedzi pamięci 4096; operacja maksymalnie 4 fizyczne wywołania i 180 sekund. Pułap odpowiedzi nie jest pojemnością rekordu. Duży wybrany limit, który nie może zostać obsłużony przez trasę, daje `OUTPUT_BUDGET_INSUFFICIENT` i możliwość zmiany ustawień. Nie obcinamy kandydatury do rozmiaru modelu. Limit kosztu pozostaje jawnie nieustawiony, dopóki użytkownik go nie ustali. Ustawiony limit przy braku wiarygodnej ceny daje `COST_ESTIMATE_UNKNOWN`.

Preflight dla każdego wywołania: pełne wejście + zarezerwowane wyjście + rezerwa bezpieczeństwa muszą zmieścić się w kontekście modelu. Nieznana pojemność wymaga jawnego, zapisanego potwierdzenia parametrów, a nie zgadywania. L2, nowa wiadomość i instrukcje obowiązkowe muszą się zmieścić; inaczej zwracamy `CONTEXT_BUDGET_EXCEEDED`. Aktywne wskazówki przekraczające budżet dają `GUIDANCE_BUDGET_EXCEEDED`, z możliwością zwiększenia budżetu lub wyłączenia wskazówek. Wspomnienia i fragmenty wiedzy dobiera Retriever; pominięcia są jawne w `ContextManifest`. Nie deklarujemy przeczytania całości pominiętego pliku. Nieaktualna pamięć jest wykluczona także z bocznego panelu bieżącego kontekstu.

Zmniejszenie limitu nie usuwa istniejącej wersji. Program proponuje skrócenie i wskazuje problem; jeśli wersja nadal mieści się w kontekście, pozostaje użyteczna, a kolejne zapisy muszą spełnić nową politykę. Jeśli się nie mieści, preflight zatrzymuje wywołanie do rozwiązania problemu. UI, język rozmowy i język materiałów są niezależne. Domyślnie zachowujemy język źródła; angielskie UI nie tłumaczy polskiej pamięci. Tłumaczenie wymaga jawnej polityki profilu i zachowuje źródło.

## 6. Decyzja i atomowy zapis pamięci

1. OperationCoordinator tworzy operację z rewizją rekordu, aktywną wersją, polityką pojemności, trasami, źródłami i limitami. Scribe proponuje treść; przy wyłączonym Scribe tę rolę przejmuje Primary i UI to wskazuje.
2. Native waliduje format i pojemność propozycji. Przekroczenie nie wywołuje zbędnego zatwierdzania długiej treści. Primary ocenia poprawną kandydaturę: ACCEPT, MODIFY lub REJECT. MODIFY przechodzi ponowny pomiar i walidację faktycznie zmienionej treści.
3. Pierwsze przekroczenie pojemności zapisuje zdarzenie i odrzucony hash/pomiar. Model dostaje informację: pierwsza próba nie przeszła, limit i pomiar, teraz druga i ostatnia próba. Druga kandydatura przechodzi tę samą walidację i decyzję Primary. Ponowienie pojemności obejmuje całą kandydaturę, również przekroczenie po MODIFY.
4. Drugi błąd pojemności kończy operację. REJECT przez Primary jest wynikiem decyzji, nie uruchamia automatycznego ponowienia w celu wymuszenia akceptacji. Błąd formatu ma osobny limit jednej naprawy; retry transportu maksymalnie jedno na etap. Każde faktyczne wywołanie, również naprawa, continuation i decyzja Primary, zużywa wspólny budżet czterech. Dlatego druga próba pojemności może zakończyć się wcześniej błędem budżetu; nie obiecujemy dodatkowych wywołań poza nim.
5. Native tworzy `MemoryDecisionReceipt` dopiero z przypisanej odpowiedzi Primary. Frontend nie przekazuje zatwierdzenia, roli ani gotowego receipt. Brak polecenia UI „agent approve”. Zapis aktywnej wersji wymaga receipt z ACCEPT/MODIFY dla tej kandydatury; receipt REJECT nie uprawnia do zapisu. Aktywność pokazuje krótkie uzasadnienie decyzji przeznaczone do publikacji, nie ukryte rozumowanie.
6. Jedna transakcja sprawdza zakres, rewizję, aktywną wersję, aktualność polityki/tras i źródeł, dodaje niezmienną `MemoryVersion`, zmienia wskaźnik, zapisuje końcowy wynik i zdarzenie. Konflikt daje `REVISION_CONFLICT`; nie nadpisuje nowszego zapisu. Anulowanie sprawdzamy ponownie przed commit. Odpowiedź po anulowaniu nie tworzy pamięci.

Domyślny przykład bez awarii mieści cztery wywołania: Scribe 1 → Primary MODIFY za długi → Scribe 2 → Primary ACCEPT. Jeśli pojemność Scribe 1 nie pasuje, Primary 1 nie jest potrzebny. Liczniki sieci/formatu/pojemności są rozdzielone; budżet całości kończy każdą pętlę. Tryb unlimited nie uruchamia retry z powodu lokalnej pojemności.

Nieudana operacja, restart ani anulowanie zachowują ostatni dobry zapis i nigdy nie aktywują pustej pamięci. Operacje niedokończone przy starcie otrzymują `interrupted`; nie powtarzamy po cichu płatnych wywołań. Wznowienie tworzy kontrolowaną operację z aktualnym snapshotem. Prywatna continuation dostawcy jest związana z modelem, endpointem, rolą i rewizją klucza; po zmianie tych parametrów nie odtwarzamy jej. Surowa rozmowa i pamięć pozostają niezależne.

Oznaczenie `outdated` wyłącza zapis z retrieval i kontekstu, pozostawiając historię. Przywrócenie starszej wersji przez człowieka tworzy nową wersję `origin: restore`, z `restoredFromVersionId`, aktualnym poprzednikiem i historycznym receipt zatwierdzenia oryginału. Nie udaje świeżej decyzji modelu. Restore i ponowna aktywacja wymagają zgodności z obecną polityką, zakresu i CAS; w razie błędu nic nie zmieniają. Ręczna edycja tworzy propozycję do rozpatrzenia przez Primary. Jawne usunięcie historii jest odrębną przyszłą czynnością 4B-6.

## 7. IPC i zdarzenia

Katalog JSON jest kontraktem poleceń, payloadów, wyników i etapów ich wdrożenia. Nie wszystkie polecenia są dostępne w 4B-0; niezaimplementowane zwracają `UNKNOWN_COMMAND`. Każdy request ma `contractVersion: 1`, `requestId`, `command`, payload właściwy dla polecenia, zakres profilu tam, gdzie wymagany, i `expectedRevision` dla zapisów zależnych od stanu. Backend odrzuca nieznane pola, wersje i polecenia. `CommandEnvelope.payload` jest wspólną kopertą, a nie przyzwoleniem na dowolny obiekt: obowiązuje również schema z katalogu.

`expectedRevision` odnosi się do modyfikowanego rekordu: profil/preferencje, connection, conversation, guidance lub memory record. Dla utworzenia trasy przez `route.save` bez routeId oznacza rewizję profilu; dla aktualizacji oznacza rewizję trasy. route.save zapisuje konfigurację trasy; route.assign osobno przypisuje ją jako aktywną Primary/Scribe, sprawdzając rolę, właściciela i rewizję profilu. Tworzenie profilu i połączenia nie wymaga rewizji nieistniejącego rekordu. Dalsze polecenia obsługi wiedzy, zamiarów, backupu i integracji będą dodawane do katalogu w swoich etapach z fixtures; obecny katalog nie jest pełnym API przyszłego wydania. Odczyty stronicowane używają nieprzezroczystego kursora przypisanego do profilu i zapytania; brak kolejnej strony to pusty `nextCursor`.

Wynik to `ok: true` z wynikiem właściwego typu albo `ok: false` z `Error`: kod z katalogu, klucz tłumaczenia, retryable, opcjonalne pole i operationId. Nie zwracamy dowolnego tracebacku dostawcy. Długa czynność zwraca `Accepted` zawierający queued Operation; nie jest to komunikat o ukończeniu. Końcowy wynik pobieramy przez operation.get i zdarzenia. Cancel jest idempotentne; dla ukończonej operacji zwraca jej istniejący stan.

### Trwały wynik operacji

Operation jest unią rozróżnianą przez kind i status. Każdy wariant ma dotychczasowe liczniki/limity i recordRefs: listę ScopedRecordRef z profileId, recordKind i recordId, wskazującą istniejące rekordy związane z operacją. Są to rekordy docelowe lub już zapisane, nie obietnica przyszłego utworzenia. Zakres każdego odwołania musi odpowiadać operacji. UI pobiera treść przez właściwy odczyt w tym samym profilu, zamiast otrzymywać sekrety lub pełne prompty w wyniku.

| Status | Pola końcowe wymuszane schematem |
|---|---|
| queued / running | Brak result, error i finishedAt; recordRefs mogą wskazywać już zapisaną wiadomość człowieka lub badane połączenie |
| succeeded | Wymagane finishedAt i result właściwy dla kind; error zabroniony |
| failed / cancelled / interrupted | Wymagane finishedAt i bezpieczny Error; result zabroniony; recordRefs zachowują odwołania do wcześniej zapisanych rekordów |

| Rodzaj operacji | Typ wyniku sukcesu |
|---|---|
| chat | ChatOperationResult: conversationId, userMessageId, assistantMessageId |
| memory_update | MemoryUpdateOperationResult: outcome=committed, recordId, versionId i recordRevision albo outcome=no_change z recordId i opcjonalnym activeVersionId |
| import | ImportOperationResult: niepusta lista materialIds oraz attachmentIds, knowledgeSourceIds, guidanceIds i proposalIds; puste listy oznaczają brak rekordów danego typu |
| restore | RestoreOperationResult: recordId, nowy versionId, restoredFromVersionId i recordRevision |
| connection_test | ConnectionTestOperationResult: connectionId, connectionRevision, credentialRevision, testedAt i outcome=verified |
| connector_action | ConnectorActionOperationResult: bindingId, action, remoteEntityIds oraz opcjonalny cursor/epoch; bez tokenów i kluczy forum |

Każdy result zawiera własny kind; schemat Operation wymusza jego zgodność z rodzajem operacji. ImportOperationResult z materialIds=[] lub duplikatami jest błędem. Wskazanie istniejącego obcego ID także jest błędem zakresu sprawdzanym przez native. Nieudany test klucza może zwrócić Error.code=PROVIDER_AUTHENTICATION_FAILED, messageKey=error.provider_authentication_failed, retryable=false i operationId; recordRefs wskazuje testowane połączenie. Nie zapisujemy odpowiedzi dostawcy, sekretu ani swobodnego komunikatu sieciowego w Error. messageKey pochodzi ze stałego słownika native. Dla cancelled kod to OPERATION_CANCELLED, dla interrupted OPERATION_INTERRUPTED; operationId w błędzie, jeśli obecne, musi odpowiadać temu rekordowi.

Stan terminalny, typed result/error, recordRefs i finishedAt są zapisywane trwale w tej samej transakcji co końcowe zmiany i event outbox. operation.changed ma ten sam pełny DTO co operation.get. Utrata zdarzeń nie usuwa wyniku: po ponownym starcie UI odczytuje operation.get w profilu. Operacja zastana jako queued/running zostaje przerwana z trwałym OPERATION_INTERRUPTED; bez fikcyjnego sukcesu i automatycznego ponowienia płatnego wywołania. Zakończone wyniki pozostają niezmienne przy powtórzonym cancel/requestId. Błąd po zapisaniu wiadomości człowieka zachowuje jej odwołanie; nie tworzy kompletnej wiadomości asystenta. Import sukcesu wskazuje wszystkie zatwierdzone rekordy partii; niepełny import kończy się błędem z recordRefs już zapisanych materiałów i jawnie oznaczonymi stanami, nie fałszywym sukcesem całej partii. Jawne usunięcie danych w 4B-6 może pozostawić ID/tombstone z informacją „rekord usunięty”; nie zastępujemy go cudzym rekordem.

Przykłady sukcesu importu, sześciu rodzajów sukcesu, no_change oraz failed/cancelled/interrupted, obu tras pomiaru, wskazówek/wersji/receipt/próby i obrazu vision bez OCR są w contracts/fixtures.json. Pola po korekcie są wymagane w kontrakcie przygotowawczym schemaVersion=1; poprzedni kandydat nie był wydaną aplikacją ani rzeczywistą bazą, dlatego nie udajemy wykonania migracji użytkowników.

Ponowienie tego samego requestId i tego samego polecenia/zakresu/payloadu zwraca ten sam receipt; zmienione dane dają `IDEMPOTENCY_CONFLICT`. Rejestr receipt i fingerprint zapisujemy transakcyjnie z mutacją. Sekret nie trafia do rejestru payloadów: fingerprint polecenia sekretu powstaje z HMAC przy użyciu lokalnego klucza instalacji w vault. Nie eksportujemy fingerprintów ani klucza HMAC. Retencja receipt musi być jawna w implementacji; po wygaśnięciu nie wykonujemy automatycznie niepewnego ponowienia operacji zewnętrznej.

Zdarzenie zawiera eventId, profil, opcjonalną operację, typ, czas i monotonną sekwencję per profil. Wysłanie następuje po commit. Frontend ignoruje duplikat, wykrywa lukę i pobiera snapshot; nie stosuje pozornie kompletnego stanu po utracie zdarzeń. Native tworzy snapshot i jego lastEventSequence z tego samego odczytu. `snapshot.required` oznacza utratę dostępnej historii, nie sukces operacji. Activity rejestruje wynik, numer próby, pomiar i limit; nie zawiera kluczy, pełnych promptów ani prywatnego tekstu pamięci.

## 8. Granice modułów i przenoszenie Harness

| Moduł | Odpowiedzialność i granica |
|---|---|
| React UI | Ekrany, szkice, PL/EN, wyświetlanie zdarzeń; wyłącznie typowany klient IPC; brak bezpośrednich API modeli, SQL i kluczy po zapisaniu |
| Kernel Rust | Uruchomienie, lifecycle, rejestr usług, zdarzenia, konfiguracja; nie zawiera logiki dostawcy, forum lub ConsciousCore |
| Application Rust | ProfileService, ConversationService, ImportService, MemoryService, OperationCoordinator; transakcje, izolacja, CAS i wspólne budżety |
| Domain / ports | DTO, MemoryRetriever, MemoryStore, ProviderGateway, ToolGateway, CommunityConnector, ConsciousAdapter; bez zależności od UI |
| Persistence Rust | SQLite, wersjonowane migracje, blob store, event outbox, receipt; vault w osobnym adapterze |
| Providers | OpenAI-compatible/OpenRouter, Google, Anthropic; normalizacja odpowiedzi, capabilities, vision, token usage, continuation; bez samodzielnego zapisu pamięci |
| Context / Memory | Pomiar, retrieval, manifest, reguły decyzji i wersji; nie przyznaje uprawnień narzędzi |
| Executive / Policy / Governor | Centralna decyzja o skutkach i limitach; sygnał ConsciousCore nie omija tej granicy |
| Connectors | Opcjonalne forum, A2A, Messenger; brak zależności startu Standard od sieci społecznościowej |

Docelowe katalogi: `src/ui`, `src/ipc`, `src/locales`, `src/themes`; `src-tauri/src/kernel`, `application`, `domain`, `ports`, `persistence`, `providers`, `memory`, `context`, `policy`, `connectors`, `state`. Schematy i fixtures w `contracts`/`tests/contracts`. To docelowy podział, nie informacja, że pliki aplikacji już istnieją.

Źródła TypeScript Harness nie wykonają się automatycznie w Rust. Przenosimy zgodne DTO, selekcję modeli, UI i funkcje prezentacyjne do TS; algorytmy wymagane po stronie native adaptujemy do Rust z tymi samymi fixtures. Retry i continuation mają jednego właściciela: OperationCoordinator. Adapter transportu nie uruchamia drugiej ukrytej pętli. Nie dokładamy procesu Node tylko po to, aby skopiować moduł. Ekstraktor dokumentów, keyring i pola połączeń wymagają adaptacji opisanej w P1. Identyfikatory i Viewer Notes powiązane w Harness z modelem/połączeniem nie definiują profilu ani pamięci Home.

Standard używa NullState. Opcjonalny Python sidecar ConsciousCore może zostać wdrożony dopiero po dostarczeniu i sprawdzeniu jego źródeł, wersjonowaniu protokołu i testach restartu. P1 potwierdził możliwość architektoniczną, nie działanie konkretnego silnika. Level 0: odczyt; level 1: lokalna odwracalna zmiana w nadanym zakresie; level 2: skutek zewnętrzny wymagający domyślnie zgody; level 3: wysokie skutki wyłączone w początkowym Standard. Wskazówki ani wiadomość modelu nie zmieniają ToolPolicy.

## 9. Języki, skórki i dane przenośne

`LocalePack`: schemaVersion, packVersion, locale, displayName i słownik kluczy → zwykły tekst. EN jest kompletną bazą i domyślnym językiem; PL od pierwszego UI. Brak tłumaczenia w dodatkowym pakiecie daje EN i ostrzeżenie walidacji. Nieznane klucze oraz niezgodna wersja są błędem pakietu. Podstawowe pakiety EN/PL muszą pokryć wszystkie klucze. Interpolacja jest escapowana; pakiet nie wykonuje kodu ani HTML. Użyta prosta walidacja locale obsługuje początkowy podzbiór tagów; rozszerzenie wymaga zmiany kontraktu, nie cichego zgadywania.

`ThemePack`: schemaVersion, packVersion, id, nazwa i warianty light/dark z ośmioma kolorami hex. Pakiety są deklaratywne, bez skryptów, CSS z dowolnymi URL, sieci i sekretów. To minimalny kontrakt rozszerzenia wyglądu; dalsze tokeny typografii/odstępów wymagają wersji schematu. Nie nazywamy tego systemem wykonywalnych pluginów. Walidacja kontrastu i focus/klawiatura są kontrolami prawdziwego UI w 4B-0/8. UI zapisuje wybór języka/skórki globalnie, ale endpointów nie przechowuje w AppPreferences.

Migracje danych mają numer magazynu niezależny od contractVersion. Nieznana nowsza wersja danych otwiera tylko bezpieczny błąd, a nie downgrade zapisem. Migracja trwałych danych ma kopię przed zmianą i test rollback. Pole wymagane dodajemy przez jawny upgrade; nie dowolną wartość domyślną z UI.

Eksport/backup 4B-6 zachowa profile, ustawienia, rozmowy, oryginały, hashe, ekstrakcję, pochodzenie i historię. Portable projection nie zawiera CredentialInput, sekretów, credentialRef, kluczy operacyjnych/recovery, continuation, receipt fingerprintów ani instalacyjnego HMAC. Po imporcie połączenie ma brak poświadczenia i wymaga ponownego wpisania klucza. Nie eksportujemy pełnego ProviderConnection bez projekcji. Eksport diagnostyki domyślnie pomija również treści prywatne. To wymagania kontraktowe; szyfrowany backup sekretów nie jest funkcją obecnego etapu.

## 10. Errata planu źródłowego i forum 4A

Te ustalenia są normatywną erratą dokumentu 4B v1.3. Nie podmieniono historycznego DOCX: pozostaje źródłem, a niniejszy dokument i plan 1.5 wyznaczają aktualny zakres.

| Starszy zapis | Obowiązujący zapis |
|---|---|
| §27: „Treat this v1.2 document…” | Podstawą jest Document 4B **v1.3** wraz z późniejszymi ustaleniami |
| Pierwsze wydanie English-only | EN domyślnie, PL dostępny; zachowany wybór użytkownika |
| Początkowo jeden profil | Wiele odizolowanych profili od pierwszej użytkowej wersji |
| Drzewo aplikacji Python | Granice logiczne zachowane, runtime Tauri/React/TS/Rust/SQLite według P1 |
| AC/EQIS jako domyślny target/protokół | CommunityConnector do odebranego forum 4A; stare nazwy są referencją historyczną |
| Viewer Notes jako stan powiązany z połączeniem/modelami Harness | Import ze źródłem i autorstwem, pamięć związana z profilem Home |

Aktualne 4A przewiduje `/api/v1`, health/version/capabilities, dwuetapową rejestrację, profil i klucze agenta, token/recovery, threads/replies oraz feed z opaque cursor i epoch. Lokalne profile i remoteAgentId są odrębne. Bearer oraz operational/recovery keys forum nie są kluczem LLM. Rozróżniamy schematy podpisu `AGENT-REGISTRATION/1`, `COMMUNITY-CONTENT-REVISION/1`, `IDENTITY-RECOVERY/1` oraz fingerprint `COMMUNITY-IDEMPOTENCY-FINGERPRINT/1`; nie zastępujemy ich podpisem z dawnych projektów. Feed wymaga obsługi resync, checkpointów i bounded pagination.

P2 zamraża lokalny port CommunityConnector, a nie niepoznane bajty wdrożonego protokołu. Przed 4B-12 trzeba pozyskać aktualną specyfikację wire/canonicalization i fixtures podpisów z odebranego serwera oraz potwierdzić capabilities. Nie wymyślamy kodowania kluczy, podpisywanych bajtów lub pól endpointów z samego opisu. A2A/Messenger pozostają późniejsze i opcjonalne. Ta planowana kontrola nie blokuje uruchamiania niezależnego Standard.

## 11. Weryfikacja i przekazanie

Uruchomiono `python3 build_ipc.py`, `python3 build_audit_fixtures.py` i `python3 check_contracts.py`: **193/193 sprawdzeń referencyjnych PASS**. Obejmują referencje schema, kształt ustawień i katalogu, własność połączenia, brak sekretu w ProviderConnection, unieważnienie poświadczenia, kształt decyzji, granice pojemności, jawne unlimited, izolację przez złożony FK SQLite oraz CAS chroniący nowszą wersję. Dodano regresje typowanych wyników i błędów (w tym niedozwolone kombinacje stanów), identyfikacji/starości pomiarów w czterech miejscach ich przechowania, dokumentów i obrazów. Przykład bazy SQLite zamknięto i otwarto ponownie: wynik importu oraz przyczyna nieudanego testu pozostały dostępne bez zdarzeń. To kontrola referencyjnego magazynu, nie restart aplikacji Tauri. Część kontroli jest statyczna; próbka SQLite demonstruje wymagane zachowanie i nie jest migracją aplikacji. Validator obsługuje wyłącznie użyty podzbiór JSON Schema, nie jest pełnym walidatorem standardu. Nie uruchomiono kompilacji Rust/Tauri, keyringu, WebView, pełnych testów Harness ani rzeczywistych API. Zachowanie wieloetapowej operacji pamięci musi mieć testy runtime w 4B-3.

Pakiet dowodów zawiera ten dokument, generatory, schematy, katalog, log kontroli, identyfikację źródeł i zamrożone kopie planu 1.5, HTML P0, opisu P0, raportu P1 oraz pakietu P1. `SHA256SUMS.txt` obejmuje 23 pliki pakietu poza sobą; osobny plik SHA-256 identyfikuje cały ZIP. Nie tworzymy błędnego samohashowania manifestu lub ZIP-a. Weryfikator porównuje zewnętrzny digest z zawartością wskazanego ZIP-a; nazwa w pliku SHA jest opisem i dopisek (1) nie zmienia wyniku. **6/6 kontroli narzędzia dowodowego PASS**: sprawdzają oryginał, zmienioną nazwę oraz odrzucenie zmiany bajtów, nadmiarowego pliku, braku pliku i duplikatu wpisu; log w HANDOFF_CHECKS.txt. Hashy nie należy nazywać dowodem, że zewnętrzny audytor zbadał tę dokładną kopię; identyfikują przekazanie P2.

Odbiór P2: ocena kontraktów i erraty z tym pakietem. **Następne 4B-0**: prawdziwe repozytorium i moduły, działający start, natywny kernel/IPC, testy kontraktów w TS/Rust, EN/PL, skórki i smoke WebView. Bez testowego wysyłania do forum lub niezamówionej publikacji. Repozytorium GitHub można utworzyć także wcześniej; uruchamialna podstawa jest praktycznym momentem dodania kodu, nie ograniczeniem platformy.
