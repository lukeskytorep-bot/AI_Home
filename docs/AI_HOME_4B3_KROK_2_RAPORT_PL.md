# AI Home — 4B-3 krok 2: trwały Working Context, modele i panel

21 października 2026. **Wykonano lokalnie; przekazanie do nowego CI/audytu. Całe 4B-3 pozostaje otwarte.**

## Co działa

W wybranej rozmowie pojawia się panel Working Context. Użytkownik zapisuje pojemność oraz jawne okna kontekstu modeli i wybiera automatyczny Scribe albo wyłączenie Scribe. Ręczny przycisk uruchamia aktualizację z tekstu rozmowy. Scribe (albo Primary w jego roli) proponuje, potem Primary osobno akceptuje, zmienia lub odrzuca. Panel pokazuje rzeczywiste próby, publiczne uzasadnienie i ostatnią zapisaną wersję. Wyłączenie Scribe nie usuwa etapu decyzji.

**Ten krok zapisuje i pokazuje pamięć, ale nie dołącza jej jeszcze do kolejnych wywołań czatu.** To zaplanowany krok 3. Pliki i obrazy w rozmowie nie są w tym kroku czytane przez Scribe. Brak OCR, RAG, automatycznej aktualizacji po odpowiedzi i działań w tle tylko dlatego, że program jest otwarty. Panel ostrzega o tym zakresie. Odczyty stanu działają lokalnie i nie wywołują modeli.

## Dokładna baza

`AI_HOME_4B3_KROK_1_SOURCE.zip` — SHA-256 `2ba80109442b38c99d6c7d8abdb5f1d5ceff05c6dcd11c6e9dca508b21cd7331`. Edward potwierdził zielony wynik tej paczki i upoważnił następny krok; bez numeru/platform przebiegu lub nowego tekstu audytu. Nie użyto innego GitHub ZIP-a. Plan zachowuje nazwę i ma treść 1.25.

## Moduły i IPC

- Nowy `src-tauri/src/memory_runtime.rs` jest natywnym koordynatorem obok chat/import runtime, poza kernelem. Czysty `memory/mod.rs` nadal nie zależy od infrastruktury; otrzymał odczyt pending candidate i walidowane metadane wypowiedzi: rola human/assistant, język, sequence, data. Modele dostają te informacje jako dane. Sam hash tekstu nie poświadcza prawdziwości wypowiedzi ani niezależnej weryfikacji wcześniejszych twierdzeń AI.
- Zamrożony `memory.update.request`: payload dokładnie `recordId`/`sourceRefs`; expectedRevision dotyczy rekordu pamięci. UI nie przesyła content, PrimaryDecisionOutput, roli, receipt czy zatwierdzenia. Nie ma polecenia UI zatwierdzającego zapis za agenta.
- Suplement `contracts/memory.ipc.json`: `memory.working.get` (bieżąca wersja, ustawienia i ślad ostatniej operacji), `memory.working.sources` (referencje wiadomości wybranej rozmowy), `memory.settings.set` (scopeId + expectedRevision profilu + pojemność/tryb/okna). Dotychczasowe kontrakty, w tym całe P2/foundation, są niezmienione bajtowo.
- `profile.snapshot` pokazuje prawdziwe memoryRecords. Zapis profilu tworzy pusty rekord L2, migracja dodaje go dawnym profilom. Po jednym Working Context na profil; nie jest to odrębna pamięć każdej rozmowy. Kolejna rozmowa świadomie aktualizuje ten sam profilowy zapis, z poprzednią zatwierdzoną treścią jako danymi.

## Trwałość i ochrona zakresu

SQLite **v8→v9** dodaje memory_records, memory_proposals, memory_decisions, memory_attempts, memory_versions, memory_runs i memory_contexts. Klucze i FK zachowują profile_id; indeks ogranicza profil do jednego working record. Rewizje i DTO są walidowane przy odczycie/zapisie. Dawne rozmowy, dokumenty, obrazy, replay, receipts i preferencje pozostają zachowane. Dawne testy migracji zmieniają tylko oczekiwaną końcową wersję 8→9; źródłowe numery historycznych fixture pozostały bez zmian.

Przy Accepted w jednej transakcji powstają queued Operation, natywny snapshot i receipt idempotencji. Błąd receipt cofa całość przed HTTP. Snapshot zawiera profil/ustawienia, rekord/bazę wersji, obie trasy i połączenia, credentialRevision/ref, teksty i metadane źródeł oraz język rozmowy. Nie zawiera surowego klucza API.

