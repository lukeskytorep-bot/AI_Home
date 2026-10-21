# AI Home — 4B-3 krok 1: protokół Scribe i Working Context

21.10.2026. Wykonano przygotowawczy silnik i testy. **To nie jest jeszcze działająca pamięć w aplikacji ani zamknięcie 4B-3.**

## Baza i zakres

Edward potwierdził zielone CI poprawki vision i polecił kontynuować. Nie przekazał numeru/platform przebiegu ani nowego tekstu audytu. Dokładna baza: `AI_HOME_4B2A_KROK_3_AUDIT_FIX_SOURCE.zip`, SHA-256 `5e744d54b89920d7343003a3d21d1aaa9ec70109b22a9504795323911eda04bb`. Nie korzystano z innej rewizji GitHub.

Nowy `src-tauri/src/memory/mod.rs` jest czystą procedurą Rust, eksportowaną przez lib.rs; `src-tauri/tests/memory.rs` zawiera 29 regresji. Nie ma nowych poleceń IPC, tabel ani ekranów pamięci. `memory.update.request` nadal nie jest implementowane. Silnik nie wykonuje HTTP, nie czyta źródeł z dysku/bazy i nie przechowuje nic po restarcie. Ostatnia pamięć jest wyłącznie wejściowym snapshotem, którego nie modyfikuje.

## Zachowanie

- Propozycję tworzy trasa Scribe lub, gdy wyłączona, Primary. Zawsze występuje osobny etap decyzji Primary, nawet gdy oba zadania obsługuje ten sam model. Role mają różne tożsamości tras, mogą współdzielić połączenie. Model/protokół dostawcy nie jest zaszyty w silniku. Custom OpenAI pozostaje obsługiwane przez istniejący transport; nie wykonano nowego live testu.
- Propozycja to dokładnie `content`, `language`, `claimKind`; brak dodatkowych pól. Decyzja używa zamrożonego P2 PrimaryDecisionOutput. Pusty/whitespace tekst, błędny JSON i obce pola nie są kandydaturą. Kod nie usuwa markerów ani nie skraca treści.
- ACCEPT zachowuje dokładne bajty propozycji. MODIFY ponownie mierzy i hashuje faktyczną treść, a claimKind ustawia konserwatywnie na mixed. REJECT kończy procedurę bez ponowienia wymuszającego zgodę. Uzasadnienie jest publicznym polem reason; silnik nie żąda ukrytego rozumowania.
- Domyślna pojemność 300; 500/800/1000/własna dodatnia wartość oraz unlimited są obsługiwane przez kontrakt polityki. Unlimited nie podstawia ukrytego limitu treści. To nie znosi przyszłych ograniczeń odpowiedzi, HTTP, czasu, kontekstu ani dysku. W tym kroku nie ma jeszcze ustawienia ani ostrzeżenia w UI.
- Fallback pomiaru: liczba bajtów UTF-8, method=estimated, estimatorId=utf8_bytes_v1, ID i rewizja Primary. To konserwatywny szacunek treści, nie dokładne tokeny, liczba znaków czy gwarancja dostawcy. 300 nie oznacza 300 znaków; np. żż ma 4 bajty. Nie ma tokenizera modelu. Bieżący pomiar powstaje na nowo, nie bierze starego wyniku trasy Scribe.
- Pierwsze przekroczenie zapisuje w pamięci procesu numer/hash/pomiar i daje drugą, ostatnią próbę z jawną informacją o porażce pierwszej. Nadmiar propozycji nie uruchamia zbędnej decyzji Primary. Nadmiar po MODIFY ponawia całą kandydaturę. Dwa przekroczenia kończą procedurę bez nowej kandydatury.
- Jedna naprawa formatu na operację ma osobny licznik; nie resetuje prób pojemności. Najwyżej cztery wydane żądania logiczne; naprawa może wyczerpać pulę przed drugą decyzją. To **nie jest licznik fizycznych POST-ów**: przyszły koordynator musi rezerwować je w istniejącym trwałym budżecie przed każdym fizycznym wywołaniem, także retry/continuation. Nie dodano nowego retry transportu.
- Snapshot zawiera profil/rewizję, rekord/rewizję, poprzednią wersję i treść, obie trasy z pełnym scopeId, język, pojemność i źródła. Źródła muszą spełnić SourceRef, zgadzać się z profilem i hashem dostarczonego tekstu. To nie dowodzi samo w sobie prawa dostępu: tekst i referencje musi rozwiązać z własnej bazy natywny koordynator, nigdy UI. Ten krok obsługuje tekst, bez obrazów/OCR/automatycznego Scribe vision.
- Źródła/poprzednia pamięć/feedback pozostają JSON-em w user data; nie są interpolowane do system. Prompt nakazuje traktować je jako niezaufane dane. Sam prompt nie stanowi ochrony uprawnień.
- Język zadania pochodzi z rozmowy/polityki pamięci, nie locale aplikacji. Inny deklarowany language w propozycji jest odrzucony. Test potwierdza pl w danych i odrzucenie en; nie dowodzi jakości polszczyzny żywego modelu ani automatycznego rozpoznania języka po MODIFY.
- Powtórzona odpowiedź w złej fazie nie zmienia wyniku. Anulowanie/provider/budżet kończą procedurę; późny wynik jest odrzucony. Nie oznacza to jeszcze testu trwałego STOP/restart pamięci, których runtime nie jest podłączony.

