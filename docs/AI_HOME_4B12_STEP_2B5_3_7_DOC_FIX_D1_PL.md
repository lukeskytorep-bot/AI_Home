# AI Home 2B-5.3.7 — korekta dokumentacyjna D1

Data: 29.10.2026. D1/P3: FIXED IN DOCUMENTATION.

Uaktualniono angielski wstęp docs/COMMUNITY_IDENTITY_DEVELOPER.md: CommunityPanel istnieje, używa natywnego community_dispatch i jawnego status/progress/recovery. Poprawiono zdanie historycznego kroku 2B-5.3.6 o braku ekranu. Wyraźnie oddzielono obecną implementację od wcześniejszych zapisów etapów.

Ogólny plik bieżącego planu zsynchronizowano z treścią 1.94 i dodano wersjonowany plik 1.94. Oba zawierają pełną, niezmienioną treść 1.93; zachowano wszystkie historyczne decyzje, w tym obowiązkowy przegląd OSS przed MCP i odłożenie przeglądarki. Poprzednie pliki wersjonowane i ZIP-y nie są przepisywane.

Nie zmieniono kodu Rust/TypeScript, UI, testów, kontraktów, zależności, migracji, permissions ani workflow. Nie uruchamiano ponownie testów runtime dla tej korekty dokumentacyjnej. Wcześniejsze wyniki audytu nie są przedstawiane jako nowe wykonanie.

Zweryfikowano bazę SHA-256 58ccefae8b4512016aa23b2a4373f540a9c1bb93da5746ed50ef2593adcb62c0, brak zmian poza dokumentami i ich sumami, zachowanie historii 1.93 oraz byte-exact BASE + overlay = SOURCE. Zaktualizowano pełny manifest źródeł i SHA256SUMS, aby uniknąć mieszanego drzewa.

Nałożyć CHANGED_FILES na dokładny SOURCE 2B-5.3.7; SOURCE z tej korekty jest pełną alternatywą. Nie nakładać obu źródeł jednocześnie. Zweryfikować scripts/verify-source.py i wykonać zwykły commit/push/CI. Bez nowego release.

Nowe natywne CI i rzeczywisty odbiór desktop/Windows/WebView/OS vault/forum/PHP/TLS nadal wymagane. COMMUNITY_DESKTOP_ACCEPTANCE_PL.md pozostaje procedurą NOT EXECUTED. Nie zamknięto 2B-5.3, 2B ani 4B-12. Główny dokument architektury 4B nie wymaga zmiany — korekta usuwa wyłącznie rozbieżność bieżących zapisów wskazaną w D1.
