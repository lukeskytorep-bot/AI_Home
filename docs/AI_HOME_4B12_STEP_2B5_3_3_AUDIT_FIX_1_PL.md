# 2B-5.3.3 Audit Fix 1 — F1/P2

28.10.2026. IMPLEMENTED / AWAITING CI AND RE-AUDIT. Plan 1.87. Całe 2B-5.3 i 4B-12 pozostają OPEN.

Baza: dokładny SOURCE 2B-5.3.3, SHA-256 42b388f1c3a92cd7c69b693a4263ecec8202172ec177e4a2cb8c02558378a611. Audyt główny wskazał DO POPRAWY: brak oryginalnych kluczy jest opakowany w FinishError::Registry(RegistryError::Identity), podczas gdy adapter obsługiwał wyłącznie bezpośrednie FinishError::Identity.

Korekta dodaje do istniejącej gałęzi false wyłącznie zagnieżdżony RegistryError::Identity. Pozostałe RegistryError nadal trafiają do STORAGE_ERROR. Bazowy mechanizm tożsamości, vault i rejestru nie został zmieniony. Komenda nie regeneruje kluczy.

Trzy nowe regresje używają gotowego finish, poprawnego tokena i oryginalnych kluczy; najpierw potwierdzają true. Następnie osobny testowy backend kluczy zwraca brak materiału, VaultUnavailable lub inny poprawny zestaw kluczy dla tego samego scope. Wszystkie oczekują dokładnego sukcesu localCredentialUsable=false i onlineStatusKnown=false. Sprawdzają zachowanie sekretów, liczników create/remove i liczby receipts. Metody zapisu testowego backendu panikują, gdyby adapter próbował je wywołać.

RED na bazowej implementacji: 0/3 PASS, 3 FAIL. GREEN po poprawce: cały moduł adaptera 25/25 PASS, w tym trzy nowe regresje, istniejący pozytywny token i corrupt_finish -> STORAGE_ERROR. Logi i hashe w docs/evidence/4b12-step-2b5-3-3-fix-1/.

Weryfikacja Rust/Linux headless, realny SQLite i testowy vault; --lib --no-default-features --locked --offline. Pełnych 634 bazowych testów ani frontend nie powtarzano dla tej punktowej korekty. Poprzedni pełny wynik nie jest nowym wykonaniem. Nowe CI i ponowny audyt pozostają wymagane. Desktop, Windows, real OS vault i live forum nie zostały odebrane.

Nakładkę nałożyć na dokładny SOURCE 2B-5.3.3, commit/push i dotychczasowe CI. Bez migracji, dependencies, UI, uprawnień, zmian workflow lub release. Pakiet audytowy zawiera bazę, poprawiony SOURCE, overlay, wejściowy audyt, raport, plan i dowody rekonstrukcji.
