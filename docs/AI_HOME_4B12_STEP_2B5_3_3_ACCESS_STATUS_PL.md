# 4B-12 / 2B-5.3.3 — lokalna kontrola poświadczeń

Data: 28.10.2026. Status: IMPLEMENTED / AWAITING CI AND AUDIT.

Baza: AI_HOME_4B12_STEP_2B5_3_2_SOURCE.zip, SHA-256 214eca73cc3d2e37f782918bcc7e9af1c86c9859f9aab76f964bfbac334a02f6. Edward potwierdził zielony wynik; zewnętrzny run nie był niezależnie odczytywany.

Dodano supplemental native command community.registration.access.status i osobne LocalCapability::AccessStatus. Komenda korzysta z tych samych kontroli profilu, rewizji, kierunku Standard, Experimental binding, deletion, registry lifecycle i host binding co adapter lokalny. Payload jest pusty i ścisły. Dostęp Status nie daje AccessStatus.

Wynik ma dokładnie localCredentialUsable oraz onlineStatusKnown=false. Brak przygotowania lub terminalnych metadanych daje false bez odczytu sekretów. Ready metadata wymaga RegistrationFinish::current: ponownego powiązania archiwalnego kontekstu rejestracji z oryginalnymi kluczami oraz załadowania tokena o właściwym scope, registration ID, agent ID i generation. Brak/niedostępność/niewłaściwe powiązanie kluczy lub tokena daje false. Uszkodzone metadane rejestru dają STORAGE_ERROR.

To odczyt chwilowego stanu lokalnego, nie grant dla późniejszego HTTP. Każdy późniejszy zapis sieciowy musi ponownie autoryzować kierunek, scope i token w swoim wywołaniu. Flaga true nie dowodzi aktualnej akceptacji tokena przez serwer. Nie zwracamy ID rejestracji, referencji vault ani tokena.

Odczyt nie zapisuje receipts, nie generuje ani nie usuwa kluczy/tokenów, nie wykonuje HTTP. Żądania nie są replayowane jako trwały snapshot: ponowienie tego samego request ID odczytuje bieżący stan. Wspólna bramka hosta zachowuje workera po anulowaniu i nie pozwala maintenance wyprzedzić trwającego odczytu.

Dodano cztery testy: absent/reserved/local-ready bez promocji do credential; osobne capability i ponowna kontrola archived; uszkodzony finish; rzeczywiste poprawne klucze i token z późniejszą utratą/podmianą scope, bez zapisu receipts i ujawnienia sekretu.

Weryfikacja: Rust 634/634 PASS w 23 zestawach (4 nowe testy), frontend 219/219, build PASS, P2 193/193, granice 34/34, migracje 14, parity 59/59 i wszystkie dotychczasowe skrypty verify PASS. Wyniki zapisano w docs/evidence/4b12-step-2b5-3-3/. Rust/Linux headless z --all-targets --no-default-features --locked; testowy backend sekretów, realny SQLite. Produkcyjny desktop, real OS vault i live forum pozostają do osobnego odbioru.

Zakres tego podkroku został ograniczony do kontroli poświadczeń przed adapterem HTTP. Start/complete/retry/recovery HTTP oraz świadome desktop enablement pozostają kolejnymi częściami 2B-5.3. Całe 2B-5.3, 2B i 4B-12 są OPEN. Store v16 i registry v4, bez migracji, dependencies, zmian UI, Tauri permissions i CI. Nałożyć overlay na dokładną bazę 2B-5.3.2, commit/push, zwykłe CI i audyt. Bez release/instalatorów.
