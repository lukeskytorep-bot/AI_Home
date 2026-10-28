# AI Home — 4B-12 / 2B-5.3.1 — Audit Fix 1: F1/P2

Data: 28.10.2026. Status: **IMPLEMENTED / AWAITING NEW CI AND RE-AUDIT**. Plan treść **1.84**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

Baza: `AI_HOME_4B12_STEP_2B5_3_1_SOURCE.zip`, SHA-256 `17051b868cfbdc47305e7cfd23db395f5c487ce5f9ddae70c9b5f8bf8ba020c9`, 942 pliki. Źródło ustalenia: `AI_HOME_4B12_STEP_2B5_3_1_AUDYT_GLOWNY_PL.md`, werdykt DO POPRAWY, F1/P2. Nie deklarujemy odbioru tej bazy ani zamknięcia podkroku; poprawka wymaga nowego CI i ponownego audytu.

## Usterka i korekta

Nowy read-only guard `backup::host_restore_pending` uznawał dowolny parsowalny JSON wyniku za dowód zakończenia starego restore. Samo `Some(value)` z result_by_request wystarczało, choć value mogło być null, pustym obiektem, applied=false albo wynikiem innego żądania. Żądanie też nie było w pełni sprawdzane.

Guard uznaje teraz completion dopiero po sprawdzeniu dokładnego istniejącego formatu żądania v2 i terminalnego wyniku jego writera:

- Request: dokładnie manifestVersion=2, requestId jako string 1–128 znaków Unicode zgodnie z envelope, bezpieczne backupId i safetyBackupId, backupSha256 jako 64 małe cyfry hex oraz requestedAt jako RFC3339. Nazwa pliku musi odpowiadać istniejącemu request_key(requestId).
- Result: dokładnie wymagane pola, applied jako boolean true, requestId/backupId/safetyBackupId identyczne z prawidłowym request, appliedAt jako RFC3339, sourceSchemaVersion jako znany dodatni integer 1–16 i restoredSchemaVersion jako taki sam integer. Legacy writer instaluje ten sam snapshot przed późniejszą migracją Store; nie zmienia wersji między tymi dwoma polami.
- Nieznane/dodatkowe pola i inna wersja żądania nie są udawane jako rozpoznany bieżący format. Brak result oznacza pending; błędny JSON, typ, pole, powiązanie, bezpośredni symlink lub nieprawidłowy plik wyniku daje błąd. Oba stany utrzymują RecoveryRequired przez istniejących callerów hosta.

Nie porównujemy bieżącego Main z dawnym wynikiem restore ani z backupSha256 z request. Legalna aplikacja mogła od zakończenia zmienić bazę. Nie wymagamy zachowania plików starych backupów do samego rozpoznania poprawnego historycznego ledgeru. Nie wymuszamy chronologii dat przy możliwej zmianie zegara systemowego. Daty muszą mieć rozpoznawalny format.

Zmiana funkcjonalna dotyczy wyłącznie nowego guardu hosta i prywatnych walidatorów w backup.rs. Nie zmienia istniejącego writer/apply_pending_restore/Standard dispatch ani frozen IPC. Wyniku błędu nie naprawia się przez usunięcie ledgeru. Host, jego ownership, key/token lifecycle, registry merge/tombstone i reszta restore core są identyczne z bazą.

## Regresje i dowód odtworzenia

Dodano 10 testów Rust do istniejącego modułu hosta. Te same nowe testy uruchomione przed zmianą backup.rs dały **8 FAIL / 2 PASS**. Osiem negatywnych grup odtworzyło nieprawidłowy Offline zamiast RecoveryRequired; oba pozytywne przypadki przechodziły już na bazie. Log `red-old-guard.log` jest oczekiwanym dowodem błędu starego kodu, nie wynikiem obecnej poprawki.

Negatywne przypadki sprawdzają null/{}/nie-obiekt, applied=false i zły typ applied, obcy requestId, backupId, safetyBackupId, usunięte i błędnie typowane wymagane pola, niepoprawną datę, wersje schematu, kształt request mimo obecnego result oraz obce pola. Przy odmowie testy sprawdzają brak otwarcia i dispatch Application, brak publikacji nowego bundle, odmowę rzeczywistego natywnego restore ze zweryfikowanego wejścia, brak nowego receipt/marker/fence oraz bajtowo zachowane Main i oba pliki ledgeru.

Pozytywne testy sprawdzają prawidłową parę historyczną z otwarciem aplikacji, backupem i native restore, a także request/result wygenerowane rzeczywiście przez istniejący Standard writer i apply_pending_restore. Po ich legalnym zakończeniu zmieniane są preferences w Main; ponowne otwarcie i nowy backup muszą nadal działać.

Odtworzenie RED: na czystą dołączoną bazę nałożyć wyłącznie nowy `src-tauri/src/community_backup/host_tests.rs` i uruchomić filtr `community_backup::tests::host_tests::legacy_`, pozostawiając bazowy backup.rs. Oczekiwane 2 PASS / 8 FAIL. Ten sam filtr z oboma plikami poprawki ma 10 PASS; cały moduł hosta ma 27 PASS.

## Weryfikacja końcowa

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline | **612/612 PASS**, 23 zestawy |
| Nowe regresje F1 (cały moduł hosta 27/27) | **10/10 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript/Vite build | **PASS** |
| Frozen P2 | **193/193 PASS** |
| Granice / parity | **34/34 / 54/54 PASS** |
| Pozostałe 10 skryptów verify | **PASS** |

Weryfikacja lokalna: rustc 1.99.1, cargo 1.99.1, Node 24.22.1, npm 11.9.0. Rust testowano z `--no-default-features`, bez Tauri/WebView. Pełny desktop/default-feature build pozostaje do dotychczasowego CI.

Rust/Linux headless z --all-targets --no-default-features --locked --offline. Testy używają realnych SQLite i testowych backendów sekretów; nie łączą się z forum ani płatnymi modelami. Nie odebrano default desktop, Windows/WebView, realnego OS vault, live forum/TLS, force-kill/power-loss ani fsync failure. Dotychczasowe ostrzeżenie nieużywanego InternalStateProvider w niezmienionym teście pozostaje.

## Przekazanie

Nałożyć nakładkę na dokładną audytowaną bazę **2B-5.3.1**, nadpisać pliki, commit/push i uruchomić dotychczasowe CI. Nie ma usunięć. Workflow i cache bez zmian; dodatkowy release/instalatory nie są potrzebne. Cargo.toml/lock, schematy Store v16 / registry v4, frontend, providers, uprawnienia i kontrakty są bajtowo zgodne z bazą.

Pakiet audytowy zawiera dokładną bazę, poprawiony SOURCE, overlay, wejściowy audyt, raport, plan 1.84, aktualny guide EN, log RED i GREEN, pełną regresję, hashe oraz dowód BASE + overlay = SOURCE. Po zielonym CI i pozytywnym ponownym audycie można odebrać ten podkrok. Nie włączamy desktopu ani nie przechodzimy do następnej części w tej poprawce.
