# AI Home — 4B-2A krok 1: lokalny import dokumentów

Wersja 1.0, 20 października 2026. Kandydat do nowego CI/audytu; 4B-2A pozostaje otwarte.

Wynik końcowy: **146/146 Rust Linux headless, 103/103 frontend, TypeScript/build PASS, 21/21 granice, 193/193 P2**. Zwykły równoległy przebieg Rust po korekcie watchdogów testowych przechodzi. Wcześniejszy timing FAIL zachowano w pakiecie; szczegóły w sekcji walidacji.

## Baza i zakres

Bazą jest dokładny `AI_HOME_4B2_KROK_2_POST_AUDIT_SOURCE.zip`, poprzednie przekazanie po korekcie audytu. Edward potwierdził zielone CI i upoważnił dalszą pracę. Nie otrzymano osobnego PASS ponownego audytu ani odbioru Windows/Tauri/WebView. Nie pobierano nowej rewizji GitHub. Identyfikację bazy, źródła Harness i liczniki kontroli zawiera status.json oraz mapa ponownego użycia.

Ten krok umożliwia lokalny import, podgląd i zachowanie oryginałów TXT/MD/DOCX/tekstowego PDF we wskazanej rozmowie. Nie podłącza treści do chat.send, pamięci ani instrukcji systemowych. Zwykły czat nadal wysyła wyłącznie wpisaną wiadomość i dotychczasową historię. Panel EN/PL jednoznacznie to komunikuje. Obrazy, vision, OCR i wyszukiwanie fragmentów nie zostały uruchomione.

## Ponowne użycie

Zaadaptowano parsery documents.rs i natywny wybór plików dialogs.rs z udostępnionej migawki Harness z 19.10.2026. Źródłowy ZIP ma SHA-256 `c772a795bab3d3f11fad1c1b67890e1f2290917cfb8461f77d9162f1b156124f`. Hash oryginałów i docelowych plików oraz różnice zapisano w docs/reuse/4B2A_IMPORT_REUSE.json. Zachowano licencję MIT z atrybucją. Nie kopiowano zasobów/protokołów RV ani domeny treningu.

Przegląd dokumentacji Open WebUI, LibreChat i AnythingLLM posłużył do rozdzielenia ekstrakcji, pełnego kontekstu, wyszukiwania fragmentów i vision. Nie kopiowano ich kodu ani nie dodawano serwera/wektorowej bazy. Odnośniki do oficjalnej dokumentacji:

- https://docs.openwebui.com/features/chat-conversations/rag/document-extraction/
- https://www.librechat.ai/docs/features/upload_as_text
- https://docs.anythingllm.com/chatting-with-documents/introduction

## Przepływ i kontrakty

1. `material.select` otwiera natywny dialog przez Rust. UI nie podaje ścieżek ani bajtów. Najpierw sprawdzany jest profil i otwarta rozmowa. Każdy wybrany plik ma jawny wynik; ponad 12 plików lub 30 MiB całej partii powoduje jawny błąd całego wyboru. Puste, zbyt duże lub nieobsługiwane pliki otrzymują odrębne odrzucenie w wyniku wyboru.
2. Prawidłowe oryginały są przechwytywane jako niezmienne bajty. Token wyboru jest przypisany do profilu/rozmowy, wygasa po 5 minutach i nie może być przeniesiony do innego zakresu. Zmiana pliku na dysku po wyborze nie podmienia przechwyconych bajtów. Anulowanie okna niczego nie importuje.
3. Zamrożony P2 `material.import` przyjmuje token, purpose=conversation i deklarowane autorstwo. Importer to odrębny lokalny aktor human/local-user, bez twierdzenia o zweryfikowanej tożsamości osoby. Autor human/ai/collaboration/unknown jest deklarowany przez importera i pozostaje verified=false.
4. Jedna transakcja zapisuje queued Operation, oryginały, pending ImportedMaterial, staged Attachment, zdarzenie oraz receipt. Dopiero po commicie token jest zużywany i startuje lokalny worker. Błąd receipt wycofuje wszystko i zachowuje token. Powtórzenie tego samego requestId zwraca Accepted bez nowego importu, także po zużyciu tokenu; zmiana payloadu daje konflikt idempotencji.
5. Worker odczytuje TXT/MD UTF-8, DOCX z głównego word/document.xml lub warstwę tekstową PDF. Wynik tekstowy ma osobny blob i SHA-256. Oryginał zachowuje dokładne bajty. Normalizacja tekstu usuwa BOM dla TXT/MD i ujednolica końce linii; nie obcina białych znaków brzegowych ani treści do limitu.
6. Zapis końcowy wszystkich wyników i operacji jest atomowy. Każdy plik ma własny stan ready/unreadable/failed. Częściowy sukces daje succeeded z identyfikatorami również nieczytelnych materiałów; UI pokazuje indywidualne stany, więc succeeded nie oznacza odczytania wszystkich dokumentów. Jeśli żaden plik nie daje tekstu, operacja kończy się błędem, a oryginały nadal istnieją.
7. `operation.get` ma trwały wynik/błąd i zero physicalCallCount. STOP atomowo wycofuje gotowość pending materiałów i nie dopuszcza późniejszego wyniku. Restart oznacza operację interrupted oraz pending materiały jako failed, bez automatycznego wznowienia; oryginały pozostają do pobrania. Nieudany końcowy zapis nie zostawia aktywnego pustego tekstu ani osieroconego wyciągu.

P2 domain.schema.json/ipc.catalog.json pozostają bajtowo identyczne. Material.import/get, Accepted, Operation, ImportedMaterial i Attachment używają zamrożonych DTO. Dodatkowy imports.ipc.json opisuje wybór, stronicowaną listę, odczyt tekstu/oryginalnych bajtów, odłączenie i ostatnią operację. UI mock waliduje żądania i wyniki względem katalogu; testy Rust walidują rzeczywiste Error/ResultEnvelope, Operation i materiały. Metadane nie zawierają oryginalnych ścieżek.

