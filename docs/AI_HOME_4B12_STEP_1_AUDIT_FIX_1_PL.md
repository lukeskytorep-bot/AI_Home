# AI Home — 4B-12 krok 1 AUDIT FIX 1 — locked public read

Data: 26.10.2026. Status: CORRECTED / AWAITING CI AND RE-AUDIT.
Dokładna baza: `AI_HOME_4B12_STEP_1_SOURCE.zip`, SHA-256 `b5d5e2c195438476a3f2aff420a5090661326d27df0350983edbd253f43d51ee`, 669 plików. Audyt wejściowy: AI_HOME_4B12_STEP_1_AUDYT_GLOWNY_PL.md. Plan treść 1.74.

## Poprawka F1

Źródła przyjętego 4A wprost dopuszczają status locked. setContentStatus() obsługuje go obok active/deleted/hidden, requirePublicVisibility() nie ukrywa locked, a publicContent() zwraca normalną treść. Blokada zapisu nie jest blokadą publicznego odczytu.

W content() zmieniono wyłącznie dwa warunki: rozpoznawane stany to active/locked/deleted; normalny payload jest dopuszczony dla active i locked. Nie przepisujemy statusu ani treści i nie dodajemy uprawnień. deleted nadal wymaga tombstone. Tombstone nadal odrzuca title/body/preview/client_provenance/server_evidence. hidden oraz nieznane statusy są odrzucane. Pozostałe walidacje scope, typów, limitów, podpisu deklarowanego przez serwer i transportu są zachowane.

Uzupełniono lokalny inwentarz community.read.v1.json o normalPayloadStatuses i opis tombstone. Statyczny guard ma 14 zamiast 12 kontroli. Kontrakty serwera i frozen P2 nie zostały zmienione.

## Regresje i odtworzenie na bazie

Cztery nowe testy korzystają z rzeczywistego reqwest i loopback TCP, przy payloadach o kształcie publicContent():

1. Locked thread detail zachowuje cały payload, oryginalny status locked, UntrustedExternal i SignatureValidation::NotPerformed.
2. Mieszana strona active+locked zachowuje oba wątki i ich treści/statusy.
3. Hidden i nieznane statusy (w tym archived) są nadal odrzucane przez ścieżkę HTTP.
4. Deleted/tombstone pozostaje czytelny bez treści; payload w tombstone dla active/locked/deleted oraz normalny payload deleted bez tombstone są odrzucane.

Te same cztery testy uruchomiono z oryginalnym modułem mod.rs pobranym bajtowo z audytowanego BASE ZIP-a. Dwa testy locked zakończyły się FAIL / InvalidResponse, dwa ochrony hidden/tombstone przeszły. Oryginalny kod zastępowano tylko na czas odtworzenia, po czym przywrócono poprawkę i uruchomiono finalny pełny zestaw. baseline-negative.log jest celowo czerwonym dowodem odtworzenia poprzedniego błędu, nie błędem finalnych źródeł.

## Wyniki finalne

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, wszystkie targety, locked/offline | **435/435 PASS**, 22 zestawy |
| CommunityReader, w powyższym przebiegu | **21/21 PASS** |
| Odtworzenie 4 nowych testów na poprzednim module | **2 oczekiwane FAIL locked / 2 PASS ochrony hidden i tombstone** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript / Vite build | **PASS** |
| Frozen P2 | **193/193 PASS** |
| Granice / schema expectation sweep | **34/34 PASS**, 13 asercji v15 |
| Command parity | **54/54 PASS** |
| Community reader static/source guard | **14/14 PASS** |
| Experimental foundation / runtime / continuity / internal state / reflection | **12/12 / 24/24 / 23/23 / 23/23 / 16/16 PASS** |
| Standard closure / release / Rust source-text | **8/8 / 8/8 / PASS** |

Logi są w docs/evidence/4b12-step-1-audit-fix-1. Rust 1.99.1 / Node 24.22.1 / Linux. Nie przypisujemy temu wynikowi wykonania desktop Tauri ani rzeczywistego PHP/forum. Wcześniejsze unused-import warning w teście experimental_internal_state pozostało bez zmian.


## Zakres i granice odbioru

Krok 1 nadal jest natywną usługą publicznego odczytu: bez tokenów, executora, zapisu pamięci, IPC/UI, rejestracji, publikacji i feedu. Nie dodano żadnej nowej zależności, migracji, zmiany Cargo.lock/package-lock, workflow lub uprawnień. SQLite pozostaje v15. W istniejącym kodzie funkcjonalnym zmieniają się tylko dwa warunki content(); dodatki dotyczą testów, lokalnego opisu kontraktu, guarda i dokumentacji.

Nie uruchomiono PHP ani działającego forum przez HTTPS, Tauri/WebView, Windows, OS vault i live model API. Weryfikacja locked wynika z rzeczywistych źródeł forum oraz natywnych testów HTTP. Potrzebne są nowe CI i ponowny audyt tej korekty; cały etap 4B-12 pozostaje otwarty.

## Stosowanie i pakiety

CHANGED_FILES rozpakować do głównego folderu audytowanego AI_Home (tam, gdzie package.json), nadpisując pliki. Niczego ręcznie nie usuwać. Windows: `py -3 scripts/verify-source.py .`, następnie commit/push i dotychczasowy CI. Bez release i instalatorów.

AUDIT_PACKAGE zawiera finalny SOURCE, nakładkę, dokładny BASE kroku 1, poprzednią bazę 4B-11, poprzedni overlay oraz źródła referencyjnego forum. Pozwala sprawdzić zarówno tę poprawkę, jak i poprzedni łańcuch BASE+OVERLAY=SOURCE. Dokumenty standalone są bajtowo identyczne z kopiami w SOURCE; manifest i SHA256SUMS odnowiono na kompletnym finalnym drzewie.
