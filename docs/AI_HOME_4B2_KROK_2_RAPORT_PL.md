# AI Home — 4B-2 krok 2: wykonywalny czat tekstowy

Wersja 1.1, 19 października 2026. Kandydat do nowego CI/audytu. 4B-2 pozostaje otwarte.

## Korekta po audycie

Audyt kroku 2: DO KOREKTY. Ta wersja jest kandydatem do ponownego CI i audytu, nie zamyka etapu.

- Accepted rozlicza wysłany tekst synchronicznie w stanie rodzica, przed odczytem historii i usunięciem próby. Callback działa również po odmontowaniu panelu. Zmienia wyłącznie szkic oryginalnej rozmowy, jeżeli nadal odpowiada wysłanej treści; zachowuje późniejsze edycje i ekran aktualnie wybranej rozmowy.
- Historia i conversation.get są uzgadniane niezależnie. Polling zmienia stan operacji dopiero po uzgodnieniu, aby własny cleanup nie odrzucał spóźnionej rewizji. Błąd historii nie blokuje aktualizacji rekordu.
- Załączniki w tym kroku zwracają istniejący kod VALIDATION_ERROR i jego lokalizowany komunikat. Test rzeczywistego dispatchera waliduje zarówno Error, jak i ResultEnvelope zamrożonego P2; kontrakty pozostają bajtowo niezmienione.

Cztery nowe testy UI: przełączenie rozmowy przed Accepted, błąd odczytu historii po Accepted wraz z ręcznym odświeżeniem, późniejszy szkic przy przełączeniu oraz opóźniony odczyt historii przy terminalnym pollingu. Uruchomienie przeciw poprzednim źródłom odtwarza trzy błędy; ochrona nowszego tekstu przechodzi również wcześniej. Log before-ui zachowuje interfejs callbacku poprzedniej wersji, aby nie mylić zmiany sygnatury z regresją.

Źródło tej korekty: dokładny poprzedni SOURCE kroku 2, bez pobierania nowej rewizji GitHub. Nakładka zawiera tylko zmienione/dodane pliki względem tego źródła. Bez usuwania plików, nowych workflow, release, instalatorów, zależności, migracji i zmian kontraktów. Wyniki i identyfikacja bazy: docs/evidence/4b2-step-2-correction/status.json. Logi 4b2-step-2 poniżej są historycznym wynikiem przed korektą.

Wyniki korekty: **125/125 Rust headless**, **93/93 frontend**, TypeScript/build PASS, **20/20 granice**, **193/193 P2**. Wykonano pełny zestaw Rust na Linux bez desktop feature; vault w testach jest wstrzyknięty, HTTP loopback. Nie wykonano live API, rzeczywistego OS vault, Windows/Tauri/WebView ani renderowania przeglądarki.

## Baza i potwierdzenia

Edward napisał, że 4B-1 przeszedł na zielono na GitHub, i upoważnił kolejny krok. Zachowano dokładne brzmienie: nie jest to osobne potwierdzenie CI/audytu poprawki 4B-2 krok 1. Brak numeru przebiegu, commitu i zakresu platform. Bazą kodowania jest ostatni poprawiony SOURCE kroku 1, SHA-256 `93ad8491d27e63c78bb5f45684de9d69698b9130ffe6a3e1bce34fe6e62ca8db`. Starsza spójna baza 4B-1 + deadline, na którą powstaje skumulowana nakładka: `0b7c424b3fa2b07aa221ecd80d922472f5c85ee2a816f11e53418bd8a703c2c4`. Nie pobierano nowego stanu GitHub. Obie bazy plus nakładka są odtwarzane do identycznego nowego drzewa; log przekazania zawiera dowody.

Paczka zawiera także wcześniejszą część 4B-2, aby nie zakładać, że każdy plik kroku 1 został już nałożony. Nie usuwa plików, nie aktualizuje zależności ani nie tworzy nowych jobów/release/instalatorów. Cargo.toml/Cargo.lock, package.json/lock, workflow, uprawnienia i wszystkie dotychczasowe kontrakty/snapshoty zachowano bajtowo.

## Przepływ

UI wybiera profil i rozmowę, korzysta z jego przypisanej trasy Primary oraz zweryfikowanego połączenia. Scribe i pamięć nie są wywoływane w tym kroku. Wiadomość nie trafia do innego profilu lub rozmowy. Niepuste attachmentIds są odrzucane do czasu 4B-2A. Świadomy tryb autonomiczny i narzędzia nie są uruchamiane.

