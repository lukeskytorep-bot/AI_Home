# AI Home 4B-2A krok 3 — obrazy i vision

21 października 2026. Kandydat do nowego CI i niezależnego audytu. Edward potwierdził zielony wynik całego poprzedniego kroku 2 i polecił kontynuację. Nie dostarczono identyfikatora przebiegu/platform ani nowego tekstu werdyktu audytu. Całe 4B-2A pozostaje otwarte.

## Co działa

Natywny wybór plików obejmuje dokumenty oraz statyczne PNG, JPEG, WebP i GIF. Import sprawdza sygnaturę bajtów, zgodność rozszerzenia, wymiary i rzeczywiste dekodowanie. Ograniczenia: 10 MiB na obraz, maksymalnie 16 384 px na bok i 40 mln pikseli, limit alokacji dekodera 192 MiB; wspólny wybór nadal ma do 12 plików i 30 MiB. Są to granice bezpieczeństwa, nie obietnica, że każdy plik poniżej nich przejdzie każdy kodek. Animowany GIF, APNG i animowany WebP są odrzucane jawnie, zamiast wysyłać wyłącznie pierwszą klatkę. SVG/HEIC/HEIF są poza zakresem.

ImportedMaterial obrazu ready ma oryginalny blob/hash i zweryfikowane imageMetadata(format,width,height). Nie ma sztucznego extractedBlobId, importMethod ani OCR. Materiał odrzucony przez parser ma własny stan/błąd i zachowuje oryginał. Mieszany batch zachowuje wynik każdego pliku. Nie ma model calls podczas importu.

Dokument i obraz można zaznaczyć do tej samej wiadomości. Obraz jest przesyłany jako oryginalne bajty w base64; nie ma automatycznej konwersji, kompresji, zmiany rozmiaru/orientacji ani usuwania metadanych EXIF. UI EN/PL przed wysłaniem informuje o przekazaniu oryginału wraz z ewentualnymi metadanymi dostawcy i o braku OCR. Lokalny podgląd pobiera zweryfikowane bajty przez scoped IPC i tworzy Blob URL; URL jest odwoływany po zamknięciu, zastąpieniu i odmontowaniu panelu. Nie są pobierane zdalne URL-e obrazów.

## Vision i granica konfiguracji

Lista dostępnych modeli nie dowodzi obsługi obrazu. W tym kroku nie dodano katalogu automatycznie atestowanych capabilities. Nowy osobny vision.ipc.json opisuje chat.vision.get/set: użytkownik jawnie deklaruje supported albo unsupported dla dokładnego modelu i endpointu. Domyślny unknown blokuje obrazy kodem MODEL_CAPABILITY_UNKNOWN; unsupported kodem VISION_UNSUPPORTED. To deklaracja użytkownika, nie test API ani dowód producenta. Funkcja nie zgaduje po nazwie modelu, nie włącza vision globalnie dla providera i nie korzysta z cudzej deklaracji.

Zakres obejmuje profil, ID/rewizję Primary route, model, ID/rewizję połączenia, endpoint/provider oraz rewizję klucza. Zapis ma CAS osobnej policyRevision i expectedRevision trasy. Zmiana modelu, trasy, połączenia, endpointu lub klucza unieważnia deklarację. Zmiana samego limitu kontekstu przenosi wyłącznie nadal zgodną deklarację na nową rewizję tej samej trasy. Zmiana deklaracji po przyjęciu żądania jest wychwytywana przed wywołaniem i przy końcowym zapisie — spóźniona odpowiedź nie trafia do historii.

Obrazy w poprzednich wiadomościach również wymagają vision. Zmiana na nieobsługującą trasę nie usuwa ich po cichu i nie robi tekstowego fallbacku. Wymaga ponownej jawnej konfiguracji albo oddzielnej rozmowy tekstowej; segmentacja/wybór kontekstu to dalsze moduły.

## Źródła i trwałość

Wykorzystano istniejące material.import/get, import_blobs/materials/attachments, message_documents, receipts, operacje, STOP i restart. SourceRef obrazu wskazuje oryginalny blob i SHA-256; dokument nadal wskazuje ekstrakcję. Snapshot metadanych utrwala wersję, pochodzenie i kolejność. Kolejne tury oraz restart odtwarzają dokładnie te same bajty i metadane, weryfikując hash i rozmiar. Metadane wiadomości wskazują kolejność imagePartIndex względem części obrazu. Dokumenty pozostają niezaufanymi danymi użytkownika, bez awansu do system promptu, pamięci lub zasad.

