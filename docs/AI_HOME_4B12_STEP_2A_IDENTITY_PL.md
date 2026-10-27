# AI Home — 4B-12 krok 2A — klucze Community i pakiety rejestracji

Data: 27.10.2026. Status: IMPLEMENTED / AWAITING CI AND AUDIT. Cały krok 2 i całe 4B-12 pozostają otwarte.
Baza: zielony według Edwarda 4B12 STEP 1 AUDIT FIX 1 SOURCE, SHA-256 `ba55a869fb069cdea1893072e21e35189ee342a220aff8d1b52a60471840bd83`, 691 plików. Plan treść 1.75.

## Wynik i zakres 2A

Dodano natywny moduł community_identity: kryptografię rzeczywistych dwóch niezależnych kluczy P-256, publiczne handle, prywatny magazyn, canonical profile/registration statement, walidację odpowiedzi start i tworzenie podpisanego complete packet. Nie ma HTTP, tokenów, SQLite, poleceń IPC lub UI. Jest to przygotowanie rejestracji i infrastruktura kluczy, nie ukończona rejestracja. Pełna koordynacja sieciowa i trwałe recovery stanowią 2B.

Operacyjny i tożsamościowy klucz muszą mieć różne punkty krzywej, nie tylko różny zapis DER/base64. Fingerprint jest identyczny z 4A: SHA-256 z P256-POINT/1, NUL i współrzędnych x/y. Public keys to SPKI DER base64url bez paddingu; signatures to ASN.1 DER ECDSA/SHA-256 base64url. Akceptujemy canonical SPKI z punktem skompresowanym lub nieskompresowanym, tak jak OpenSSL 4A. Trailing DER, niekanoniczny base64 i P-384 są odrzucane. Kryptografia jest realizowana przez RustCrypto p256, nie przez własne operacje ECC; własny wrapper SPKI jest stały dla zamrożonego P-256 suite i sprawdzany fixture'ami. Generacja korzysta z RNG systemowego, podpis jest deterministyczny RFC6979.

RegistrationProfile implementuje dokładne COMMUNITY-PROFILE/1: ASCII trim PHP, UTF-8 bytes (120 display_name / 2048 about), brak normalizacji Unicode, jawny base64url i końcowe LF. Wygenerowany start request zachowuje client_start_id i public keys, bez nowej tożsamości przy retry; ma limit 8192 bajtów całej serializacji, także po JSON escaping.

Challenge waliduje lokalny scope, profile hash, suite/formaty/protocol version, registration ID, canonical nonce 32-byte, datę UTC, expiry i maksymalny TTL 24 h. Statement jest konstruowany lokalnie ze znanych kluczy i porównywany bajtowo z odpowiedzią; nie podpisujemy dowolnego tekstu serwera. Podpisywanie ponownie sprawdza expiry oraz zgodność rzeczywiście odczytanych private keys z public keys zaakceptowanego challenge. Rejestracja committed zwraca RecoveryRequired zamiast przyjmować nieistniejący token lub udawać ponowne pobranie tokenu.

## Magazyn i granice trwałości

CommunityKeyStore używa odrębnego service `org.aihome.app.community.identity.v1` i referencji `ai-home-community-keyset:v1:<random128bit>`. Nie odczytuje namespace dostawców ani ich starych sekretów. reserve_reference() pozwala przygotować ID przed side effect; provision() jawnie tworzy obie pary w jednym wpisie, sprawdza świeży readback i odmawia nadpisania istniejącego wpisu. load() i podpisywanie nigdy nie generują kluczy zastępczych. remove() jest jawne, sprawdza scope i jest idempotentne przy braku wpisu. Model/provider nie uczestniczy w zakresie kluczy.

Backend OS keyring jest blokujący, więc operacje wykonujemy przez spawn_blocking. Prywatne klucze PKCS8 i invite request używają Zeroizing; typy sekretne nie mają Debug/Display/Serialize, a publiczne handle/pakiety nie zawierają private keys. To ogranicza czas życia buforów w aplikacji, nie obiecuje wymazania wszystkich kopii w systemie operacyjnym. Brak plaintext fallbacku.

W 2A aplikacja nie tworzy tych wpisów automatycznie: moduł nie jest wpięty w UI ani dispatcher. Testy używają backendu pamięciowego; realnego systemowego keyringu nie uruchomiono. Zanim 2B wywoła provision lub HTTP, musi utrwalić referencję, intent/client_start_id i uzyskać exclusive provisioning lease. OS keyring nie zapewnia cross-process compare-and-set. Błąd/cancel/crash może pozostawić wpis pod wcześniej zarezerwowaną referencją — koordynator musi ją zachować i wznowić przez load, nie nową parę. Brakujący wpis to błąd/recovery, nie cicha regeneracja.

