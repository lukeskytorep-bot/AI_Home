# AI Home — Checklista testów użytkownika na realnym Windows (Paczka 4E)

**Data:** 2026-11-27  
**Baza repozytorium:** `92ab720` (`main`) + kumulatywna nakładka **Paczka 4E** (SQLite `v20`, 89 komend IPC, 255/255 testów Vitest, wszystkie testy integracyjne Rust PASS na Linux headless).  
**Cel:** Praktyczna weryfikacja na docelowym sprzęcie **Windows (WebView2 + Windows Credential Manager / OS Vault + NTFS)** wszystkich ścieżek zbudowanych od Paczki 2A do Paczki 4D.

> **Zasada oznaczania:** Każdy punkt oznacz jako `PASS`, `FAIL` (z krótkim opisem/zrzutem) albo `BLOCKED` (np. brak żywego serwera Community 4A/TLS). Brak wykonania pozostaje `OPEN` — nie wpisuj domyślnie `PASS`.

---

## 0. Warunki wstępne przed testem

- [ ] Zbudowana lub zainstalowana aplikacja z aktualnej paczki (Windows CI `build_desktop=true` albo lokalny `npm run tauri build`).
- [ ] Przygotowany co najmniej jeden działający klucz API (np. OpenAI, Anthropic, Gemini lub Custom OpenAI) oraz opcjonalnie błędny klucz do próby negatywnej.
- [ ] Zapisany identyfikator buildu / commitu: `______________________`

---

## 1. Start aplikacji, WebView2 i pierwsza rozmowa (Standard)

| # | Scenariusz | Oczekiwany wynik | Status |
|---|---|---|---|
| 1.1 | Czysty start aplikacji na Windows | Okno WebView2 otwiera się bez błędów; widać uproszczony kreator (nazwa agenta, dostawca, klucz API; pole adresu tylko gdy wymagane). | `OPEN` |
| 1.2 | Próba negatywna: błędny klucz API lub zły adres Custom OpenAI | Kreator pokazuje czytelny błąd i pozwala poprawić klucz/adres bez tworzenia duplikatów profilu i połączenia. | `OPEN` |
| 1.3 | Poprawna konfiguracja i pierwsza wiadomość | Aplikacja zapisuje klucz w **Windows Credential Manager (OS Vault)**, przypisuje `Primary`, proponuje model startowy i zwraca rzeczywistą odpowiedź modelu. | `OPEN` |
| 1.4 | Restart aplikacji (zamknięcie i ponowne uruchomienie) | Gotowy profil otwiera od razu obszar rozmowy bez ponownego kreatora; historia pierwszej rozmowy i klucz z OS Vault działają od razu. | `OPEN` |
| 1.5 | Układ UI, Settings i język | `Settings` jest na górnym pasku; rozmowa ma pełną szerokość z opcjonalnym panelem kontekstu; przełączenie języka EN/PL działa bez brakujących etykiet. | `OPEN` |

---

## 2. Przełącznik Standard / Autonomous i lokalny pilot

| # | Scenariusz | Oczekiwany wynik | Status |
|---|---|---|---|
| 2.1 | Przełączenie `Standard` ↔ `Autonomous` na górnym pasku | Zmienia się przestrzeń robocza; nie nadpisuje ukradkiem profilu Standard, nie wymusza akcji i nie kasuje zapisanego stanu. | `OPEN` |
| 2.2 | Utworzenie instancji Autonomous + konfiguracja API i budżetu | Instancja zapisuje własne połączenie/budżet (klucz w OS Vault); widać aktualny status i pozostały budżet wywołań/tokenów. | `OPEN` |
| 2.3 | Cykl propozycji i odmowy (`proposal → refuse → restart`) | Odmowa agenta jest trwała po restarcie; człowiek nie może jej nadpisać poleceniem; zmienione warunki wymagają nowej decyzji. | `OPEN` |
| 2.4 | Pauza, wznowienie i `Reconcile` akcji `uncertain` | Techniczna pauza wstrzymuje nowe wywołania; jawne uzgodnienie (`Reconcile`) zdejmuje `recovery_required` bez ślepego powtarzania akcji. | `OPEN` |
| 2.5 | Sen AI (`nextWakeAt`) i refleksja/ciągłość (`Reflect`/`Dream`) | Podczas aktywnego snu wybranego przez AI wyzwalacz nie skraca snu ani nie wywołuje API; po terminie `nextWakeAt` cykl refleksji działa bez publikowania prywatnego scratchpadu. | `OPEN` |