Accepted atomowo zapisuje wiadomość człowieka, powiązania bound, snapshoty, operację, receipt i zdarzenia. Niepowodzenie receipt wycofuje całość przed HTTP. Ponowienie tej samej próby nie dubluje wiadomości ani źródeł; provider retry ma identyczny payload. STOP zachowuje przyjętą wiadomość człowieka i źródła, nie zapisuje spóźnionej odpowiedzi. Bound nie można odłączyć ani przypiąć drugi raz. Oryginał po restarcie nie wymaga ponownego wyboru z dysku. Nie dodano szyfrowania bazy, trwałego kasowania, reattach ani backupu.

SQLite v7 → v8 dodaje tylko chat_vision_policies. Dawne dane i struktury źródeł pozostają zachowane. Baza v8 nie jest przeznaczona do otwierania przez starszą aplikację akceptującą najwyżej v7.

## Kontekst i protokoły

Wspólne prepare() nadal obsługuje read-only preflight i atomowe chat.send. Dla obrazów pomiar ma jawny identyfikator utf8_bytes_and_image_reserve_estimate. Obciążenie obrazu to długość base64 +8192 +4*ceil(piksele/256); dochodzi pełny tekst/metadane, prywatny replay, narzut wiadomości, wyjście i rezerwa profilu. To konserwatywna heurystyka lokalna, nie tokenizer ani gwarancja rozliczenia lub przyjęcia przez model. Duży skompresowany obraz nie jest liczony wyłącznie według rozmiaru pliku. Pomiar wskazuje routeId/revision, imageCount/imageInputTokens i visionPolicyRevision. UI unieważnia go po zmianie deklaracji, a send sprawdza wszystko ponownie.

Nie ma cichego przycinania historii, zmniejszania obrazów ani usuwania części. Całe żądanie ma do 20 obrazów, również z wcześniejszych tur, oraz istniejące limity buforów i transportu. Przekroczenie zatrzymuje wywołanie przed zapisem/HTTP. Ten limit jest celowo lokalny, mniejszy od niektórych limitów dostawców.

Gotowe buildery Harness zachowano bajtowo: OpenAI/OpenRouter/custom → image_url z data URL; Google → inlineData; Anthropic → image/source/base64. Nie zmieniano endpointów, autoryzacji, retry, response parsing ani prywatnego replay. Dodano walidację dozwolonych MIME, roli user, liczby części i poprawności base64 w protokole. Na natywnej trasie Google GIF jest odrzucany bez konwersji; dla natywnego Anthropic odrzucane są obrazy powyżej 8000 px/bok i 10 000 000 bajtów base64. OpenRouter/custom nadal zależą od deklaracji użytkownika i rzeczywistych możliwości wybranej trasy; ten etap nie atestuje każdego backendu ani własnego ograniczenia każdego modelu.

Oficjalne odniesienia sprawdzone 21.10.2026:
- https://ai.google.dev/gemini-api/docs/image-understanding — obsługiwane MIME i inline data.
- https://platform.claude.com/docs/en/build-with-claude/vision — bloki base64, formaty i granice żądania.
- https://developers.openai.com/api/docs/guides/images-vision — data URL i formaty obrazów statycznych.
- https://docs.rs/image/0.25.10/image/struct.ImageReader.html — limity i dekodowanie kodeków.

## Weryfikacja

| Kontrola | Wynik |
| --- | --- |
| Rust Linux headless, --all-targets --locked --offline, domyślnie równolegle | 169/169 PASS |
| Frontend Vitest/jsdom | 113/113 PASS |
| TypeScript/Vite | PASS |
| Granice modułów | 21/21 PASS |
| Kontrole P2 | 193/193 PASS |
| Usunięte/zaktualizowane stare wpisy Cargo.lock | 0 |
| Nowe wpisy Cargo.lock | 10 |
| Nakładka → pełne źródła, manifest, SHA256SUMS | wynik w OVERLAY_VERIFICATION.log |