Źródła są rozwiązywane z własnej bazy native. Akceptowane są kompletne wiadomości human/assistant jednej otwartej rozmowy, bez duplikatów i bez obcego profilu. Hash jest liczony ponownie. Teksty źródeł i poprzedniej pamięci idą w JSON user data, nie są wklejane do system promptu. Role i daty nie pochodzą z deklaracji modelu. Snapshot pełnych wiadomości chroni także przed zmianą źródła bez podniesienia jego rewizji. UI nie przekazuje dowolnych ścieżek ani sekretów.

Koordynator porównuje aktualny profil, pamięć/bazę wersji, trasy/połączenia/klucze/konteksty i źródła przed wywołaniami oraz przed commit. Trwały RecordAttempt sprawdza zakres przed każdym fizycznym POST, także retry. Sama zmiana modelu, endpointu, klucza albo polityki nie przenosi starej kandydatury na nowy zakres. Ustawienia mają odrębny pełny scopeId, porównywany z CAS w transakcji; stary panel nie zatwierdzi deklaracji dla nowego zakresu. Retest identycznej konfiguracji nie zmienia jej tożsamości tylko przez testedAt.

Propozycje, decyzje i próby są utrwalane po etapach. MemoryDecisionReceipt powstaje wyłącznie z przypisanej odpowiedzi wywołania Primary. Pomiar/hash dotyczą ostatecznej treści, również po MODIFY; wskazują ID i rewizję Primary. Ostatni receipt musi odpowiadać kandydaturze i bieżącej operacji. Finalny commit atomowo tworzy wersję, przełącza rekord, utrwala wynik Operation oraz zdarzenia. Konflikt lub błąd zapisu nie zostawia aktywnej pustej pamięci ani osieroconej wersji. Poprzednie wersje pozostają niezmienne i mają previousVersionId.

REJECT jest zakończoną decyzją `succeeded` + `memory_update/no_change`; nie generuje ponowienia dla wymuszenia zgody. Błędy formatu/pojemności/provider są failed z kodami zamrożonego Error; odwołanie do poprzedniej aktywnej wersji nie zmienia się. STOP zamyka operację i pending proposal, a późny wynik nie aktywuje wersji. Restart oznacza queued/running jako interrupted, zamyka pending proposals i nie wznawia sieci. Odczyt po restarcie odzyskuje wynik/błąd/wersję bez zdarzeń.

## Budżety, modele i język

Domyślne L2 300, presety 300/500/800/1000, własna dodatnia wartość i unlimited. Unlimited nie wprowadza ukrytego limitu treści; panel ostrzega o koszcie i możliwym przekroczeniu kontekstu. Zmniejszenie pojemności nie obcina/usuwa istniejącej wersji. Pozostałe memorySettings profilu nie są zmieniane.

Pomiar to **estimated / utf8_bytes_v1**, konserwatywny szacunek bajtowy, nie tokenizer lub gwarancja dostawcy. JSON/instrukcje/pełne źródła/poprzednia treść/odpowiedź/rezerwa są sprawdzane przed każdym żądaniem obu modeli. Brak cichego obcinania. Okna modeli deklarowane są jawnie, osobno dla obu tras pamięci; nie zgadujemy ich i nie zmieniamy ustawienia okna czatu. Zmiana zakresu unieważnia deklarację. Nie są to zweryfikowane możliwości API. Zbyt duży skończony limit zapisu względem limitu odpowiedzi daje OUTPUT_BUDGET_INSUFFICIENT przed płatną operacją.

Wykorzystano istniejące HttpTransport/BoundCredential/CallBudget/RecordAttempt. Wszystkie wywołania (Scribe, Primary, retry, naprawa, recovery) dzielą najwyżej **4 fizyczne POST-y / 180 sekund**, albo niższy skonfigurowany limit profilu. Rezerwacja licznika jest trwała przed HTTP. Jedna naprawa formatu i dwie próby długości nie resetują globalnej puli. Druga próba może zakończyć się błędem budżetu. Kontrola kontekstu obejmuje do 8 MiB przygotowanego tekstu i 16 MiB zakodowanego żądania jako granice ochronne; nie są to pojemności pamięci. Błędne schematy providerów zachowują istniejącą klasyfikację i politykę retry. Brak ukrytego plaintext vault/fallbacku.

Istniejące adaptery OpenAI/OpenRouter/Google/Anthropic/Custom OpenAI pozostają bez zmian. Natywny test loopback sprawdza faktyczny compatible POST, odrębny model Scribe i Primary oraz wspólny budżet. To **nie jest live test pięciu dostawców**. Każdy etap jest nowym stateless zadaniem z danymi; nie skopiowano historii providerowego reasoning pomiędzy rolami. Nie dodano streamingu/narzędzi.

Domyślnie język pochodzi z ostatniej wskazanej wiadomości człowieka według sequence (fallback: ostatnia wskazana wiadomość). Locale UI nie bierze udziału. Istniejąca explicit_translation używa memoryLanguage profilu. Testy sprawdzają pl pod EN UI oraz jawną politykę en; odpowiedzi są kontrolowanymi fixture, nie dowodem jakości językowej żywego modelu.

