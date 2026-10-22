# Instrukcja wdrożenia — AI Home 4B-6 krok 2

1. Bazą jest dokładny `AI_HOME_4B6_KROK_1_AUDIT_FIX_4_SOURCE.zip`.
2. Nałóż `AI_HOME_4B6_KROK_2_CHANGED_FILES.zip` na katalog `AI_Home/`, bez usuwania innych plików.
3. Uruchom pełne CI: Cargo `--all-targets --locked`, Vitest, TypeScript i Vite build.
4. Szczególnie sprawdź `src-tauri/tests/backup.rs`, migracje do schema 12, restart po restore oraz brak regresji vault/provider.
5. W desktop smoke: utwórz backup, zmień lokalne dane, wykonaj preflight, Prepare restore, zamknij i uruchom aplikację ponownie; potwierdź odtworzenie danych i obecność safety backupu.
6. Nie traktuj backupu jako portable exportu. Klucze API pozostają w OS vault; krok 2 nie implementuje przenoszenia poświadczeń.
7. Nie rozpoczynaj 4B-6 kroku 3 przed zielonym CI i audytem tej paczki.
