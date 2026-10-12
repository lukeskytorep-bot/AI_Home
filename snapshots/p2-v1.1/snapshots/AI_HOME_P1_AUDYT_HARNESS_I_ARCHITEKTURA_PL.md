# AI Home — P1: ponowne użycie Harness i architektura

11 października 2026 · Wersja 1.0 · Audyt przygotowawczy wykonany; przeniesienie kodu i odbiór runtime pozostają w etapach implementacji.

## Wynik i decyzja

Budujemy AI Home na **Tauri 2 + React/TypeScript + Rust + SQLite**, z własną bazą, identyfikatorami profili, przestrzenią sekretów i modułami domenowymi. To decyzja wykonawcza w ramach upoważnienia Edwarda do kontynuacji projektu, nie deklaracja istniejącego programu. Najważniejszy powód: rzeczywiste moduły dostawców i importerów Harness są już napisane w Rust/TypeScript; pełny runtime Python oznaczałby ich przepisanie albo dodatkowy most od pierwszej wersji.

Zachowujemy granice funkcjonalne planu 4B v1.3. Zmieniamy proponowane w nim drzewo plików Python na strukturę odpowiednią dla Tauri. ConsciousCore pozostaje opcjonalny za `InternalStateProvider`; Standard korzysta z NullState i nie wymaga Pythona. Jeśli dostarczony silnik ConsciousCore ma pozostać w Pythonie, późniejszy adapter będzie osobnym procesem z wersjonowanym protokołem i pakietem dla każdej platformy. Nie rozpoczynamy teraz jego portowania ani nie kopiujemy prototypowego event → speech.

Edward przyjął wygląd P0 i pozwolił przejść dalej. Uwagi audytora zostały zachowane i poprawione w makiecie. Domyślnym językiem nowej instalacji jest **angielski**; polski pozostaje wyborem użytkownika. Po zapisaniu preferencji aplikacja ma ją respektować przy następnych uruchomieniach.

## Źródło audytu

| Właściwość | Ustalenie |
|---|---|
| Migawka | `AI-RV-Harness-ai-rv-2.zip`, udostępniona 11.10.2026 |
| SHA-256 archiwum | `534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e` |
| Komentarz ZIP | `2cb5bba5b857f808851388e8078127e37389434b` — identyfikator zapisany w archiwum; nie zweryfikowano go zdalnie jako commit |
| Deklarowana wersja | `package.json` i `Cargo.toml`: 0.7.13; obecność dokumentów/skryptów v0.7.14 nie zmienia tej deklaracji |
| Integralność | CRC wszystkich wpisów poprawne; 1075 plików źródłowych zgodnych bajtowo z archiwum |
| Digest drzewa | `7044d8e5435d8fd15d0c5537028b302ed96e597cee9d067d4a265ccc402b0cc7` |
| Inwentarz kandydatów | 96 plików: metadane, źródła modułów, testy i fixtures; SHA-256 każdego oraz importy TS w pakiecie dowodów |
| Baseline AI Home | Nie dostarczono repozytorium aplikacji. Istnieją plan i makieta; 4B-0 nie jest wykonane |

Digest drzewa: SHA-256 konkatenacji posortowanych wierszy `sha256_pliku`, dwie spacje, względna ścieżka, LF. Audyt dotyczy tej migawki, nie dowolnej późniejszej gałęzi Harness. Nie ogłaszamy wyników GitHub Actions dla AI Home ani pełnej aktualności upstream. Nowa migawka wymaga porównania zmian przed importem.

## Mapa przeniesienia

Ścieżki są względne względem katalogu źródłowego Harness. „Adaptować” oznacza zachować użyteczne rozwiązanie i testy, zmieniając granice produktu; nie wkleić całego pliku wraz z jego zależnościami.

