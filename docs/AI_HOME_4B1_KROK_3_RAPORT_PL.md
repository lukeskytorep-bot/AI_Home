# AI Home — 4B-1 krok 3: vault, trwała konfiguracja i test połączenia

Wersja treści 1.0 · 12 października 2026 · Kandydat do istniejącego CI i audytu

**Wykonano konfigurator desktopowy, trwałe profile/połączenia/trasy, systemowy vault i trwały test dostępu do listy modeli. Całe 4B-1 pozostaje otwarte.** Rozmowy i pamięć agenta jeszcze nie działają. Test połączenia wykonuje GET models, nie płatne generowanie odpowiedzi i nie potwierdza możliwości konkretnego modelu.

## Podstawa

Edward potwierdził zielony wynik kroku 2 i upoważnił kolejny krok. Nie przekazano numeru przebiegu, zakresu platform ani nowego ZIP-a. Podstawa nakładki: AI_HOME_4B1_KROK_2_SOURCE.zip, SHA-256 `a0aac58ff49d67e5dd11ce88d8581017ca81ac6758ddd9a79255b064b692b47d`. Zakładamy zastosowanie całej poprzedniej paczki bez dodatkowych zmian. Nakładka zawiera tylko różnice i nie usuwa plików. Sprawdzono odtworzenie pełnego drzewa przez jej nałożenie na dokładną bazę.

## Co działa w kodzie

1. profile.create/list/snapshot: stabilne losowe UUID, wiele profili także o tej samej nazwie, domyślne pamięci P2 (L2 300 tokenów), Standard i preserve_source. Tworzenie i odczyt profilu wdrożono wcześniej jako zależność konfiguratora 4B-1; pełny zakres 4B-2 i rozmowy pozostają nierozpoczęte. profile.list ma trwały nieprzezroczysty kursor powiązany z zapytaniem i limitem. Profile nie są identyfikowane przez klucz, provider ani model.
2. connection.create/update/credential.set: endpoint jest własnością połączenia w profilu. Konfiguracja oraz zmiana klucza używają CAS i idempotency receipts. Zmiana ustawień lub klucza unieważnia attestation i cache modeli; zmiana providera/endpointu wymaga ponownego wpisania klucza. Połączenia mogą korzystać z tego samego dostawcy. Native lookup i composite foreign keys nie pozwalają użyć obcego połączenia/trasy/operacji.
3. Vault: zaadaptowano binding i fresh readback z Harness secrets.rs (MIT), z własnym service org.aihome.app.credentials.v1 oraz prefiksem ai-home-credential:v1:. Rekord wiąże credentialRef, profil, połączenie, provider, kanoniczny endpoint i credentialRevision. Brak legacy secret-only fallbacku, Debug sekretów ani ich serializacji do DTO IPC i komendy odczytu klucza do UI. Systemowe wywołania pracują na spawn_blocking. Zapis dostaje nowy immutable credentialRef; poprzedni działający wpis nie jest nadpisywany.
4. Windows: Credential Manager; macOS: Keychain; Linux: trwały Secret Service (crypto-rust, vendored), zamiast kernel-only magazynu Harness. Wybrano keyring =3.6.3. Zablokowany lub niedostępny vault zwraca bezpieczny błąd, bez fallbacku do SQLite/localStorage. Odczyt kontrolny jest wymagany przed zatwierdzeniem konfiguracji.
5. SQLite i keyring nie tworzą wspólnej transakcji. Dlatego użyto immutable refs: wpis vault → readback → transakcja metadanych/receipt/event. Nieudany commit zachowuje poprzednie połączenie i binding; nowy nieużywany wpis może pozostać w vault. Nie jest używany jako fallback. Stare wpisy są zachowane także dla prawidłowego replay receipts. Ich kontrolowany purge należy do późniejszego etapu danych, nie jest wykonywany automatycznie w tej paczce.
6. W SQLite, receipts i dzienniku nie zapisuje się klucza ani skrótu wyprowadzonego z klucza. Fingerprint idempotency obejmuje tylko metadane. Przy replay operacji zapisującej klucz podany sekret porównuje się natywnie z oryginalnym immutable wpisem vault; zmiana sekretu dla tego samego requestId daje IDEMPOTENCY_CONFLICT. Tożsamość profilu jest odrębna.
7. route.save/assign: osobne trasy Primary i Scribe, także na wspólnym połączeniu. Nowa trasa wymaga rewizji profilu, aktualizacja rewizji trasy, przypisanie ponownie rewizji profilu, zgodnie z P2. Trasa wymaga zweryfikowanego połączenia i poprawnego buildera protokołu. ReasoningValue bez potwierdzonej capability jest odrzucane. Zmiana klucza lub nieudany test przy aktywnych trasach przywraca profil do draft. ready oznacza kompletne przypisanie tras do zweryfikowanych połączeń, nie dowód działania czatu/modelu.
8. connection.test zwraca Accepted z queued Operation. Worker zapisuje running, odczytuje powiązany klucz i wykonuje natywne discovery z wspólnym CallBudget. Rezerwacje wywołań są commitowane przez CallRecorder przed HTTP; provider nie zależy od SQLite. Jak w kroku 2, jest to konserwatywny licznik rezerwacji — anulowanie może zużyć rezerwację bez wysłania bajtów. Końcowy stan/result/error oraz attestation/cache/event są zapisywane transakcyjnie. Stary wynik nie zmienia nowszej konfiguracji; STALE_CONNECTION_TEST pozostaje w starej operacji. Zmiana konfiguracji anuluje odpowiednie aktywne testy.
9. operation.get/cancel: wynik i safe error pozostają dostępne bez zdarzeń. Cancel jest scoped, idempotentny i terminalny; spóźniony worker go nie nadpisuje. Niepowodzenie zapisu workera próbuje zapisać STORAGE_ERROR; przy całkowicie niedostępnej bazie wynik pozostaje nieukończony do odzyskania przy restarcie. Otwarcie bazy zamienia queued/running w interrupted z OPERATION_INTERRUPTED i zapisuje zdarzenie, bez automatycznego ponawiania sieci.
10. Dziennik profilu ma własną sekwencję, oddzielną od app_events fundamentu. Mutacje, receipts i zdarzenia są w tej samej transakcji SQLite. Konfigurator używa snapshot i polling operation.get; nie dodano jeszcze streamu zdarzeń profilu. kernel.status.get nadal opisuje zgodny kontrakt fundamentu 4B-0. Brak zmian w uprawnieniach home_dispatch i brak SQL/sieci/keyringu w UI.