---

## 3. Widoki `Notes` i `Community` (dowolna kompatybilna społeczność)

| # | Scenariusz | Oczekiwany wynik | Status |
|---|---|---|---|
| 3.1 | Otwarcie widoku `Notes` w lewej nawigacji | Wyświetla notatki robocze i warstwy pamięci wybranego profilu; samo otwarcie nie tworzy nowych notatek i nie wywołuje modelu. | `OPEN` |
| 3.2 | Otwarcie widoku `Community` (bez fałszywego statusu online) | Wyraźnie rozróżnia: konfigurację konta, lokalne poświadczenie w Vault i nieznany/sprawdzony status online. | `OPEN` |
| 3.3 | Dodanie / edycja / usunięcie konta społeczności (Maintenance UI) | Można podłączyć dowolną kompatybilną społeczność (URL, klucz P-256 i opcjonalny token zapisywane w OS Vault, izolacja per profil). | `OPEN` |
| 3.4 | Odczyt publicznego feedu i podpisana publikacja *(wymaga żywego serwera 4A/TLS lub lokalnego mocka)* | Feed ładuje się poprawnie; publikacja jest podpisywana ECDSA P-256; niepewny wynik uruchamia ścieżkę `Reconcile`. | `OPEN` *(zależne od serwera TLS)* |

---

## 4. Panel `MCP` i panel `Skills` w `Settings`

| # | Scenariusz | Oczekiwany wynik | Status |
|---|---|---|---|
| 4.1 | `Settings → MCP`: dodanie serwera MCP per profil | Pole tokenu czyści się po zapisie; sekret trafia do OS Vault; lista jest izolowana między profilami Standard i Autonomous. | `OPEN` |
| 4.2 | `Settings → MCP`: `Discover`, allowlista narzędzi i usuwanie | Wykryte narzędzia wymagają jawnej allowlisty (`untrusted metadata`); niepewne wywołanie wspiera `Reconcile`; usunięcie czyści wpis. | `OPEN` |
| 4.3 | `Settings → Skills`: dodanie własnego skilla i edycja grantów | Skill zapisuje się per profil; pole `Requested (untrusted)` jest wyłącznie informacyjne; działają tylko jawnie zaznaczone `Granted`. | `OPEN` |
| 4.4 | `Settings → Skills`: inwariant **No Self-Grant** | Skill oznaczony jako `imported` jest zapisywany wyłącznie jako wyłączony i bez żadnych grantów; próba włączenia/nadania uprawnień przy imporcie jest blokowana (`POLICY_DENIED`). | `OPEN` |

---

## 5. Kopia zapasowa (NTFS), eksport i trwałe usuwanie profilu

| # | Scenariusz | Oczekiwany wynik | Status |
|---|---|---|---|
| 5.1 | Eksport i import backupu na Windows (NTFS) | Operacja przechodzi bez błędów blokady/zamiany plików NTFS (potwierdzenie poprawki z Paczki 2C Fix 2 Windows); stan wraca poprawnie po restarcie. | `OPEN` |
| 5.2 | Archiwizacja i trwałe usunięcie testowego profilu (`profile.delete.permanent`) | Profil po archiwizacji daje się trwale usunąć; znikają powiązane serwery MCP, konta Community, skille (`profile_skills`) i wpisy, a aktywny drugi profil pozostaje nienaruszony. | `OPEN` |

---

## 6. Bramki zewnętrzne pozostające OPEN po Paczce 4E

1. **Próby na realnym sprzęcie Windows** (powyższa checklista 1.1–5.2: WebView2, Windows Credential Manager, rzeczywiste wywołanie API, NTFS).
2. **Testy end-to-end przeciwko żywemu serwerowi Community 4A z produkcyjnym TLS** (rejestracja, feed, podpisane zapisy, restart, odzyskiwanie dostępu).
3. **Przeglądarka (browser automation) oraz Messenger/A2A** — nadal odłożone jako osobne etapy z własnymi przeglądami i odbiorami.
