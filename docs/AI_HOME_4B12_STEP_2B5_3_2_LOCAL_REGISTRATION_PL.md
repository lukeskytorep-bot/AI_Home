# AI Home — 4B-12 / 2B-5.3.2 — lokalny adapter rejestracji i bramki kierunku/capability

Data: 28.10.2026. Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść **1.85**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

Baza: dokładny `AI_HOME_4B12_STEP_2B5_3_1_AUDIT_FIX_1_SOURCE.zip`, SHA-256 `803202e1d4acec12f927ef0dd9a5b19ce641fe868d033b4b10148293aeb589f1`, 957 plików. Edward potwierdził zielony wynik poprzedniej poprawki; nie odczytywano niezależnie zewnętrznego CI.

## Zakres

Dodano uzupełniający, natywny adapter czterech lokalnych operacji:

- `community.registration.status`,
- `community.registration.prepare`,
- `community.registration.provision`,
- `community.registration.recover`.

Kontrakt `contracts/community.local.ipc.json` nie jest produkcyjnym Tauri IPC. Domyślny `home_dispatch`, UI, permissions/capabilities Tauri i setup desktopu nie wywołują tego adaptera. Jest to kontrolowany punkt integracji dla następnego hostowego etapu, bez HTTP, tokenów i automatycznego kontaktu z forum.

## Autoryzacja i kierunek

`LocalAccess` jest osobnym argumentem natywnym. Nie implementuje deserializacji, a JSON żądania nie zawiera i nie może sam nadać capability. Każde wywołanie wymaga jednocześnie:

- capability odpowiadającego dokładnej operacji,
- zgodności profilu z przypiętym `CommunityScope`,
- aktualnej rewizji profilu z Main,
- aktywnego, niearchiwalnego profilu w kierunku `standard`,
- braku jakiegokolwiek autorytatywnego powiązania `experimental_instances`, nawet gdy body profilu zostałoby ręcznie podrobione jako Standard,
- braku pending profile deletion, zarezerwowanego ID usuwania, restore fence i nieaktywnego/reconcile/deleted lifecycle,
- zgodności trwałego host bindingu z tym samym rejestrem Community.

Capability jest oceniane ponownie także przy replay. Terminalny receipt nie omija późniejszej zmiany kierunku, rewizji, lifecycle albo uprawnień. Experimental nie dostaje w tym kroku żadnego owner override; jego decyzja/kierunek będą osobnym etapem integracji.

## Trwałość, idempotencja i przerwanie

Operacje mutujące rezerwują ID oraz fingerprint w istniejącej tabeli `receipts` Main **przed** zapisem rejestru lub vault. Fingerprint obejmuje komendę, profil, rewizję profilu, payload i dokładny scope binding. ID nie może zostać przejęte przez zwykły dispatcher ani inną komendę lokalną.

Po trwałym claimie adapter ponownie autoryzuje profil i utrzymuje transakcję zapisu Main przez operację rejestru/kluczy. Dzięki temu współpracujący Store nie może w trakcie zarchiwizować profilu ani zmienić kierunku. Wspólna bramka `CommunityHost` zatrzymuje nowe dispatch przy maintenance, czeka na istniejący adapter i zachowuje worker po anulowaniu future wywołującego.

- Niedokończone `prepare` można wznowić wyłącznie z identycznym requestem i pierwotnym `clientStartId`; bazowy rejestr rozstrzyga idempotencję.
- Niedokończone `provision` lub `recover` nie powtarzają automatycznie operacji vault. Zwracają `REGISTRATION_RECONCILIATION_REQUIRED`; operator/host wykonuje świeży `status`, a potem jawny nowy `recover`.
- `recover` tylko ładuje istniejące klucze. Brak kluczy zwraca `CREDENTIAL_UNAVAILABLE` i nigdy nie generuje zastępczej tożsamości.
- Zapisany terminalny wynik jest sprawdzany względem dokładnego schematu, requestId i semantyki stanu. `null`, `{}`, obcy lub wadliwy JSON nie staje się sukcesem.
- Replay zwraca historyczny snapshot wyniku. Aktualny stan należy odczytać świeżą komendą `status`.

`ready` oznacza wyłącznie gotowość lokalnego materiału kluczy. Każdy wynik ma `onlineStatusKnown: false`; adapter nie twierdzi, że profil jest zarejestrowany, uwierzytelniony albo aktywny na serwerze.

## Ochrona sekretów i granic

Wynik ujawnia jedynie `prepared`, `registrationRevision`, `preparationState` oraz `onlineStatusKnown`. Nie zwraca key reference, kluczy publicznych/prywatnych, tokenów ani danych odpowiedzi serwera. `status` nie czyta vault i niczego nie zapisuje. `prepare` nie generuje kluczy. `provision` używa istniejącego one-shot claimu registry; `recover` nie tworzy kluczy.

Nie dodano migracji, zależności, endpointu HTTP, retry sieciowego, token trust, produkcyjnego desktop enablement, zmian frontendowych, permissions ani workflow CI. Store pozostaje v16, Community registry v4. Frozen P2 pozostaje bez zmian; nowy kontrakt jest jawnie supplemental/native-only.

## Regresje

Dodano 18 testów Rust na realnych plikach SQLite i deterministycznym testowym backendzie sekretów. Sprawdzają pełną ścieżkę absent → reserved → ready → recover, dokładny replay bez powtórnego tworzenia kluczy, izolację profilu i scope, default deny, brak self-grant w JSON, stale revision, archived/non-Standard/Experimental, pending deletion, konflikt ID, wadliwy receipt, restart z pending claim, brak kluczy podczas recover, lifecycle reconcile oraz odmowę Offline/RecoveryRequired.

Test współbieżności zatrzymuje backend sekretów podczas `provision`, anuluje oczekującego klienta i potwierdza, że Main write lock oraz host maintenance gate pozostają utrzymane. Zewnętrzna próba archiwizacji dostaje blokadę, backup czeka, nowy local dispatch dostaje `Busy`, a po zwolnieniu powstaje dokładnie jeden zestaw kluczy i status jest `ready`.

## Weryfikacja końcowa

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline | **630/630 PASS**, 23 zestawy |
| Nowy moduł adaptera | **18/18 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript/Vite build | **PASS** |
| Frozen P2 | **193/193 PASS** |
| Granice / migracje / command parity | **34/34 / 14 / 58/58 PASS** |
| Pozostałe skrypty verify | **PASS** |

Weryfikacja lokalna: rustc 1.99.1, cargo 1.99.1, Node 24.22.1, npm 11.9.0, Linux headless. Rust uruchomiono z `--all-targets --no-default-features --locked --offline`. Testy nie łączą się z live forum ani płatnymi modelami. Nie odebrano default-feature desktop, Windows/WebView, realnego OS vault, live forum/PHP/TLS, fizycznego power-loss/force-kill ani fsync failure.

## Przekazanie

Nakładkę nałożyć na dokładną zieloną bazę **2B-5.3.1 Audit Fix 1**, nadpisać pliki, wykonać zwykły commit/push i uruchomić dotychczasowe CI. Nie ma usunięć, migracji ani potrzeby release/instalatorów. Pakiet audytowy zawiera bazowy SOURCE, overlay, pełny SOURCE, raport, plan, logi, hashe i dowód `BASE + overlay = SOURCE`.

Po zielonym CI i audycie następnym krokiem jest hostowy adapter HTTP/start/complete/recovery z jawną polityką tokenów oraz późniejsze świadome włączenie desktopu. Ten pakiet nie deklaruje ich wykonania ani zamknięcia całego 2B-5.3.