1. Native waliduje P2 CommandEnvelope/chat.send, zakres, rewizję rozmowy, trasę i klucz, kontekst oraz limity. Nieznane capability/reasoning, niegotowe połączenie lub za duża historia blokują HTTP. Jedna rozmowa może mieć tylko jeden queued/running chat.
2. Transakcja zapisuje oryginalną wiadomość człowieka, czyszczony szkic rozmowy, queued Operation, prywatny snapshot chat_runs, receipt i zdarzenia. Przed commit nie uruchamia się HTTP. Ten sam requestId/payload/expectedRevision zwraca zapisany Accepted i nie uruchamia drugi raz. Inny payload z tym ID daje konflikt.
3. Running Operation używa istniejącego wykonawcy providerów, jednego wspólnego CallBudget i durable CallRecorder przed każdym fizycznym wywołaniem. Budżet obejmuje także odczyt vault, utworzenie klienta i oczekiwanie. Liczniki retry/recovery zachowują reguły 4B-1: maksymalnie 4 fizyczne wywołania/180 s, a niższy skonfigurowany limit zostaje zachowany. Restart nie wznawia sieci. Nie obiecujemy exactly-once po stronie dostawcy, gdy odpowiedź POST została utracona i transport wykonuje retry.
4. Przed końcowym commit native ponownie sprawdza anulowanie i aktualność profilu, trasy oraz połączenia/rewizji klucza. Stary wynik nie jest dopisywany po zmianie konfiguracji. Nowa edycja szkicu w czasie generowania nie blokuje odpowiedzi i nie jest nadpisywana.
5. Jedna transakcja zapisuje pełną widoczną odpowiedź AI, jej prywatny replay (jeśli potrzebny), rewizję rozmowy, końcowy typed ChatOperationResult, recordRefs i zdarzenia. Błąd zapisu nie pozostawia osieroconej odpowiedzi. Wiadomość człowieka pozostaje w historii także przy błędzie lub anulowaniu; nie tworzymy fikcyjnej odpowiedzi AI.
6. UI odczytuje operation.get i oryginalne wiadomości przez stronicowane IPC. STOP anuluje aktywną operację; terminalnego wyniku nie nadpisuje spóźniona odpowiedź. Ostatni stan jest pobierany po ponownym otwarciu rozmowy. Jeśli polling napotka błąd transportu, użytkownik może użyć Odśwież czat. W tej wersji historia jest pobierana wszystkimi stronami i renderowana jako zwykły tekst, bez interpretacji HTML.

Accepted oznacza przyjęcie do wykonania, a nie odpowiedź modelu. UI czyści wysłany szkic dopiero po odzyskaniu przyjęcia; nowe edycje wpisane w trakcie pozostają zachowane. Przy niejasnym błędzie wysłania ten sam attempt zachowuje requestId i pierwotną rewizję w pamięci ekranu, także przy przełączeniu rozmowy. Zamknięcie aplikacji nie zachowuje tej lokalnej tożsamości próby, lecz trwała historia i ostatnia operacja są dostępne; nic nie jest automatycznie wysyłane ponownie.

## Kontekst i limity

Obecne discovery daje listę modeli, nie zweryfikowaną pojemność kontekstu. Nie zgadujemy jej z nazwy modelu. Nowe scoped chat.settings.get/set pozwala użytkownikowi jawnie zadeklarować contextTokens 1024–2000000 na podstawie dokumentacji modelu. UI opisuje source=user_declared i measurement=utf8_bytes_estimate. Zapis używa CAS rewizji trasy i podnosi ją; deklaracja jest ważna wyłącznie dla tej rewizji. route.save/model change wymaga nowej deklaracji. get/set zwracają także efektywny outputTokens i inputReserveTokens, wyświetlane w UI. Ustawienie nie zmienia tożsamości profilu.

Do modelu trafiają instrukcja językowa, cała dotychczasowa historia w kolejności sequence i nowa wiadomość. Brak podsumowania, automatycznego skracania lub pomijania starszych wiadomości. Pomiar jest jawnym ostrożnym oszacowaniem: bajty UTF-8 treści, narzut 64 na wiadomość, rozmiar prywatnego replay oraz inputReserveTokens. Nie jest to właściwy tokenizer dostawcy ani gwarancja, że błędna deklaracja użytkownika odpowiada rzeczywistemu limitowi modelu. Przekroczenie blokuje wysłanie; interfejs proponuje nową rozmowę/sprawdzenie limitu. Historyczny magazyn danych nie jest obcinany. Dodatkowy guard >8 MiB danych historii również odrzuca żądanie bez skracania; to limit tej implementacji, nie pojemność pamięci L2.