## Dane i interfejs

Migracja v5 → v6 dodaje import_blobs, import_materials i attachments. Oryginały i tekst są zwykłymi lokalnymi BLOB-ami SQLite, bez deklarowania szyfrowania. Złożone FK oraz zapytania pilnują profilu i rozmowy. Dotychczasowe preferencje, klucze w vault, profile, połączenia, trasy, rozmowy, szkice, wiadomości, receipts i prywatny replay zachowano. Odczyt oryginału/tekstu weryfikuje hash; podgląd i pobieranie są stronicowane. Granice stron tekstu respektują UTF-8. Podgląd renderuje zwykły tekst, bez wykonywania HTML/Markdown lub ładowania zewnętrznych zasobów.

UI rozdziela autora materiału od osoby dodającej, pokazuje formaty, limity, stan każdego importu, błędy, lokalny charakter etapu, podgląd, pobranie oryginału i odłączenie. Odłączenie wymaga rewizji Attachment, usuwa tylko powiązanie z listy i zachowuje materiał/oryginalne BLOB-y. Stałe usuwanie i sprzątanie magazynu nie należą do tego kroku. Odrzucenia odczytu przed Accepted są komunikatem danego wyboru, nie rekordami materiałów bez oryginału.

## Limity i granice ekstrakcji

- 12 plików, 15 MiB na oryginał, 30 MiB na wybór; maksymalnie 4 niezużyte wybory i 60 MiB zachowanych wybranych bajtów. Wygasłe wybory są usuwane leniwie przy kolejnym wyborze lub użyciu tokenu.
- Wyciąg tekstowy do 2 MiB, PDF do 200 stron. Przekroczenie odrzuca ekstrakcję, nie obcina zawartości i zachowuje zaakceptowany oryginał.
- DOCX: do 512 wpisów ZIP, 50 MiB deklarowanego rozpakowania, 8 MiB XML, współczynnik kompresji do 120 dla większych wpisów, ograniczenia liczby/wysokości węzłów; odrzucane makra, obiekty aktywne, niebezpieczne ścieżki, DTD/encje.
- Jedno lokalne zadanie parsowania naraz. Budżet oczekiwania 30 s; timeout/anulowanie blokują commit wyniku. Rust spawn_blocking nie zapewnia przymusowego przerwania już działającego parsera. Jego permit jest zachowany do rzeczywistego końca pracy, więc timeout nie uruchamia nieograniczonej liczby kolejnych parserów. To nie jest pełna izolacja procesu dla dowolnego złośliwego PDF-a.
- PDF: tylko warstwa tekstowa, bez OCR/analizy ilustracji. PDF bez tekstu ma PDF_NO_TEXT i zachowany oryginał. PDF mieszany może dać tekst tylko z części stron; nie deklarujemy odczytu skanów ani całego układu.
- DOCX: główna treść, bez nagłówków, przypisów, komentarzy i analizy grafik/układu; ograniczenie jest widoczne w panelu.

## Walidacja i odbiór

Przy końcowym przebiegu równoległym pięć wcześniejszych testów czatu przekroczyło krótkie watchdogi fixture (5/8 s), a kolejne uruchomienie ujawniło analogiczny timeout fixture połączeń (4 s), mimo poprzedniego pełnego PASS 145. Log niepowodzenia zachowano osobno. Oczekiwanie fixture czatu/połączeń na fizyczne odebranie żądania i trwały wynik zwiększono do 30 s, a testowy polling trwałego wyniku zmniejszono z 2–5 ms do 25 ms, aby ograniczyć konkurencję o SQLite podczas równoległych testów; nie zmieniono asercji, retry, deadline HTTP ani budżetów aplikacji. Fixture importu czeka do 45 s, aby również terminalny błąd budżetu importu 30 s mógł zostać odczytany. Diagnostyka pojedyncza potwierdziła 15/15 wcześniejszych testów czatu; jej sesja wykonawcza zakończyła się przed pełnym raportem importów, więc nie liczymy jej jako pełnego PASS. Niepełny log zachowano z odpowiednią nazwą. Końcowy wynik po korekcie watchdogów jest zapisany oddzielnie od wcześniejszych przebiegów.

Aktualne wyniki znajdują się w docs/evidence/4b2a-step-1/status.json oraz odrębnych logach. Testy używają lokalnych, generowanych dokumentów i podstawionego wyboru ścieżek. Nie otwierają rzeczywistego dialogu i nie wykonują płatnych API. Zachowano regresje szkiców, opóźnionego IPC, idempotencji, providerów i izolacji poprzednich etapów.

Nie przeprowadzono Windows/Tauri/WebView, natywnego okna wyboru i pobierania na komputerze, rzeczywistego vault ani live API. Dodano przypięte zależności parserów/dialogu i aktualny Cargo.lock, zachowując wcześniejsze wersje pakietów. Nie zmieniano workflow, capabilities okna ani zestawu frontendowych poleceń Tauri — UI nadal używa tylko home_dispatch. Plugin dialogu jest wywoływany z Rust; nie dodano przeglądarce ogólnych uprawnień plikowych.

Potrzebne nowe CI na dotychczasowym workflow i audyt tej paczki. Bez release i instalatorów. Następny krok: podłączenie dokumentów do czatu z pełnym preflight, zachowaniem użytej wersji źródła i uczciwym raportowaniem zakresu; następnie obrazy/vision. 4B-2A, wcześniejszy odbiór desktopu i ponowny audyt 4B-2 pozostają otwarte.