| Źródło | Co wykorzystać | Zależności i adaptacja | Etap i testy |
|---|---|---|---|
| `src-tauri/src/providers.rs`, `providers/adapters.rs`, `request_builders.rs`, `response_parsers.rs`, `validation.rs` | Rodziny OpenAI-compatible, Google, Anthropic; konstrukcja żądań, discovery i parsowanie | reqwest, serde/serde_json, secrets, Channel; nowa fasada AI Home, nazwa produktu i nagłówki OpenRouter | 4B-1; wybrane testy z `providers/tests.rs`, kontrakty czterech podstawowych providerów |
| `providers/transport.rs`, `errors.rs`, `streaming.rs`, `endpoint_capabilities.rs` | Ograniczony transport, klasyfikacja błędów, anulowanie, SSE i metadata endpointów | tokio, futures-util, limity buforów; zachować brak automatycznej powtórki po rozpoczęciu semantycznej odpowiedzi | 4B-1; Rust transport/SSE, TS `requestExecutor.test.ts`, `retry.test.ts`, testy streaming |
| `src/providers/types.ts`, `capabilities.ts`, `modelReasoningRegistry.ts`, `capabilitySnapshot.ts` | Jawne możliwości modeli, normalizacja discovery i obsługiwane parametry | Registry wymaga aktualizacji według rzeczywistych API; nie kopiować rekomendacji modeli RV ani narzucać reasoning wszystkim modelom | 4B-1; `capabilities.test.ts`, `modelReasoningRegistry.test.ts`, `capabilitySnapshot.test.ts` |
| `src/providers/continuationContract.ts`, `openRouterContinuation.ts`, `googleContinuation.ts`, `anthropicContinuation.ts` i fixtures | Opaque continuation state, walidacja rozmiaru/formatu i zgodności replay z trasą | Zachować oddzielenie od widocznej rozmowy; zmiana modelu zachowuje profil, ale nie upoważnia do replay niezgodnego stanu | 4B-1/2/5; testy continuation oraz most zgodności TS/Rust |
| `src/providers/retry.ts`, `providerError.ts`, `requestExecutor.ts` | Jedno miejsce wykonywania fizycznych prób i wynik operacji | requestExecutor importuje native/envelope/operation profiles; przeniesienie wymaga zamknięcia zależności i limitu całej operacji | 4B-1/3; testy retry/executor, anulowanie w backoff i po pierwszym fragmencie |
| `effectiveRequestEnvelope.ts`, `inputTokenEstimate.ts`, `operationResourceProfiles.ts`, `streamingPolicy.ts`, `streamPresentation.ts`, `outputRecovery.ts` | Rozdzielenie budżetu wejścia, wyjścia, streamu i recovery | Lista operacji jest RV-specific: zastąpić chat, scribe proposal, primary decision, retrieval, później reflection. Nie przenosić budżetów Training jako wartości domyślnych Home | 4B-1/3/5; testy envelope, outputRecovery i operation profiles po zmianie domeny |
| `src-tauri/src/secrets.rs` oraz komendy provider binding | Keyring, zapis i odczyt kontrolny, związanie sekretu z providerem i endpointem, rozdzielenie store/rebind | Nowy SERVICE_NAME i prefiks rekordów; nie odczytywać sekretów Harness. Fingerprint klucza nie jest ID profilu AI Home | 4B-1; testy binding, legacy i readback; runtime keyring Windows/Linux |
| `src/providers/native.ts`, `service.ts` | Most native, test/discovery połączeń, obsługa błędów, rebind | Odciąć import `../storage` i AppRepository RV; wprowadzić wąski ConnectionRepository/RuntimePort. Unieważniać wynik testu po zmianie provider/key/endpoint | 4B-1/2; `service.test.ts`, `credentialBindingService.test.ts`, `nativeCredentialStorage.test.ts` |
| `src-tauri/src/documents.rs`, `dialogs.rs`; `src/attachments/native.ts` | Parser TXT/MD/DOCX/tekstowego PDF, obrazy, limity plików i natywny wybór | lopdf, zip, roxmltree, image, base64, sha2; wydzielić parsery i usunąć wbudowane leksykony/protokoły. Osobno zbudować przechowanie oryginału i pochodzenie | 4B-2A; istniejące testy parserów plus nowe DOCX/PDF i metadane importu |
| `src/sources/service.ts` | Hash treści, opis metody importu, traktowanie źródła jako danych | WorkspaceSource i workspaceId są domeną Harness; zastąpić ImportedMaterial, profileId, conversation scope. Rozdzielić hash oryginalnych bajtów i hash wydobytego tekstu | 4B-2A/4; autor/importer, cztery przeznaczenia, brak cichego obcięcia |
| `src/aiCenter/fieldGuideUpdate.ts`, `viewerNotes.ts`, `fieldGuide.ts`, `baseVersion.ts` | Wzorzec pojemności, druga próba, NO_CHANGE, historia, kontrola nieaktualnej bazy i źródeł | Silne zależności od Training, Reveal, leksykonu, AiIdentity, credentialFingerprint i AppRepository. Wyciągnąć ogólną logikę; napisać osobne prompty pamięci Home i decyzję Primary | 4B-3/4; adaptować testy capacity retry, FAILED_CAPACITY, STALE_BASE, provenance i wersji |
| `src/features/profiles/useProfileSetupController.ts`, `ProfileSetupUnifiedController.test.ts` | Wzorzec wspólnego kontrolera konfiguracji i resetu wyborów połączenia | Usunąć Viewer/Judge/Monitor, factory prompts, zależność od etykiety Home do rozpoznania języka. Primary i Scribe mają odrębne ModelRoute | 4B-2; kontroler, utrata szkicu, unieważnianie testu i wiele profili |
| `src/i18n.ts`, `src/i18n/*` oraz CSS/theme conventions | Typowane klucze, słowniki i zmienne wizualne | Własne teksty Home, domyślny en, zapis wyboru, fallback i format pakietów w P2; nie kopiować całego słownika RV ani całej skórki | 4B-0/2; kompletność kluczy, fallback, niezmienianie języka materiałów |

