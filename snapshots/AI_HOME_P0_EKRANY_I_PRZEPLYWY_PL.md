# AI Home — P0: ekrany i przepływy

11 października 2026 · Wersja 1.1 po uwagach audytora. Edward przyjął kierunek wizualny i upoważnił przejście dalej. Korekty makiety wykonane. To odbiór projektu ekranów, nie działającego runtime.

## Rezultat

`AI_HOME_P0_MAKIETA.html` jest samodzielną klikalną makietą z danymi demonstracyjnymi, bez serwera, zewnętrznych bibliotek i połączeń API. **Angielski jest domyślny**, polski dostępny przełącznikiem. W realnej aplikacji zapisany wybór ma być zachowany przy kolejnych uruchomieniach. Makieta przechowuje dane tylko do odświeżenia.

Nie wpisywać prawdziwego API key. Import zapisuje nazwę, przeznaczenie i przykładowe metadane; nie odczytuje ani nie wysyła treści. Wszystkie odpowiedzi i decyzje agenta są symulowane. Conscious pokazano jako planowany i niedostępny; konfiguracja uruchamia Standard.

## Ekrany

| Ekran | Zawartość |
|---|---|
| Konfigurator 1/4 | Standard, planowany Conscious; język dostępny od startu |
| Konfigurator 2/4 | Provider, demonstracyjny klucz, custom endpoint, test i błąd |
| Konfigurator 3/4 | Główny model, Scribe on/off, dwa niezależne demo modele Scribe |
| Konfigurator 4/4 | Nazwa profilu, demonstracyjny wybór imienia, domyślne 300 tokenów L2 |
| Rozmowa | Profile i historia po lewej, rozmowa w centrum, zwijana pamięć po prawej; osobne szkice |
| Pamięć | Robocza, wydarzenia, relacje, wiedza trwała, zamiary; wersje i nieaktualność |
| Wiedza | Cztery przeznaczenia, autor/importer, aktywne wskazówki i decyzje agenta |
| Zasady pracy | Przykład wskazówki z doświadczenia; bez rzeczywistego uczenia |
| Aktywność | Próby 1/2 i 2/2, odrzucenie, przyjęcie i zachowana pamięć |
| Ustawienia | Limity L2, bez limitu, symulacje, wygląd, język, opcjonalne forum/A2A |
| Dialogi | Import, historia wersji, symulowane ACCEPT/MODIFY/REJECT |

Skórka Orbital: jasny i ciemny wariant, akcent niebiesko-fioletowy i motyw orbity. Kolory w zmiennych CSS, etykiety w słownikach EN/PL. Reguły CSS poniżej 1000 i 650 px przestawiają panele. Instalacja nowych skórek i języków wymaga kontraktów P2. Zmiana UI nie tłumaczy zapisanych rozmów ani pamięci.

## Przepływy i poprawki

Pierwsze uruchomienie: tryb → połączenie → modele → tożsamość → rozmowa. Zmiana providera, klucza lub endpointu unieważnia test i blokuje przejście do ponownego sprawdzenia. Błąd zachowuje wpisane wartości. Test nie wykonuje żądań.

Każdy profil ma osobne dane i wybór modeli. Nowy profil ma własny konfigurator; ten sam provider jest dozwolony. Nowa rozmowa nie usuwa pamięci. Szkic należy do konkretnej rozmowy i pozostaje po zmianie wyglądu, języka, panelu lub powrocie z innego profilu. Po wysłaniu pole jest czyszczone. Trwałe ID, klucze i zapis należą do implementacji.

Dokument: plik → przeznaczenie → autor → dodanie. Przeznaczenia: rozmowa, wiedza profilu, stałe wskazówki, propozycja pamięci. Importer jest człowiekiem niezależnie od autora: człowiek/AI/współpraca/nieznany. Viewer Notes dostarczone przez człowieka nie otrzymują automatycznie jego autorstwa.

Propozycja pamięci otwiera „Symuluj decyzję agenta”: ACCEPT, MODIFY albo REJECT. Człowiek uruchamia demonstrację; UI nie udaje wykonanej decyzji modelu. MODIFY pokazuje przykładową poprawioną propozycję, REJECT jawne odrzucenie. Makieta nie przetwarza treści pliku i nie uruchamia Scribe.

Stałe wskazówki można włączyć i wyłączyć. Panel pokazuje aktywne nazwy dla następnego wywołania demonstracyjnego. W implementacji dochodzą pomiar budżetu i obsługa zbyt dużego zestawu. Treść nie otrzymuje prawa zmiany uprawnień aplikacji.

L2: 300 domyślnie, 500/800/1000, własna dodatnia wartość lub bez limitu długości zapisu. Ostrzeżenie wyjaśnia koszt i kontekst. Usunięto stałe 74%; makieta nie mierzy wykorzystania tokenów. Rozmiary prób są przykładowe.

Pamięć: pierwsze odrzucenie → druga i ostatnia próba → sukces albo końcowe odrzucenie. Niepowodzenie zachowuje poprawną wersję. Panel pokazuje wynik ostatniej operacji. Wycofana pamięć jest wyłączona z kontekstu, a jej treść pozostaje w historii. Przywrócenie tworzy kolejny zapis. NO_CHANGE nie tworzy nowej treści.

Docelowe uczenie odbywa się przez propozycje pamięci, źródła i zasady pracy. Primary przyjmuje, poprawia lub odrzuca; wyłączenie Scribe przenosi zadania do Primary. Makieta pokazuje ten projekt, nie implementuje uczenia ani treningu wag.

## Weryfikacja i odbiór

Przeszła kontrola składni i **62 asercje logiki z zastępczym DOM**: domyślny EN, szkice, izolacja, połączenie, Scribe, pamięć/wersje, importy, decyzje, wskazówki, limity, zatrzymanie i słowniki.

Nie wykonano niezależnego testu rzeczywistej przeglądarki: brak działającego silnika w środowisku, wcześniejsze pobranie nie powiodło się. Edward ocenił wygląd jako odpowiedni i pozwolił kontynuować. P0 zamknięto jako projekt ekranów; rzeczywiste renderowanie, klawiatura, focus/dialogi i małe okno pozostają kontrolą pierwszego działającego UI/WebView. Nie wpisano fikcyjnego PASS dla tych kontroli.

## Jak sprawdzić

1. Otwórz HTML: start po angielsku. Przejdź konfigurator; zmień klucz/endpoint po udanym teście i wybierz Scribe.
2. Wpisz szkic, zmień wygląd, język i panel, przełącz rozmowę/profil i wróć.
3. Zasymuluj dwa odrzucenia, sprawdź panel; wycofaj pamięć i przywróć wersję.
4. Dodaj propozycję pamięci, sprawdź trzy decyzje. Włącz/wyłącz stałe wskazówki.
5. Sprawdź oba wyglądy, PL/EN, wąskie okno i klawiaturę. Odświeżenie przywraca dane startowe.

## Dalej

P1 — audyt ponownego użycia Harness i decyzja techniczna — opisano w `AI_HOME_P1_AUDYT_HARNESS_I_ARCHITEKTURA_PL.md`. Wybrano Tauri 2 + React/TypeScript + Rust + SQLite. Następny etap P2 zamrozi kontrakty, potem 4B-0 uruchomi fundament aplikacji. Makieta nie zawiera runtime Harness. AI Home ma działać niezależnie od odebranego forum 4A.
