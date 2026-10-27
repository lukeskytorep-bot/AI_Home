# AI Home — 4B-12 krok 2B-1 — trwały rejestr przygotowania rejestracji

Data: 27.10.2026. Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 2B i 4B-12 OPEN. Plan treść 1.76.

Baza: `AI_HOME_4B12_STEP_2A_SOURCE.zip`, SHA-256 `2f13c3d0aba0ad610b0a9d8eb78f4218ebe524a203de0defb51e72aff95d1f78`, 747 plików. Edward potwierdził zielony wynik; nie pobierano nowego przebiegu GitHub ani audytu. Protokół forum i kryptografia pozostają z odebranej bazy 2A.

## Wynik

Dodano `community_identity::registry::RegistrationRegistry`: SQLite, nie fikcyjny stan w pamięci. Rejestr przechowuje oryginalny client_start_id, dokładny profil publiczny, pełny zakres local profile/API root/pinned origin/instance, zarezerwowaną referencję kluczy, stan, rewizję i po utworzeniu kluczy oba oczekiwane public keys. Idempotentne powtórzenie wejścia zwraca ten sam wpis; zmieniony profil/start ID lub przeniesienie start ID do innego scope kończy się konfliktem.

`prepare` nie tworzy kluczy. `provision` najpierw wykonuje atomowy UPDATE z CAS (stan reserved + rewizja), zatwierdza claim i dopiero potem wywołuje istniejący vault 2A. Dwa niezależne poole nie uzyskują jednocześnie prawa do generacji pod tą samą referencją. Owner marker jest prywatny dla modułu. Po sukcesie CAS utrwala oba public keys i stan Ready.

Claim nie ma automatycznego timeoutu/przejęcia. Anulowanie async nie zatrzymuje spawn_blocking OS keyring; wygasająca blokada pozwoliłaby drugiej instancji ścigać się z pierwszym writerem. Każdy niejednoznaczny wynik pozostawia trwałe Provisioning. `recover` tylko ładuje wcześniej zarezerwowany wpis, niczego nie generuje, nie nadpisuje i nie usuwa. Istniejące poprawne klucze mogą być zatwierdzone przez CAS. Brak/unavailable pozostawia stan bez zmian. Crash przed zapisem może wymagać ręcznego uzgodnienia przez przyszły host; nie ma teraz komendy reset/cleanup i nie należy edytować SQLite.

`load_intent` wymaga Ready, ponownie ładuje faktyczne klucze i porównuje je z utrwalonym publicznym oczekiwaniem, po czym rekonstruuje oryginalny RegistrationIntent. Zmienione klucze lub brak po odtworzeniu bazy blokują dalsze przygotowanie. Odtworzone start_request ma te same bajty przy tym samym invite. **Ready to lokalna gotowość kluczy, nie receipt rejestracji online.**

## Zakres danych i integracji

Rejestr otwiera jawnie podaną osobną bazę SQLite WAL z własnym application ID 1095254851 / schema v1. Nie zmienia Store v15, nie przyjmuje istniejącej bazy aplikacji ani obcej/przyszłej wersji. Host musi w przyszłości dostarczyć jedną stabilną chronioną ścieżkę. Inna/pusta ścieżka nie jest sposobem odzyskiwania starej próby. Oddzielny rejestr jest odseparowany od migracji Store, ale przed udostępnieniem rejestracji musi zostać objęty wspólnym lifecycle i backup/restore.

To metadane treści publicznego profilu, nie diagnostyka bez treści. Invite, prywatne PKCS8, bearer token i request body nie trafiają do SQLite. Sekrety pozostają w oddzielnym namespace Community z 2A, bez fallbacku do sekretów modeli.

Nie ma HTTP, przyjęcia tokenu, nowych DTO/IPC/UI, automatycznej rejestracji ani uruchamiania schedulera. Produkcyjna aplikacja nie otwiera jeszcze rejestru ani nie wywołuje go przy tworzeniu profilu. Nie sprawdza on obecności/kierunku/capabilities profilu w Store — to obowiązek przyszłego hosta. Nie jest jeszcze wpięty w profile.delete.permanent, backup/restore ani cleanup. Przed włączeniem użytkownikowi te powiązania są obowiązkowe. Surowy CommunityKeyStore jest niskopoziomowym API wymagającym caller exclusivity; przyszły flow nie może nim omijać rejestru.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless all-targets, locked/offline | **474/474 PASS**, 22 zestawy |
| Nowe testy rejestru (w powyższym przebiegu) | **18/18 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| Frozen P2 reference | **193/193 PASS** |
| Granice modułów / aktualne asercje migracyjne | **34/34 PASS**, 13 asercji v15 |
| Command parity | **54/54 PASS** |
| Standard closure / release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Community reader / identity static guards | **14/14 i 14/14 PASS** |
| Rust source-text | **PASS** |
| Manifest, ZIP, dokładne baza + nakładka = SOURCE | **PASS** — osobne pliki w pakiecie |

Wszystkie dotychczasowe testy są zachowane bajtowo. Nowe 18 testów obejmuje trwałość rezerwacji i pakietu, dokładną idempotencję, konflikty, izolację profili/endpointów, walidację ID, dwóch konkurentów z niezależnymi połączeniami, CAS, readback failure, przerwanie przed zapisem, anulowanie po zapisie przy działającym spawn_blocking, brak kluczy i zmienione publiczne oczekiwanie po restore, korupcję metadanych, odrzucenie obcej/przyszłej bazy i brak sekretów w plikach SQLite/WAL.

Testy bazują na prawdziwych tymczasowych plikach SQLite i mocku sekretów. Close/reopen nie jest testem restartu procesu OS ani rzeczywistego keyringu. Test anulowania używa sygnału o zapisie i kontrolowanego blokującego writera, zamiast wnioskować z krótkiego sleep. Nie uruchomiono nowego OpenSSL CLI; istniejące fixture/testy kryptografii 2A przechodzą ponownie jako część Rust. Historyczne dowody OpenSSL są w zachowanej bazie 2A.

W lokalnym przebiegu pozostaje wcześniejszy warning unused InternalStateProvider w tests/experimental_internal_state.rs; plik nie był zmieniany. Real OS vault, live forum/PHP, API modeli, Windows/Tauri/WebView i nowy GitHub CI nie zostały wykonane lokalnie. Ich odbiór pozostaje oddzielny.

## Przekazanie i następny zakres

Nakładka bez usunięć jest policzona na dokładnej bazie 2A. Audyt zawiera bazę, nakładkę, wynikowy SOURCE, plan, raport, developer guide, logi i dowód zgodności. Cargo.toml, Cargo.lock, package lock, workflow, provider vault/adapters, frozen P2, uprawnienia i stare testy są niezmienione. Dotychczasowy job cargo --all-targets wykryje nowe testy bez zmiany CI. Nie ma release/instalatorów.

Kolejne części 2B: trwały start/complete i HTTPS; proof-status oraz reissue utraconego jednorazowego tokenu; osobny vault tokenów; lifecycle profili, backup/restore, kontrola uprawnień i IPC przed włączeniem użytkownikowi. Dalej podpisane zapisy i feed w 4B-12. Niniejszy pakiet nie zamyka 2B ani całego 4B-12.