## Konkretne różnice wymagające adaptacji

1. **Tożsamość.** Viewer Notes wiążą AiIdentity z fingerprintem credential/model route. AI Home dostaje trwały `profileId` niezależny od tego powiązania. Rotacja klucza i zmiana modelu zachowują dane; odrębne modele Primary/Scribe nie tworzą dwóch tożsamości. Ślady trasy nadal mogą służyć walidacji continuation i audytowi wywołania.
2. **Repozytoria i IPC.** Moduły TS importują AppRepository i wykrywanie Tauri z warstwy storage. Przenosimy je na małe porty. Native odpowiada za autorytatywną walidację zapisów i skutków; zmiana UI nie może omijać limitów. Nie kopiujemy bazy `rv_harness.db`, migracji, plugin ACL ani komend SQL z całym zakresem Harness. Home otrzyma własną rejestrację migracji i ograniczone komendy domenowe.
3. **Dokumenty.** ParsedDocumentSource zwraca wydobyty tekst i jego hash, nie pełny trwały magazyn oryginałów. Home musi zachować oryginalne bajty, hash, autora, importera, datę i zakres. W źródle wybór plików jest ograniczony przez `.take(12)` — Home ma jawnie zgłosić przekroczenie liczby zamiast pomijać nadmiar bez informacji. Skan PDF bez czytelnego tekstu nie otrzymuje obietnicy OCR.
4. **Pojemność.** Viewer Notes mają domyślnie 1024 i zamknięty zestaw pojemności. Home potrzebuje 300 dla L2, wartości własnej i jawnej polityki unlimited. Konserwatywny pomiar z Harness jest estymacją; nie opisywać go jako dokładnego tokenizera. Unlimited wyłącza limit długości zapisu, a nie limit wyjścia, budżet wywołania czy walidację formatu.
5. **Powtórzenia.** Istnieją osobno retry transportu, naprawa JSON, odzyskanie odpowiedzi po limicie wyjścia i retry długości pamięci. Dwie próby semantyczne nie oznaczają dwóch fizycznych żądań HTTP. Home wprowadzi osobne liczniki i wspólny budżet czasu/wywołań/kosztu, bez zagnieżdżonych nieograniczonych pętli.
6. **Prywatność diagnostyki.** `debug.ts` może zbierać request/response przy detailed diagnostics. Przeniesienie wymaga polityki redakcji; domyślny eksport diagnostyki nie zawiera kluczy, dokumentów, pamięci ani prywatnego continuation. Nie wystarczy zamiana nazwy produktu.
7. **Stałe wskazówki.** Użytkownik włącza i wyłącza materiał; widzi co zostanie dołączone. Dokument dostarczony przez człowieka nie staje się automatycznie system promptem ani uprawnieniem. Import propozycji pamięci kończy się decyzją agenta ACCEPT/MODIFY/REJECT, a nie podpisaniem ludzkiego kliknięcia jako rzeczywistej decyzji modelu.

## Licencje i pakiety

Kod aplikacji Harness: MIT, zachować notice i oznaczenia źródła/adaptacji. Dokumentacja, protokoły, factory prompts, cele treningowe i niekodowe zasoby: osobno CC BY 4.0 zgodnie z `CONTENT_LICENSE_CC_BY_4.0.md`. Do AI Home nie importujemy pakietów RV, leksykonów, factory prompts ani targetów. Nowe ogólne prompty Home będą własnymi materiałami projektu. Dostawcy bibliotek mają odrębne licencje; lockfile jest wejściem do późniejszego zestawienia third-party notices, nie dowodem audytu całego łańcucha zależności.

Zależności do zamknięcia przy portowaniu: Rust — Tauri, reqwest/rustls, tokio, futures-util, serde, serde_json, sha2, keyring; import — zip, roxmltree, lopdf, image, base64 i native dialog. UI — React, React DOM, Tauri API, Vite/TypeScript; markdown renderer tylko gdy potrzebny. SQLite przez ograniczone native repozytoria. Dokładne wersje i zgodność lockfile ustalamy w 4B-0/1; nie aktualizujemy ich zbiorczo w audycie.

## Architektura i koszt wyboru

