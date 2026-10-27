# AI Home — 4B-12 krok 2B-3 — complete, custody i odzyskiwanie tokenu

Data: 27.10.2026. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść 1.78. Całe 2B i 4B-12 pozostają OPEN.

Baza: AI_HOME_4B12_STEP_2B2_SOURCE.zip, SHA-256 `548a646031959a141ad5a8e1e46ea4171cde40f3ec5ffb93cc936d169c4d5574`, 797 plików. Edward potwierdził zielony wynik. Nie pobierano niezależnie nowego numeru GitHub CI ani audytu. Kontrakt pochodzi z dokładnej odebranej bazy forum 4A-8C-4 V4, zachowanej w pakiecie.

## Wynik

Dodano RegistrationFinish: natywne jawne complete, jawny retry tego samego utrwalonego pakietu oraz recovery przez proof-status i token reissue. Nie ma UI/IPC ani automatycznych wywołań przy uruchomieniu aplikacji. Moduł nie publikuje treści forum.

Pierwszy complete ponownie sprawdza oryginalny zakres, profil i faktyczne klucze. Podpisuje świeży pending challenge oboma rolami i zapisuje dokładne bajty complete w SQLite. Przed POST-em zapisuje również dispatch journal z rewizją operacji, metodą/trasą/body/headers, stosownym canonical proof statement i zarezerwowaną referencją kandydata na token. To wrażliwe podpisane artefakty o ograniczonej władzy, nie diagnostyka bez treści.

Wynik musi mieć powiązane registration_id, committed state, poprawne agent_id, fresh-token flags i oczekiwaną generację. Canonical token ma eqai_ + base64url 32 losowych bajtów. Z response jest przenoszony do SecretToken bez Debug/Serialize. Trafia do odrębnego vault i przechodzi świeży readback; dopiero potem pointer jest aktywowany w SQLite przez CAS. Publiczny wynik zawiera wyłącznie metadane, a SQLite nie zawiera bearer tokenu.

Ponowne complete na niejednoznacznym rekordzie nie wysyła niczego automatycznie. Jawny retry_complete wymaga aktualnej rewizji i odtwarza te same zapisane bajty po sprawdzeniu obu podpisów względem archiwalnego canonical statement. Nie regeneruje kluczy ani nowego client_start_id. Server 4A sprawdza committed przed expiry pending, więc identyczny replay może zwrócić null token i RecoveryRequired również po starym expiry. Nie jest to nowy sukces autoryzacji. Retry gotowego rekordu jest odmawiany; zwykły odczyt gotowości korzysta z current().

## Recovery

recover jawnie rozpoczyna nową rewizję operacji i wyłącza stary pointer tokenu. Ta operacja rotuje token na serwerze także wtedy, gdy lokalny stary wpis jeszcze istnieje; nie jest niewinnym odświeżeniem statusu. Nie ma automatycznych pętli ani retry na błędach/rate limitach.

Najpierw pobieramy świeży status proof challenge i sprawdzamy pinned origin/instance, registration, agent/key IDs, purpose, canonical nonce i expiry. Znane agent/key IDs są przypięte i nie mogą zmienić się między etapami. Statement jest konstruowany lokalnie, nie pobierany jako dowolny tekst do podpisania. Podpis korzysta wyłącznie z rzeczywistego oczekiwanego operational key; expiry jest kontrolowane wokół blokującego odczytu i przed HTTP.

Signed GET status wymaga committed, tych samych IDs, null token i poprawnej generacji nie niższej od lokalnie znanej. Potem pobieramy nowy reissue challenge i wymagamy dokładnie tej generacji. Zmiana między status a challenge zatrzymuje operację przed POST-em reissue. Canonical reissue body to expected_token_generation=N i LF; proof wiąże metodę, wirtualną trasę i SHA256 tego body. Odpowiedź musi przejść N -> N+1 oraz fresh-token flags. Null-token replay oznacza RecoveryRequired. Kolejne recovery użyje świeżych proofów i aktualnej generacji.

**Fizyczne URL-e PHP i podpisywane trasy różnią się zgodnie z 4A.** Status trafia do registration/status/?registration_id=..., lecz podpisuje /api/v1/registration/{id}/status. Reissue trafia do registration/token/reissue/, lecz podpisuje /api/v1/registration/{id}/token/reissue. Prefix instalacji wpływa na rzeczywisty URL, nie wirtualną trasę podpisu. Testowy niezależny serwer kontroluje oba warianty i weryfikuje podpisy.

## Immutable tokeny i spóźnione wyniki

Service tokenów to org.aihome.app.community.tokens.v1, odrębny od org.aihome.app.community.identity.v1 oraz vault providerów. Każda odpowiedź ma nową referencję ai-home-community-token:v1 zarezerwowaną w journal przed HTTP. Istniejące wpisy nie są nadpisywane; rekord sekretu wiąże scope, rejestrację, aktora i generację. Odczyt wymaga identycznego zakresu. Usuwanie to jawny scope-checked idempotentny prymityw; host musi wcześniej wykluczyć nadal działających writerów.

Anulowany/stary writer może dokończyć zapis wyłącznie pod swoją referencją. Nie nadpisze aktywnego wpisu. CAS rewizji nie dopuści starego pointera, a CAS generacji na serwerze blokuje starsze reissue wobec nowszej generacji. Recovery nie przyjmuje dowolnego osieroconego kandydata jako aktualnego tokenu: pobiera świeże proofy. Po niejednoznacznych zapisach mogą zostać orphan candidates, lecz ich referencje są wcześniej utrwalone i przeznaczone do przyszłego host cleanup.