Limit odpowiedzi to min(route.outputLimitTokens, profile.contextBudget.chatOutputTokens), obecnie domyślnie 1024. Deklaracja dużego kontekstu nie zwiększa wyjścia. Zaawansowany koszt bounded jest odrzucany jako COST_ESTIMATE_UNKNOWN, dopóki nie ma cennika; domyślny unconfigured pozostaje jawny. Nie udajemy obsługi załączników, narzędzi ani zaawansowanego reasoning. Limit/refusal/incomplete output pozostają terminalne zgodnie z 4B-1, bez kontynuacji, zwiększania budżetu ani dopisywania partial text. Błędy uwierzytelnienia/policy/cancel nie są oznaczane retryable.

## Prywatny replay i trwałość

Widoczny Message nie zawiera hidden reasoning, podpisów ani surowego payloadu. Wymagane metadane protokołu Google/OpenRouter/Anthropic są przechowywane osobno w native chat_replay i nie są zwracane przez IPC/history/UI. Są powiązane ze ścisłym ScopeStamp: profil, połączenie, trasa i rewizja, model, dostawca, endpoint i rewizja klucza. Zmieniony stamp nie odtwarza starego replay, a zachowuje widoczny tekst historii. Replay jest walidowany przed zapisem odpowiedzi oraz przed ponownym użyciem. Nie dodano Serialize/Debug do publicznego response DTO. Jest to prywatna część lokalnego SQLite, nie zaszyfrowany magazyn; przyszły backup/eksport i diagnostyka muszą respektować ten zakres. Sekrety API nadal pozostają wyłącznie w OS vault.

Baza v4 → v5 dodaje chat_policies, chat_runs i chat_replay z profilowymi FK. Preferencje, profile, połączenia, historia, szkice, receipts i HMAC pozostają zachowane. Pending operations przy starcie dostają interrupted i bezpieczny błąd, bez wznowienia HTTP. Stare testy migracji v1/v2/v3 oczekują obecnie v5; oryginalne treści nadal są kontrolowane. Nowy test przygotowuje faktyczny stan v4 bez nowych tabel, sprawdza migrację, integralność i FK; osobny test odtwarza unfinished chat po restarcie.

Frozen P2 pozostaje niezmienione: chat.send/operation.get/cancel/message.created używają jego DTO. Dodatek contracts/chat.ipc.json definiuje tylko chat.settings.get/set i conversation.chat.latest. Końcowy chat result jest dostępny także po utracie zdarzeń. connection.test.latest filtruje kind=connection_test, aby nowe operacje chat nie udawały testu połączenia.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust Linux headless | 125/125 PASS: 68 unit/provider/vault, 15 chat, 24 connections, 10 conversations, 8 foundation |
| Frontend jsdom | 89/89 PASS, w tym 8 nowych ChatPanel; mock waliduje żądania według P2 i dodatków |
| TypeScript/Vite | PASS |
| Granice modułów | 20/20 PASS |
| Frozen P2 | 193/193 PASS |
| Overlay/manifest | Obie bazy + nakładka odtwarzają dokładny SOURCE; brak usunięć |

Rust 1.99.0: cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --locked --offline. Logi w docs/evidence/4b2-step-2. 18 nowych testów native: 15 chat + 3 replay. Obejmują sukces, history ordering, idempotency, współdzielony retry/licznik, brak cross-profile dostępu, CAS, attachments/context reject bez POST, anulowanie i late answer, szkic podczas generowania, zmianę klucza/modelu, atomowy rollback obu faz, restart i migration v4. Signed Google visible parts przechodzą parser → prywatny zapis/odczyt → request builder bez zmiany tekstu.

Nie użyto kluczy Edwarda ani płatnych API; native integration ma loopback i wstrzyknięty vault. Nie wykonano rzeczywistego OS vault/live API, Windows, desktop feature/Tauri/WebView ani oceny renderowania w prawdziwej przeglądarce. Nowe CI i audyt są potrzebne. Nie deklarujemy zamknięcia 4B-1/4B-2 ani wcześniejszych odbiorów desktopowych. Kolejny etap funkcjonalny po odbiorze czatu to 4B-2A: bezpieczny import plików/obrazów; pamięć i zaawansowany ContextAssembler pozostają późniejsze.