## Granica bezpieczeństwa przed integracją

Outcome::Candidate jest **wyłącznie wewnętrzną zwalidowaną kandydaturą**, nie aktywną MemoryVersion ani MemoryDecisionReceipt. Nie wolno udostępnić metody decision jako zatwierdzenia UI. Koordynator musi przypisać odpowiedź do rzeczywistego, natywnie wybranego wywołania Primary, egzekwować własność źródeł/tras oraz CAS całego zakresu przed wywołaniem i commit. Sam engine nie odczyta aktualnej rewizji, nie wykryje zmiany klucza/endpointu/polityki w bazie i nie jest autorytetem uprawnień.

Do zrobienia: trwałe operacje/attempts/proposals/receipts/wersje, policy CAS, source snapshots, atomowy commit wskaźnika+wersji+wyniku+zdarzenia, przerwanie po restarcie bez automatycznego POST, preflight obu tras/output/czas/koszt, ustawienia i panel EN/PL oraz dołączenie aktywnego L2 do czatu. P2 wymaga zachowania ostatniej poprawnej wersji przy każdym błędzie. Procedura niczego dziś nie aktywuje.

## Sprawdzenia

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, wszystkie targety, --locked --offline | **200/200 PASS** |
| Nowe testy protokołu pamięci | **29/29 PASS**, zawarte w 200 |
| Frontend | **115/115 PASS** |
| TypeScript i Vite | **PASS** |
| Granice modułów | **22/22 PASS** |
| Referencyjne P2 | **193/193 PASS** |

Rust wykonany domyślnie równolegle: 79 unit +31 chat +24 connections +10 conversations +8 foundation +19 imports +29 memory. Logi w docs/evidence/4b3-step-1. Nowa kontrola granic zabrania zależności runtime w czystym module pamięci. Kompilacja zachowuje istniejące ostrzeżenia dead_code/unused imports. Zestaw nie sprawdza desktop feature, Windows/WebView, prawdziwego vault ani płatnych API. Build/test frontend jest regresją istniejącego UI, nie testem nowego panelu pamięci.

Nie zmieniono SQLite v8, żadnego kontraktu, wszystkich zależności, Cargo.lock/package-lock, workflow, capabilities, providerów ani vault. Nie skopiowano nowych źródeł Harness; dotychczasowy kod/proweniencja pozostają bez zmian. Nowy silnik realizuje reguły istniejącego P2, a podłączenie ponownie użyje już adaptowanego transportu Harness. Nie są potrzebne release, instalatory ani dodatkowy job.

## Przekazanie i następny krok

Nakładka zawiera tylko pliki dodane/zmienione względem wskazanego SOURCE. Nie usuwa niczego. Manifest aktualnego drzewa i SHA256SUMS są odnowione; stare snapshoty i dowody pozostają niezmienione. Pełny SOURCE i dowód odtworzenia nakładki służą audytorowi. Plan ma treść 1.24, zachowuje nazwę i historię.

Po nałożeniu potrzebne jest nowe istniejące CI i audyt tego kroku. Następnie 4B-3 krok 2 — trwały koordynator i pionowy przepływ panelu, z prawdziwymi decyzjami modeli przez istniejący transport. Całe 4B-3, live API/vault oraz wcześniejsze bramki Windows/WebView pozostają otwarte. Nie deklarujemy PASS CLOSED na podstawie headless.