| Wariant | Ocena dla tego projektu |
|---|---|
| Tauri + React/TS + Rust | Wybrany. Największy zakres ponownego użycia sprawdzonych źródeł provider/import; koszt to utrzymanie dwóch języków, IPC i własnej domeny Home |
| Pełna aplikacja Python | Odpowiada przykładowemu drzewu v1.3 i upraszcza integrację Python ConsciousCore, ale wymaga przepisania Rust infrastruktury lub mostu od początku; brak wskazanego gotowego UI |
| Tauri + Python jako główny runtime od początku | Zachowuje UI i część native kodu, lecz dodaje trzeci język, proces i dystrybucję zanim Standard tego potrzebuje; nie wybrany |

Docelowe granice: `UI` wysyła komendy i prezentuje zdarzenia; `Application` koordynuje przypadki użycia; `Domain` opisuje profile, rozmowy i pamięć; `Kernel` zarządza lifecycle/event bus; `Infrastructure` realizuje provider, bazę, klucze i pliki. Provider-specific i forum-specific logika pozostaje poza kernelem. Pure TS helpers mogą zostać wykorzystane przy budowaniu żądań, ale trwały stan, walidacja i działania nie są własnością komponentów React. P2 zamrozi dokładny podział komend, zdarzeń i kontraktów.

**ConsciousCore:** adapter bez bezpośredniej sieci i dostępu do kluczy; snapshot/tick/event zwraca dane i wake candidate, nie samodzielną publikację. Executive i Policy rozpatrują działanie. Tauri dokumentuje dołączanie zewnętrznego procesu z binariami dla konkretnych targetów; to potwierdza drogę integracji, ale nie dowodzi gotowej dystrybucji naszego silnika. Nie dostarczono jego pięciu referencyjnych plików w badanych wejściach P1; ich implementacja, zależności i pakowanie pozostają kontrolą przed 4B-10. Nie blokuje to Standard ani P2.

Źródła dokumentacji frameworka, odczytane 11.10.2026: [Embedding External Binaries](https://v2.tauri.app/develop/sidecar/), [Capabilities](https://v2.tauri.app/security/capabilities/). Wnioski o opłacalności stosu są decyzją projektową na podstawie zbadanych źródeł, nie wynikiem benchmarku.

## Kontrole i ich granice

| Kontrola wykonana | Wynik |
|---|---|
| Integralność ZIP i zgodność rozpakowanego drzewa | PASS, 1075 plików |
| Inwentarz kandydatów i importów | 96 plików z SHA-256; brak AGENTS.md w badanym źródle |
| Składnia i regresje poprawionej makiety | PASS, 62 asercje logiki z zastępczym DOM |
| Oryginalne pure functions Harness: retry i continuation | PASS, 43 asercje; źródła TS transformowane tymczasowo przez Node 24 bez modyfikacji logiki |
| Native/Rust, pełny Vitest/typecheck/build | NIEWYKONANE: brak Rust/Cargo i zainstalowanych zależności projektu. To obowiązkowe kontrole po portowaniu, nie PASS z audytu |
| Keyring i rzeczywiste modele | NIEWYKONANE; P1 nie wykorzystuje kluczy ani płatnych połączeń |
| Renderowanie w przeglądarce/WebView | NIEWYKONANE niezależnie; Edward przyjął kierunek wizualny. Kontrola rzeczywistego UI zostaje przy pierwszym działającym interfejsie |

43 asercje sprawdzają transient/permanent HTTP retry, zakaz retry po semantic output, Retry-After cap, poprawność trzech istniejących fixtures continuation, odrzucenie replay po zmianie credential/model oraz zgodność visible content Google. Nie jest to uruchomienie istniejącego całego zestawu Vitest i nie potwierdza działających endpointów. Pakiet `AI_HOME_P1_DOWODY.zip` zawiera skrypty, inwentarz i logi; nie zawiera kopii całego Harness.

## Zamknięcie P0 i kolejny krok

W P0 poprawiono: domyślny EN; szkice osobne dla rozmów; rzeczywisty wynik ostatniej operacji pamięci; wycofanie z bocznego kontekstu; dwa niezależne demo modele Scribe z zapisem wyboru w profilu; unieważnienie testu po zmianie połączenia; symulacje ACCEPT/MODIFY/REJECT; włączanie stałych wskazówek i listę dla następnego wywołania; oznaczenie niedostępnego Conscious; usunięcie stałego paska 74%. Te zmiany nadal są demonstracją.

P0 przyjęto jako projekt ekranów na podstawie decyzji Edwarda; nie jako odbiór funkcjonalnej aplikacji. P1 zakończono jako audyt przygotowawczy i decyzję technologiczną, z wyżej wskazanymi ograniczeniami testów. Następny etap **P2**: kontrakty profilu/połączeń/modeli, pamięci/importu, wersji i prób; IPC, języki/skórki oraz aktualizacja granic modułów dla wybranego stosu. Następnie 4B-0 utworzy repozytorium i uruchamialny fundament. Forum 4A pozostaje odebrane i opcjonalne.
