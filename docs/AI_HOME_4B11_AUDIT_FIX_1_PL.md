# AI Home — 4B-11 AUDIT FIX 1 — F1 / F2

Data: 26 października 2026. Status: CORRECTED / AWAITING CI AND RE-AUDIT.
Baza to dokładnie audytowany `AI_HOME_4B11_REFLECTION_DREAM_SOURCE.zip`, SHA-256 `488a76fcbf1e36133be16b5da2b3adeac1f5508d8cecc12ad238d87189279aef`, 618 plików. Nie zakładamy potwierdzenia CI dla nowej korekty. Plan: treść 1.72.

## F1 — zapis musi dać się później przeczytać

Przyjęto dopuszczone przez audyt rozwiązanie ze spójnym limitem pełnego serializowanego elementu. Wspólny konstruktor candidate_entry jest używany przez pakowanie kontekstu i walidację przed zapisem. Każda propozycja musi zmieścić się samodzielnie w pakiecie context() do 4000 bajtów. Zanim review() zatwierdzi własną notatkę/wartość, sprawdzany jest kompletny finalny rekord po accept/modify, z pochodzeniem i metadanymi; sama długość content nie wystarcza. Uwzględniono maksymalny licznik pominięć, przyszłe rewizje, dłuższe etykiety permanently_deleted i metadane wycofania. Limit publicznego prywatnego kontekstu nadal wynosi 4096 bajtów.

Nieprzeglądalny zapis zwraca CONTEXT_BUDGET_EXCEEDED i transakcja nie zapisuje review receipt ani nowej notatki, nie zużywa propozycji i nie zastępuje starego wpisu. Zbyt duży output nie pozostaje jako pending. Unlimited nie usuwa konieczności zmieszczenia się w kontekście przeglądu. Nie dodano obejścia wymagającego zaufania niewidocznemu wpisowi ani ręcznego zatwierdzania przez człowieka.

Regresje natywne: mała propozycja → modify 5000 bajtów przy unlimited → atomowe odrzucenie → późniejszy reject oryginalnej propozycji; output_bytes 8192 i duży candidate → brak częściowych rekordów; tekst z wieloma newline potwierdza liczenie JSON escaping; modify 2000 bajtów przy unlimited → pełny przegląd i WithdrawSelf w następnej okazji.

## F2 — pierwotne źródła pozostają w provenance

source_refs() łączy odnośniki bezpośrednie z pierwotnymi sourceRefs technical i semantic checkpoints, własnych notatek oraz pending candidates faktycznie podanych generatorowi. Przy kolejnych generacjach oryginalny conversation/tool/source pozostaje bezpośrednio w spłaszczonej liście. Review przenosi tę listę do zaakceptowanej notatki.

Deduplikacja po (sourceKind, sourceId, sourceRevision, sha256) zachowuje różne rewizje źródła; przy sprzecznej dostępności tego samego odnośnika wygrywa permanently_deleted. Maksymalnie 32 refs i 8192 bajty ich serializacji. Brak cichego obcinania: preflight odrzuca nadmierne provenance przed rezerwacją runu i wywołaniem generatora. Niezależna kontrola F1 uwzględnia rozmiar refs w późniejszym pakiecie Primary. Nie dodano surowych treści źródeł do ledgeru.

Nowy test end-to-end tworzy prawdziwe rozmowy i checkpointy przez natywne usługi, generuje Dream i akceptuje notatkę przez runtime. Druga generacja dziedziczy oryginalne źródła z notatki po zakończeniu WorkEpisode (bez ResumePacket). Po wycofaniu pierwszej notatki trzecia generacja dziedziczy je z pending candidate; Primary ją akceptuje. Następnie rzeczywisty handler conversation.archive i conversation.delete.permanent usuwa rozmowę. Test sprawdza brak rozmowy, permanently_deleted w każdej powiązanej refleksji/notatce, dostępność drugiego nieusuniętego źródła, zachowanie tekstu/hashu/statusu notatki i propagację tombstone do kolejnej generacji. Wspólne źródło z technical i semantic występuje tylko raz.

Ten test jest testem natywnego runtime oraz storage handlera; nie twierdzi, że Experimental ma już publiczne IPC do kasowania rozmów. Zachowano dotychczasowe odcięcie tego trybu w dispatcherze Standard. Dodatkowa regresja 33 refs potwierdza odrzucenie przed generacją bez utraty/obcięcia provenance.

## Wyniki kontroli

| Kontrola | Wynik |
|---|---|
| Rust Linux headless, wszystkie targety, locked/offline | **414/414 PASS**, 22 zestawy |
| Reflection integration (w powyższym zestawie) | **26/26 PASS** |
| Nowa natywna regresja runtime → permanent deletion (w lib) | **1/1 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript / Vite build | **PASS** |
| Frozen P2 reference checks | **193/193 PASS** |
| Granice / schema expectation sweep | **34/34 PASS**, 13 asercji do v15 |
| Command parity | **54/54 PASS** |
| Experimental foundation / runtime / continuity / internal state | **12/12 / 24/24 / 23/23 / 23/23 PASS** |
| Reflection static scope guard | **16/16 PASS** |
| Standard closure / release / Rust source-text | **8/8 / 8/8 / PASS** |

Nowe regresje to łącznie pięć testów (cztery integration i jeden lib). Logi pochodzą z wykonania autora tej korekty w Linux, Rust 1.99.1 / Node 24.22.1. Brak nowych zależności i zmian workflow. Zachowano wcześniejsze unused-import warning w experimental_internal_state test. Statyczne checks i P2 nie zastępują natywnych testów, a headless Rust nie potwierdza desktopu ani live API.


## Zakres i ograniczenia

SQLite v15 i wszystkie kontrakty pozostają niezmienione. Brak zmian w zależnościach, lockfiles, providerach, vault, uprawnieniach, workflow i produkcyjnym IPC. Nowy plik z testem F2 jest dołączany tylko przez cfg(test). Nie dodano konektorów, UI Experimental ani live generatora. Samodzielna przeglądalność wpisu nie oznacza, że wszystkie wpisy będą dołączane jednocześnie — ograniczony kontekst nadal jawnie pomija nadmiar. Źródła nie są semantycznie usuwane z autonomicznie zatwierdzonego tekstu; zmienia się ich dostępność w provenance.

Korekta chroni nowe zapisy. Nie rekonstruuje historycznego pochodzenia już utraconego w poprzedniej implementacji i nie naprawia automatycznie starych nieprzeglądalnych rekordów. Wersja przed korektą nie miała jeszcze produkcyjnej integracji generatora/IPC. GitHub CI i ponowny audyt nadal są wymagane. Nie przeprowadzono Windows/Tauri/WebView, systemowego vault ani live API.

## Paczki i stosowanie

CHANGED_FILES to nakładka na audytowane 4B-11 SOURCE, nie na dowolny starszy ZIP. Rozpakować do głównego katalogu AI_Home (tam gdzie package.json), nadpisując pliki. Nie usuwać niczego ręcznie ani nie rozpakowywać do src-tauri. Uruchomić `py -3 scripts/verify-source.py .` w Windows, następnie commit/push i dotychczasowy CI. Nie tworzymy release ani instalatorów.

SOURCE zawiera komplet finalnych źródeł. OVERLAY_PROOF odtwarza rzeczywiste BASE+CHANGED_FILES=SOURCE=working directory. AUDIT_PACKAGE dołącza także dokładną audytowaną bazę 4B-11 i wcześniejszą SCHEMA14_FIX, aby audytor mógł odtworzyć obie relacje bazowe. Pełne hashe, logi i nazwy plików są w dołączonych dowodach.