## Konfigurator EN/PL

Ekran pozwala utworzyć profil, dodać połączenie z kluczem, zmienić konfigurację/klucz, testować i anulować, pobrać wynik i wskazać modele Primary/Scribe. Lista modeli jest cache z ostatniego zgodnego testu; można również ręcznie wpisać ID. To nie jest attestation możliwości modelu. Pierwszy język nadal EN; zapisany wybór ma pierwszeństwo.

Klucz jest tylko chwilowym stanem formularza, trafia do istniejącego IPC i vault. Po udanym zapisie, zmianie profilu, połączenia lub providera pole jest czyszczone. Nie trafia do localStorage, logów ani diagnostyki. Szkic przetrwa zmianę języka i niejednoznaczny błąd transportu. Retry tej samej operacji formularza zachowuje requestId. Niezapisane ustawienia/klucz blokują testowanie i przypisanie modelu; podczas testu wynik pokazuje jego stan i testowane rewizje. Po restarcie ostatni test wybranego połączenia jest odczytywany z bazy.

Browser preview pozostaje jawnym podglądem; konfigurator wymaga desktopu i nie ma emulowanego zapisu kluczy ani fallbacku po błędzie Tauri. Conscious pozostaje niedostępny. Nie zmieniono przyjętych skórek.

## Kontrakty i dane

Kontrakty P2 oraz foundation pozostają bajtowo bez zmian. Dodatek contracts/connections.ipc.json opisuje dwa scoped odczyty: connection.models.list (cache zgodny z rewizjami) i connection.test.latest (ostatnia trwała operacja lub found=false), potrzebne do konfiguratora/restartu. P2 StateSnapshot nie został rozszerzony nieznanymi polami. Załączono fixtures i kontrole w nowych testach.

