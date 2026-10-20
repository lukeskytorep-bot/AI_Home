# AI Home 4B-2A krok 3 — korekta audytu vision

21 października 2026. Otrzymany werdykt: DO KOREKTY. Ta poprawka jest kandydatem do nowego CI i ponownego niezależnego audytu; nie oznacza PASS CLOSED ani przejścia do 4B-3.

## Problem i zmiana

Audytor potwierdził dwa przypadki: po odczycie deklaracji użytkownik zmienia klucz albo endpoint, poprawnie testuje nowe połączenie i dostaje unknown. Zapis wysłany ze starego panelu przechodził jednak routeRevision/policyRevision CAS i deklarował supported dla nowego zakresu. Native sprawdzał zakres zapisanej deklaracji przy odczycie, ale nie identyfikował całego zakresu, na który patrzył człowiek podejmujący nową decyzję. Poprzednie twierdzenie o pełnym związaniu decyzji wymagało tej korekty.

chat.vision.get zwraca teraz wymagany scopeId: SHA-256 serializowanego [namespace ai-home-vision-scope:v1, profileId, current vision_scope]. vision_scope obejmuje routeId/routeRevision/modelId, connectionId/connectionRevision, providerKind/normalizedEndpoint oraz credentialRevision. Identyfikator nie zawiera klucza API i nie jest sekretem ani atestacją providera — służy do optymistycznego porównania dokładnie odczytanego zakresu.

chat.vision.set wymaga scopeId z tego odczytu. Native ponownie odczytuje bieżącą trasę i połączenie w istniejącej transakcji, porównuje scopeId obok obu dotychczasowych rewizji i odrzuca rozbieżność kodem REVISION_CONFLICT przed zapisem, zdarzeniami i receipt. Nie przepisuje starej decyzji na nowy zakres. Kontrola verified nadal obowiązuje. Powtórzenie wcześniejszego już przyjętego requestId zachowuje istniejącą semantykę receipt; nie wykonuje nowej mutacji zakresu.

Panel przekazuje identyfikator rzeczywiście odczytany przez get. Po konflikcie odczytuje aktualne unknown i wymaga ponownego jawnego wyboru. Nie włącza supported automatycznie po odświeżeniu. scopeId jest również częścią klucza unieważniającego lokalny preflight. Własny vision.ipc.json wymaga identyfikatora w wynikach get/set oraz payloadzie set (64 znaki hex). Stary payload bez identyfikatora jest niepoprawny, bez fallbacku do aktualnego zakresu.

Nie zmieniono kształtu vision_scope zapisanego w SQLite, więc dotychczasowe zgodne deklaracje i dane nie wymagają resetu/migracji. Baza pozostaje v8. Nie zmieniono P2/foundation, providerów, vault, importu obrazów, limitów, zależności, Cargo.lock, workflow ani capabilities.

## CSP podglądu

Przy sprawdzaniu uwagi audytora zauważono brak jawnego blob: w img-src w obu konfiguracjach Tauri, mimo że panel używa lokalnych Blob URL. Dodano blob: wyłącznie do img-src w csp i devCsp. Nie zmieniono script-src, connect-src ani uprawnień IPC. Statyczna regresja sprawdza obie polityki, brak wildcardów i brak blob: w innych dyrektywach.

Dokumentacja dyrektywy: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/img-src . To korekta konfiguracji, nie dowód rzeczywistego renderowania. Realny WebView, dekodowanie w nim i zastosowanie CSP nadal wymagają odbioru. Nie przedstawiamy testu jsdom ani kontroli JSON jako testu przeglądarki.

## Testy

| Kontrola | Wynik |
| --- | --- |
| Celowane regresje audytowe — klucz i endpoint | 2/2 PASS |
| Pełny Rust Linux headless, --all-targets --locked --offline | 171/171 PASS |
| Frontend Vitest/jsdom | 115/115 PASS |
| TypeScript i Vite build | PASS |
| Granice modułów | 21/21 PASS |
| Referencyjne P2 | 193/193 PASS |
| Nakładka na dokładną bazę → pełne źródła | wynik w OVERLAY_VERIFICATION.log |
| Inwentarz i hashe | wynik w OVERLAY_VERIFICATION.log |

Dwie nowe stałe regresje Rust wykonują rzeczywiste polecenia dispatchera:
1. Odczyt supported; zmiana klucza; poprawny connection.test; unknown; stary scopeId przy niezmienionych routeRevision i policyRevision → REVISION_CONFLICT.
2. Odczyt supported; zmiana endpointu i ponowne związanie klucza; poprawny connection.test; unknown; stary scopeId przy niezmienionych routeRevision i policyRevision → REVISION_CONFLICT.

Obie sprawdzają niezmienione deklaracje i liczbę zdarzeń po odrzuceniu oraz możliwość nowej świadomej deklaracji po świeżym get. Nie korzystają z płatnych API ani OS vault — używają loopback i wstrzykniętego magazynu.

Nowa regresja UI weryfikuje przesłanie starego identyfikatora, obsługę konfliktu, odświeżenie do unknown, brak automatycznego drugiego zapisu i nowy zapis dopiero po jawnym wyborze z aktualnym identyfikatorem. Istniejący test CAS oraz ścisłe mocki get/set uwzględniają wymagany identyfikator. Druga nowa kontrola UI jest statycznym testem obu CSP, a nie runtime przeglądarki. Frontend wzrósł ze 113 do 115, Rust ze 169 do 171. Nie poluzowano wcześniejszych asercji, limitów ani watchdogów.

Pełny zestaw zachowuje wcześniejsze kontrole obrazów, prywatnego replay, scope, receipts, STOP i restartu. Wyniki korekty: docs/evidence/4b2a-step-3-correction/. Starsze dowody pozostały bajtowo bez zmian. Nadal widoczne są wcześniejsze ostrzeżenia dead_code/unused imports.

## Przekazanie i stan

Dokładna baza: AI_HOME_4B2A_KROK_3_SOURCE.zip, SHA-256 `8d6cc3eed8ea56ad6891193b232da13519a011c22f61a323fd3aefc4b138a3fd`. Nakładka jest przeznaczona na tę bazę, bez usuwania plików. Pełne źródła oraz zewnętrzne raport/plan są sprawdzane bajtowo. Manifest i SHA256SUMS odnowiono dla nowego drzewa. Kanoniczny plan ma treść 1.23 i zachowuje historię; aktualny Acceptance opisuje DO KOREKTY i otwarte odbiory.

Rozpakować CHANGED_FILES.zip w głównym katalogu projektu, zastąpić pliki, commit i push. Wystarczy istniejące CI; bez Prepare Cargo lockfile, dodatkowych jobs, release lub instalatorów. Potem ponowny niezależny audyt. Nie zamykać 4B-2A ani rozpoczynać 4B-3 na podstawie samych starszych zielonych wyników.

Nie wykonano desktop feature/Windows/WebView, rzeczywistego OS vault ani live API. Brak tych odbiorów jest nadal jawnym ograniczeniem.
