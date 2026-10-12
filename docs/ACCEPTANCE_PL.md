# Odbiór 4B-0

Stan po Final Normalization: etap nadal otwarty. Edward upoważnił rozpoczęcie pierwszej części 4B-1; nie oznacza to zamknięcia poniższych bramek.

- [x] Frontend: testy, TypeScript i build; lokalne wyniki w docs/evidence.
- [x] Korekta szkicu: Ponów i opóźnione zdarzenia nie kasują własnych zmian.
- [x] Cargo.lock zachowany; podpis async zwraca Result.
- [x] Edward potwierdził udane CI po poprawce; zapisano jako deklarację użytkownika, bez niezależnego odczytu przebiegu.
- [x] Jedna kanoniczna wersja P2 1.1, komplet źródeł/dowodów oraz wyjaśnienie 119/145/193.
- [x] Usunięto nieużywane duplikaty; aktualny manifest obejmuje całe przekazanie.
- [ ] Zachować link/commit/logi zielonego CI, potwierdzić zakres testów i obu platform; nie dopisywać liczby natywnych testów bez logu.
- [ ] Ręczny Windows: check_windows=true albo build_desktop=true, jeśli nie ma już dowodu jego zaliczenia.
- [ ] Kontrola wygenerowanych uprawnień głównego okna.
- [ ] WebView: start, zapis ustawień, restart, EN/PL, skórki light/dark/system, klawiatura, wąskie okno i błędy.
- [ ] Audyt Final Normalization i decyzja o zamknięciu 4B-0.

Przed 4B-12 ustalić status integracji 4A na podstawie bieżących dowodów. Potwierdzenie odbioru przez Edwarda obowiązuje; starszy indeks z otwartą bramką produkcyjną wymaga uzgodnienia, nie automatycznego przepisania na PASS.

## 4B-1 krok 1 — korekta audytowa

Korekta na dokładnej bazie AI_Home-main (1).zip: zgodny pełny inwentarz, dołączony weryfikator i zachowany podpisany tekst Google. Dodano test parser → replay; 15 testów Rust czeka na wykonanie w istniejącym CI. Szczegóły i hash bazy: docs/AI_HOME_4B1_KROK_1_RAPORT_PL.md. Całe 4B-1 pozostaje otwarte.
