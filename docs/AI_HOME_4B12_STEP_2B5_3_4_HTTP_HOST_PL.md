# AI Home 4B-12 / 2B-5.3.4 — adapter hosta dla HTTP rejestracji

Data: 29.10.2026. Status IMPLEMENTED / AWAITING CI AND AUDIT. Całe 2B-5.3, 2B i 4B-12 pozostają OPEN. Baza: AI_HOME_4B12_STEP_2B5_3_3_AUDIT_FIX_1_SOURCE.zip, SHA-256 2891cd280b883854a1bd922af3b744402712e7b22a197eec6c0d359319174b59. Edward potwierdził zielone CI; nie pobierano niezależnie numeru zewnętrznego runu.

## Zakres

CommunityHost::dispatch_registration łączy istniejący protokół RegistrationStart/RegistrationFinish z natywną autoryzacją profilu i trwałym claimem Main. Dodano cztery komendy supplemental:

| Komenda | Capability | Payload JSON |
|---|---|---|
| community.registration.start | Start | registrationRevision |
| community.registration.complete | Complete | registrationRevision |
| community.registration.complete.retry | RetryComplete | finishRevision |
| community.registration.access.recover | RecoverAccess | finishRevision |

expectedRevision w envelope pozostaje rewizją profilu Main. registrationRevision jest rewizją lokalnego przygotowania; start/complete wymagają Ready i zgodnej rewizji. finishRevision dotyczy dziennika completion/recovery. Capabilities są odrębne; przyznanie jednej nie przyznaje pozostałych.

Zaproszenie dla start jest opcjonalnym polem natywnego RegistrationAccess, typu Zeroizing<String>, poza JSON. Dla start musi być obecne i poprawne; dla pozostałych komend musi być nieobecne. Grant nie jest deserializowalny ani Debug. Scope i docelowy HTTPS root pochodzą z natywnego hosta, nie z payloadu. Konstrukcja klienta nie wysyła żądań.

## Autoryzacja i trwałość

Wszystkie nowe operacje przechodzą przez te same kontrole aktualnego profilu, rewizji, Standard, archived, autorytatywnego Experimental binding, deletion intent, registry lifecycle/fence i host binding co lokalny adapter. Grant jest sprawdzany również przed replay. dispatch_local nie może uruchomić HTTP nawet z capability sieciowym; dispatch_registration nie przyjmuje lokalnych komend.

Przed rejestrem/vault/HTTP Main zapisuje pin, requestId i fingerprint. Fingerprint obejmuje command, profil, rewizję, payload, dokładny scope i skrót zaproszenia. Zaproszenie i token nie są zapisywane w Main. Format fingerprintu istniejących lokalnych komend pozostał zgodny z bazą.

Druga transakcja ponownie sprawdza profil i utrzymuje Main write reservation przez rejestrację. Osobny Store nie może wtedy zmienić kierunku/archiwizacji. Wspólny gate hosta zatrzymuje maintenance do końca operacji i pozostaje własnością workera po anulowaniu future klienta. Timeout HTTP to 10 s na istniejącą wymianę; odzyskiwanie składa się z kilku jawnych wymian. Pełne wykonanie może utrzymywać Main write reservation dłużej niż jeden timeout.

Niedokończony claim HTTP nie jest automatycznie wykonywany drugi raz: zwraca REGISTRATION_RECONCILIATION_REQUIRED. Wymagane jest nowe jawne żądanie. Przy complete.retry istniejący protokół odtwarza oryginalne podpisane bajty, a access.recover stosuje proof/status/reissue i fencing generacji tokena. Nie powstaje zastępczy zestaw kluczy.

Awaria zapisu końcowego receipt pozostawia trwały claim. Test wywołuje rzeczywisty błąd SQLite po udanym POST, restartuje host i potwierdza brak kolejnego POST pod tym samym ID. Nowy complete może bezpiecznie odczytać istniejące poświadczenie bez ponownej publikacji. Receipt sukcesu i błędu ma ścisły sprawdzany format; replay jest historycznym wynikiem, nie potwierdzeniem bieżącej użyteczności tokena.

## Wyniki i sekrety

Wynik ma dokładnie phase, finishRevision i onlineStatusKnown=false. Start zwraca awaiting_complete albo recovery_required; completion/retry/recover zwracają credential_available dopiero po powodzeniu bazowego protokołu. Nie projektujemy tokena, key reference, registration ID, podpisów ani odpowiedzi serwera. Bieżący lokalny dostęp należy odczytać przez access.status; przyszły zapis forum ponownie przeprowadzi autoryzację i odczyt credential.

Błędy sieciowe są bezpiecznie ograniczone do lokalnych kodów, bez raw server message, URL lub sekretów. retryable pozostaje false: ponowienie jest jawne. Niepewny lub wadliwy wynik protokołu wymaga reconciliation; uszkodzenie storage nadal daje STORAGE_ERROR.

## Testy i odbiór

Dziewięć nowych testów hosta używa realnego SQLite, testowych key/token stores oraz loopback HTTP. Obejmują start z zaproszeniem i replay, podpisane complete, proof recovery/generation, utraconą odpowiedź complete i jawny retry/recovery, oddzielne capability/scope/revision/direction, nieprawidłowy payload/invite, lifecycle reconcile/deletion, uszkodzony receipt, błąd finalnego commitu oraz anulowanie klienta z zachowaniem Main lock i maintenance gate.

Wyniki końcowe: Rust/Linux headless 646/646 w 23 zestawach; nowe testy 9/9; frontend 219/219, build PASS; P2 193/193; granice 34/34; migration assertions 14; command parity 63/63; dotychczasowe verify PASS. Logi i hashe znajdują się w docs/evidence/4b12-step-2b5-3-4/.

Rust: --all-targets --no-default-features --locked --offline. Nie odebrano produkcyjnego desktopu, Windows/WebView, systemowego vault, live forum/PHP/TLS ani fizycznego power-loss. Dodatkowe seam testowe loopback są dostępne wyłącznie z cfg(test). Istniejący mock serwera rozszerzono o start; produkcyjny protokół transportu i podpisów jest niezmieniony.

## Przekazanie i dalsze prace

Nakładkę nałożyć na dokładną zieloną bazę 2B-5.3.3 Audit Fix 1. Zwykły commit/push, aktualne CI i audyt; bez release/instalatorów. Store v16, registry v4, dependencies, Cargo.lock, frontend, Tauri permissions i workflow bez zmian. Produkcyjny setup Tauri nie wywołuje jeszcze nowej ścieżki.

Zsynchronizowano bieżący plan 1.89 z repozytorium i dodano aktualizację 1.90. Historia i hashe wcześniejszych ZIP-ów pozostają zachowane. Najbliższa dalsza praca: świadome desktop routing/authorization i odbiór rejestracji, później signed writes/feed. MCP/skills i druga społeczność pozostają planem; przed implementacją MCP obowiązuje przegląd rozwiązań open source z treści 1.89. Przeglądarka jest odłożona.