## Panel i odzyskiwanie

EN pozostaje domyślny; PL ma równoległe klucze. Konfigurator połączeń nadal wybiera niezależne modele. Panel pamięci pokazuje zapisane okna/pojemność i wymaga ich konfiguracji przed nową pracą. Zmiana modelu unieważnia okno; wyłączony Scribe nie wymaga jego skonfigurowania. Widoczne są obie próby z pomiarami, rzeczywisty wynik, publiczne reason, aktualna treść i hashe źródeł ostatnich propozycji. Teksty są renderowane przez React, bez HTML modeli.

Refresh/polling nie nadpisuje niezapisanych ustawień. Zapis używa zakresu faktycznie odczytanego przy edycji; konflikt nie przenosi szkicu automatycznie na nowe połączenie. Jawne Reload settings odrzuca szkic. Niepewne memory.update.request zachowuje dokładny requestId/payload/expectedRevision w rodzicu także przy zmianie rozmowy. Accepted rozlicza próbę przed następnym odczytem stanu, również po unmount; rodzic powiadamia aktualny panel. Ponowienie nie pobiera innych źródeł ani nie zmienia ID. Nie ma udawanej decyzji agenta klikanej przez człowieka.

## Testy i dowody

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline, domyślnie równolegle | **228/228 PASS** |
| Nowe integracje runtime pamięci | **27/27**, wliczone w 228 |
| Czysty silnik pamięci | **30/30**, wliczone w 228 |
| Frontend | **130/130 PASS**, 10 plików |
| TypeScript/Vite | **PASS** |
| Granice modułów / P2 | **23/23 / 193/193 PASS** |

Rozkład Rust: 79 unit/provider/vault +31 chat +24 connections +10 conversations +8 foundation +19 imports +30 pure memory +27 memory runtime. Nowych testów względem bazy: 28 Rust i 15 UI. Natywny zestaw obejmuje DTO, idempotencję, izolację, CAS, MODIFY za długi, dwie porażki, REJECT, format repair, transport retry z wyczerpaniem wspólnej puli, zmianę modelu/klucza/polityki/źródła, rzeczywisty plik SQLite/restart, migrację, rollback receipt/wersji i bezpieczny błąd auth. UI sprawdza payload/result względem kontraktów (nie mock ignorujący kształt), dirty drafts, stale scope, niepewne żądanie, unmount, STOP, terminal polling, unlimited i escaped treści.

Logi: docs/evidence/4b3-step-2. Wcześniejsze compile/runtime/frontend/rust/ui logi z suffix initial są wcześniejszymi przebiegami i mają mniejsze liczniki. build-initial wykazał TS2554: useRef wymaga argumentu w bieżącym React; poprawiono przez jawne undefined. Końcowe build.log/frontend-tests.log/rust-tests.log dotyczą finalnego źródła. Istniejące ostrzeżenia dead_code i unused imports w dawnych testach nie są błędami. Nie zmieniano budżetów/testowych timeoutów, aby uzyskać zielony wynik.

Nie uruchomiono natywnego WebView, Windows/desktop feature, prawdziwego OS vault ani płatnego API. Odbiory te pozostają otwarte. Test restartu z pending operacją odtwarza utrwalony stan przerwanego procesu; nie jest testem zabicia desktopu w WebView. Migracja v8 odtwarza poprzedni schemat przez usunięcie wyłącznie nowych tabel z testowej bazy i ustawienie user_version=8, zachowując dawne rekordy.

## Przekazanie i dalszy zakres

Nakładka na dokładnej bazie kroku 1 nie usuwa plików. Kontrakty P2/foundation i poprzednie suplementy, Cargo.toml/Cargo.lock/package-lock, provider/vault, workflow i capabilities pozostają bajtowo bez zmian. Własny memory.ipc.json jest nowym suplementem. Historyczne snapshoty i dowody zachowano; aktualne manifesty są odnowione dla pełnego drzewa. Nie kopiowano nowych plików Harness; koordynator korzysta z już adaptowanej wspólnej infrastruktury, z zachowaniem wcześniejszych notices.

Wystarczy istniejący push CI; nie potrzeba nowego workflow, Prepare Cargo lockfile, release ani instalatorów. Po CI/audycie następny krok to aktywny L2 w chat.preflight/send z aktualnym pomiarem i walidacją pamięci podczas rozmowy. Pozostałe warstwy/wyszukiwanie/import pamięci/przywracanie wersji nadal należą do 4B-4. Ten krok nie zamyka całego 4B-3 ani dawnych bramek desktop/API/vault.