16 nowych testów Rust: 4 dekodera (4 formaty, uszkodzenia, podszyte rozszerzenie, pixel bomb, GIF/APNG i rozmiar); 1 dokładnych payloadów wszystkich 5 protokołów i odrzucenia obcych MIME/base64; 2 ograniczeń dostawców; 7 integracyjnych czatu (realny select/import, unknown/unsupported/supported, SourceRef bez ekstrakcji, obraz w następnej turze, CAS/unieważnienie po kluczu, zachowanie po zmianie limitu kontekstu, spóźniony wynik, przepełnienie i receipt rollback, rzeczywista migracja/restart/hash, retry/STOP i mieszany dokument+obraz); 2 importu (mixed batch wszystkich formatów, oryginały i brak fikcyjnej ekstrakcji, osobne odrzucenia uszkodzonego/animowanego obrazu).

4 nowe testy UI: deklaracja vision dla właściwej trasy/CAS, unieważnienie pomiaru po jej zapisie, Blob preview obrazu bez odczytu OCR i sprzątanie URL, blokada zaznaczenia/odłączenia bound wraz z czyszczeniem po zmianie rozmowy. Dotychczasowe testy niepewnego wysłania zachowują identyczny wybór załączników i requestId.

Początkowe nieudane uruchomienia zachowano: test APNG wstawiał acTL przed IHDR, więc testował wadliwy nagłówek zamiast animacji — fixture poprawiono na acTL po IHDR; mock bound w UI tworzył stan raz przy załadowaniu modułu — mock poprawiono na aktualny stan w każdym odczycie; zbiorcza aktualizacja oczekiwanego user_version przypadkowo zmieniła jedną asercję liczby zdarzeń 7 → 8 — przywrócono prawidłowe 7. To błędy w testowych zmianach, nie powody do poluzowania asercji runtime. Końcowy pełny zestaw jest zielony. Nie zwiększano deadline ani watchdogów. Dawne ostrzeżenia dead_code i unused imports pozostają widoczne.

Nie wykonano desktop feature, natywnego WebView/Windows ani wizualnego renderowania w przeglądarce. Testy UI są jsdom; rzeczywiste wyświetlanie obrazu i natywny dialog/pobieranie nadal wymagają odbioru. Nie używano rzeczywistego OS vault ani live/płatnych API — sieć testowa to loopback, klucze są atrapami. Dekodowanie w blocking worker nie jest izolowanym procesem; STOP/budżet zapobiega zatwierdzeniu wyniku, a rozpoczęte dekodowanie może zakończyć się później z nadal zajętym pojedynczym permit. Nie dodano OCR ani skanowania treści pliku.

## Przekazanie

Baza: AI_HOME_4B2A_KROK_2_SOURCE.zip, SHA-256 `50324d21080263c37563212cbf68c266aa0924b614c29fc93b267ab6fa84cc4d`. Zależność image =0.25.10 jest przypięta; używa wyłącznie PNG/JPEG/GIF/WebP. Cargo zachował wszystkie wcześniejsze wersje; dołączono nowy Cargo.lock. Nie wymaga Prepare Cargo lockfile. Frontend dependencies, workflow i capabilities bez zmian. Zamrożone P2/foundation i wcześniejsze suplementy poza własnym document-chat.ipc.json zachowano bajtowo; nowy vision.ipc.json jest osobny. Snapshoty i starsze raporty/dowody są historyczne i nie zostały przepisane.

Ponownie wykorzystano image validation z tej samej migawki Harness z 19.10.2026, z zachowanym MIT notice. Funkcje sygnatur i wymiarów przeniesiono, a dekodowanie wzmocniono limitami oraz jawnym odrzuceniem animacji. Pochodzenie i hashe opisuje docs/reuse/4B2A_IMAGE_REUSE.json. Nie kopiowano modułów RV.

Nałożyć CHANGED_FILES.zip w głównym katalogu istniejącej dokładnej bazy, bez kasowania plików, commit i push. Istniejące CI wystarcza; nie przygotowywano release, instalatorów ani dodatkowych jobs. Nowe CI i niezależny audyt nadal są wymagane. Po przyjęciu zakresu 4B-2A następny etap funkcjonalny to 4B-3 — Scribe i Working Context, przy nadal otwartych osobnych odbiorach desktopu/vault/live API.