Baza lokalna zmienia user_version 1 → 2. Dodano profiles, connections, routes, operations, connection_models, profile_events i profile_cursors. Preferencje/receipts/app_events zachowane. SQL pozostaje w części natywnej. Test actual v1 → v2 potwierdza zachowanie PL/Aurora/dark, integrity_check=ok oraz pusty foreign_key_check. Baza z przyszłą wersją jest odrzucana. Nie są to migracje forum ani Harness; nie importowano ich baz.

## Wyniki

| Kontrola | Wynik |
|---|---|
| Rust Linux bez desktop feature, --all-targets --locked --offline | 70/70 PASS: 42 protokoły/transport/discovery, 20 nowych vault/config/operation/migration, 8 dotychczasowych foundation/SQLite |
| Frontend Vitest | 71/71 PASS: 63 poprzednie + 8 nowych testów konfiguratora |
| TypeScript i build | PASS |
| Granice modułów | 18/18 PASS |
| P2 | 193/193 PASS; kontrakty/snapshoty nienaruszone |
| Weryfikacja inwentarza, ZIP i odtworzenia nakładki | PASS; odnowione kompletne hashe aktualnego drzewa |
| OS vault Windows/Linux/macOS, native Tauri/WebView | NOT RUN runtime; skompilowano linuksowy adapter w headless build, testy używają wstrzykniętego magazynu |
| Rzeczywiste API/klucze | NOT RUN; testy HTTP używają loopback, bez płatnych wywołań |
| Renderowanie Chromium | NOT RUN: pobranie przeglądarki Playwright nie powiodło się; kontrola UI ma zakres jsdom, nie wizualnego WebView |

Testy odtwarzają m.in. osierocony nowy wpis po nieudanym SQL receipt, readback bez faktycznego zapisu, brak vault, obcy/legacy binding bez HTTP, zmianę endpointu, opóźniony wynik przy zmianie klucza, cancel, zapis reservation przed dispatch, restart queued operation, idempotencję z innym sekretem, query-bound cursor i composite foreign key isolation. Działający test listy modeli/route flow korzysta z natywnego HTTP oraz faktycznej SQLite.

Narzędzia: rustc 1.99.0. Command: cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --locked --offline. Pominięcie desktop feature oznacza, że wynik nie dowodzi kompilacji Tauri ani działania WebView. Frontend używa wcześniej zainstalowanych zależności przy identycznym package-lock.json; nie wykonywano nowego npm ci. CI wykona własną instalację.

Cargo.toml/Cargo.lock zawierają keyring 3.6.3 z właściwymi backendami i bezpośredni UUID v4. Lock wygenerowano przez Cargo na poprzednim lockfile; wszystkie wcześniejsze wersje pakietów zachowano. docs/evidence/4b1-step-3/dependency-change.json identyfikuje graf i hashe. Workflow oraz zależności JS są nienaruszone. Nie trzeba wykonywać Prepare Cargo lockfile ani tworzyć release/instalatora.

## Odbiór i następny krok

Wymagane nowe zwykłe CI dla kroku 3 i audyt. Następny krok 4B-1 to domknięcie wykonania retry/recovery na jednym budżecie i odbiór połączeń: rzeczywisty readback vault na wspieranych systemach, restart, nieudany test i rotacja klucza/endpointu, natywny WebView oraz uzgodniony live smoke. Otwarte Windows/WebView 4B-0 pozostają otwarte. Dopiero potem można zamknąć całe 4B-1 i przejść do 4B-2 rozmów. Nie opublikowano aplikacji i nie wywoływano forum.

Źródła implementacji: audytowana migawka Harness, docs/reuse/4B1_VAULT_REUSE.json i zachowany MIT notice. Dokumentacja platform: https://docs.rs/keyring/3.6.3/keyring/ oraz https://docs.rs/crate/keyring/3.6.3/source/README.md. Dokumentacja nie zastępuje testu prawdziwego magazynu.