progress.ready_metadata oznacza jedynie zapis SQLite. current() sprawdza aktualne klucze, scoped token i rewizję. credential() zwraca sekret wyłącznie przyszłym natywnym connectorom po tych samych kontrolach; nie wolno serializować go do IPC/JSON/logów. Gotowość nie gwarantuje przyszłych capabilities ani braku odwołania tokenu przez operatora. Historyczny start cache nie odzwierciedla aktualnego stanu serwera; po complete host ma używać finish.current().

## Migracja, granice i dane

Dedykowany rejestr migruje atomowo schema 1/2 -> 3. Dodano finish metadata i journal dispatches. Pierwotne preparation/start DDL pozostają bez zmian, Store aplikacji nadal v15. Dwie korekty historycznych testów są czysto migracyjne: przyszła wersja 3 -> 4 i odtworzenie pełnego inventory v1 bez nowych tabel z asercją aktualnego v3. Nie usunięto testów ani asercji.

Nowy contracts/community.registration.inventory.v1.json opisuje aktualny natywny koordynator. Zachowany community.identity.v1.json ma jawny historyczny scope offline 2A, więc jego flagi nie są stanem całego koordynatora. Są to inwentarze, nie frozen P2 ani nowe serwerowe DTO. Frozen P2, Cargo.toml/lock, package lock, workflow, provider vault/adapters i uprawnienia są bajtowo zgodne z bazą.

Współdzielony reqwest exchange zachowuje brak redirect/auto retry/proxy/cookies/bearer/provider credentials, deadline całego HTTP, limit 16 KiB także dla chunked i bezpieczne błędy/statusy. Sekrety nie są zapisywane do SQLite ani publicznego rezultatu. Zeroizing obejmuje wybrane bufory; reqwest/JSON/OS mogą tworzyć dodatkowe kopie. Nie obiecujemy wymazania całej pamięci procesu.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless all-targets locked/offline | **529/529 PASS**, 22 zestawy |
| Nowe finish/proof/token testy | **30/30 PASS**, część powyższych 529 |
| Końcowy focused run po doprecyzowaniu null-token replay | **30/30 PASS**, te same przypadki |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| P2 / granice / asercje aplikacyjnego SQLite | **193/193 / 34/34 / 13 przy v15 PASS** |
| Command parity | **54/54 PASS** |
| Standard closure / release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Community reader / historyczny identity scope guard | **14/14 i 14/14 PASS** |
| Niezależny OpenSSL CLI | **2/2 podpisy proofów verified, 2/2 zmiany rejected** |
| Niezależny generator Python publicznych fixture proofów | **3/3 pliki identyczne bajtowo** |
| Rust source text, manifest, ZIP, dokładna rekonstrukcja nakładki | **PASS** — pliki dowodowe w pakiecie |

30 nowych przypadków obejmuje trwałość pakietu i tokenu, close/reopen, cały status/reissue z kontrolą podpisów, lost complete/reissue, readback i final DB failures, marker przed POST, stale revision, late complete i late reissue, cancellation, malformed receipts i null-token replay, scope/purpose/expiry/key mismatch, rollback i zmianę generacji, brak tokenu/kluczy, tampered complete packet, brak tokenów w SQLite/publicznym wyniku, immutable/scoped/removable vault entries, odmowę obcych namespace, canonical token format, migrację v2 i niezależne proof fixtures/podpisy. Niezależne poole SQLite mają jednego CAS zwycięzcę.

Testy to rzeczywiste tymczasowe pliki SQLite i reqwest HTTP loopback, z backendem sekretów w pamięci. Mock implementuje osobny serializer zgodny z PHP i weryfikuje oba complete oraz status/reissue signatures. OpenSSL jest oddzielnym CLI; eksport obejmuje tylko message/publicKey/signature, bez prywatnych kluczy i tokenów. Generator Python nie importuje Rust i nie wykonuje PHP. Nie uruchomiono PHP/live forum/TLS deploymentu, OS vault, Windows/Tauri/WebView ani API modeli. Close/reopen nie jest restartem procesu systemowego. Pozostaje wcześniejszy warning unused InternalStateProvider w niezmienionym teście; nowe moduły nie dodają warningów.

## Następny zakres

Natywny complete/recovery jest zaimplementowany, ale aplikacja nadal go nie wywołuje. Przed udostępnieniem użytkownikowi potrzebne są profile lifecycle/permanent deletion, backup/restore obu baz i referencji sekretów, kontrolowane uzgadnianie niejednoznacznych claimów/orphan candidates, host direction/capability checks i IPC. Store-only backup nie zawiera rejestru ani sekretów Community. Nie należy usuwać rejestru jako sposobu recovery. Signed forum writes/feed pozostają później w 4B-12.

Nakładka bez usunięć jest na dokładnej bazie 2B-2. Audyt zawiera bazę, nakładkę, wynikowy SOURCE, pełny reference forum, raport, plan 1.78, EN developer guide i dowody. Istniejący cargo all-targets CI wykrywa testy bez zmiany jobów. Nie tworzymy dodatkowego release ani instalatorów.