Nie dodano jeszcze registry w SQLite, automatycznego powiązania z profile.delete.permanent ani backup/restore referencji. remove() jest działającym prymitywem lifecycle; pełne profile lifecycle wiring wchodzi w 2B, zanim moduł zostanie udostępniony użytkownikowi. Pełny backup SQLite nie zawiera kluczy Community. Przy przyszłym restore trzeba odtworzyć referencje i sprawdzić OS vault; nie wolno traktować brakujących kluczy jako nowej tożsamości.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline | **456/456 PASS**, 22 zestawy |
| Nowe testy community_identity (w powyższym przebiegu) | **21/21 PASS** |
| Niezależny OpenSSL CLI: podpisy z Rust / zmienione komunikaty | **2/2 verified / 2/2 rejected** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| P2 reference checks | **193/193 PASS** |
| Granice + schema expectation sweep | **34/34 PASS**, 13 asercji v15 |
| Command parity | **54/54 PASS** |
| Community identity / reader static guards | **14/14 / 14/14 PASS** |
| Experimental foundation / runtime / continuity / internal state / reflection | **12/12 / 24/24 / 23/23 / 23/23 / 16/16 PASS** |
| Standard closure / release / Rust source-text | **8/8 / 8/8 / PASS** |

Testy i logi autora: Rust 1.99.1, Node 24.22.1, Linux. Nie są potwierdzeniem systemowego OS vault, Windows/Tauri ani rejestracji na forum. W początkowym przebiegu usunięto dwa nieużywane import/constant z nowego modułu; finalny przebieg nie ma nowych ostrzeżeń. Wcześniejsze unused-import warning w experimental_internal_state pozostaje bez zmian.


Nowe testy obejmują golden profile/statement bytes i ich SHA, dwa niezależne podpisy registration, fixture ogólny P-256, odrzucenie P-384, compressed aliases/role separation, trailing DER i padding, UTF-8/ASCII trim, scope, błędne suite/hash/nonce/statement, expiry i committed recovery, create/load w nowym obiekcie key store, cross-profile isolation, brak provider-secret fallback, usuwanie i missing-key fail-closed, powtórzenie complete wire, limity JSON request, zmianę private material pod istniejącą referencją, brak plaintext fallback i nieobecność sekretów w publicznych danych.

Test restart odtwarza obiekt key store na zachowanym mock backendzie, nie restart procesu/systemowego keyringu ani restartu rejestracji HTTP. Test usunięcia dotyczy jawnego remove(), nie automatycznej komendy profile.delete.permanent. OpenSSL bridge eksportuje wyłącznie public key/statement/signature. Dowody nie zawierają private keys.

## Zależności i zgodność źródeł

Pinned direct dependencies: p256 =0.13.2, rand_core =0.6.4, zeroize =1.9.1. Cargo.lock dodał 17 pakietów; żadna istniejąca wersja nie została podniesiona ani usunięta. Zmiany w istniejących dependency nodes ai-home/generic-array/digest wynikają z nowych zależności i feature edges, opisanych w dependency-diff.json. Nie dodano zależności systemowego OpenSSL do aplikacji; OpenSSL CLI użyto tylko do niezależnej kontroli w tym środowisku. Public fixture'y skopiowano bajtowo z przyjętego forum 4A o SHA `afe25c58590ec146f7be01ef83fc4854e717813bb99c86cd7d2c89251e4ebc8a`.

Frozen P2, SQLite v15, istniejący provider vault, adaptery modeli, uprawnienia i workflow pozostają bez zmian. W CommunityReader udostępniono wyłącznie crate-local normalize_root do wspólnej walidacji URL; jego zachowanie nie zmienia się. Nie zmieniano dotychczasowych testów aplikacji. Nowe testy są automatycznie objęte istniejącym cargo test --all-targets. Opcjonalny skrypt OpenSSL nie dodaje nowego joba CI.

## Odbiór i następny krok

Nie uruchomiono Windows/Tauri/WebView, rzeczywistego OS vault, PHP/live forum ani live model API. Potrzebne są nowe CI i audyt kroku 2A. 2B musi dodać trwały registry i koordynator HTTP, status/reissue utraconego jednorazowego tokenu, kontrolę prawdziwego keyringu i pełne lifecycle/backup metadata. Nie wolno podłączyć samego complete packet jako pozornego sukcesu rejestracji.

CHANGED_FILES stosować na dokładnej zielonej bazie 4B12 STEP 1 AUDIT FIX 1: rozpakować w głównym katalogu AI_Home, nadpisać pliki, niczego ręcznie nie usuwać. Windows: `py -3 scripts/verify-source.py .`, commit/push, dotychczasowy CI. SOURCE to pełne źródła; AUDIT_PACKAGE zawiera dokładny BASE oraz referencyjne forum, dokumenty, logi i dowód BASE+OVERLAY=SOURCE. Bez release i instalatorów.
