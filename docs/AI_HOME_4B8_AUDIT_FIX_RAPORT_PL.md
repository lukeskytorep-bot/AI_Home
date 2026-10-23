# AI Home 4B-8 AUDIT FIX — raport

Data: 24.10.2026

Zakres poprawki jest ograniczony do D1 oraz nieblokującego doprecyzowania D2 z audytu głównego 4B-8.

## D1

Test `tests/diagnostics-ui.test.tsx` nie traktuje już ogólnej frazy „conversation text” jako markera wycieku. Test oczekuje teraz obecności komunikatu prywatności, liczników, kodu błędu oraz etykiety agregatu. Dowód braku prywatnej treści pozostaje po stronie natywnego testu diagnostyki z unikalnym markerem danych źródłowych.

## D2

Etykieta `diagnostics.errors` została doprecyzowana do „Most frequent error codes” / „Najczęstsze kody błędów”. Backend nadal grupuje całą zachowaną historię według liczności, więc ta nazwa odpowiada faktycznej semantyce. Pole supplemental DTO `recentErrors` pozostaje bez zmian, aby nie wykonywać zbędnego churnu kontraktu.

## Zakres techniczny

- brak zmian w backendzie diagnostyki,
- brak zmian frozen P2,
- brak migracji SQLite,
- storageVersion 12,
- wymagana pełna walidacja GitHub Actions, w tym Vitest, typecheck/build i Cargo privacy regression,
- desktop smoke WebView nadal wymagany do formalnego PASS CLOSED 4B-8.
