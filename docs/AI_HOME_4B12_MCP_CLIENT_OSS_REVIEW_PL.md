# 4B-12/13 — Obowiązkowy przegląd OSS klienta MCP (bramka z wpisu 1.89 / sekcji 5.3 planu 1.99)

Data oceny: **25 listopada 2026**.  
Status bramki: **PRZEGLĄD DOKUMENTACYJNY WYKONANY / OCZEKUJE AUDYTU I LOKALNEJ PRÓBY FAKE PRZED KODOWANIEM MCP**.  
Zakres: wyłącznie **klient MCP** w AI Home (własne forum Community 4A nie wymaga serwera MCP). W tej paczce **nie dodano żadnych zależności do `Cargo.toml`/`Cargo.lock` ani produkcyjnego kodu MCP**.

---

## 1. Zestawienie kandydatów i macierz porównawcza

Oceniono dwa aktywnie utrzymywane SDK open source dla Rust oraz minimalny wariant własny jako punkt odniesienia:

| Kryterium | `rmcp` (oficjalny Rust SDK MCP) | `rust-mcp-sdk` (społecznościowy Rust SDK) | Minimalny klient własny (JSON-RPC 2.0) |
| --- | --- | --- | --- |
| Repozytorium | `https://github.com/modelcontextprotocol/rust-sdk` | `https://github.com/rust-mcp-stack/rust-mcp-sdk` | Wewnętrzny moduł w `src-tauri/src/` |
| Oceniana wersja | `rmcp` **3.11.0** (crates.io / docs.rs, stan 11.2026) | `rust-mcp-sdk` **2.0.0** (opubl. 2026-08-27) | Brak (punkt odniesienia) |
| Licencja i dystrybucja | **MIT OR Apache-2.0** (kompatybilna z MIT AI Home; wymagany wpis w Third-Party Notices / Credits) | **MIT** (kompatybilna z MIT AI Home; wymagany wpis w Third-Party Notices / Credits) | MIT (kod własny AI Home) |
| Specyfikacja MCP | Stabilna **2026-07-28** + wsteczna kompatybilność z `2025-11-25` i starszymi; automatyczna negocjacja wersji | **2026-07-28** (2.x) / `2025-11-25` (1.x); deklarowane 100% testów zgodności klienta i serwera | Tylko ręcznie utrzymywany podzbiór (`initialize`, `tools/list`, `tools/call`) |
| Zgodność z Rust / Tauri 2 / `tokio` | Natywny `tokio`, oddzielne feature flags klienta (`client`, `transport-child-process`, `transport-streamable-http-client`) | Natywny `tokio`, granularne feature flags (`client`, `stdio`, `streamable-http`, `sse`, `tls-no-provider`) | Pełna kontrola nad `reqwest` / `tokio::process` |
| Windows / Linux | Testowane w CI na Linux, macOS i Windows | Wspiera Linux, macOS i Windows | Wymaga ręcznego testowania procesów `stdio` na Windows (Job Objects / uchwyty) |
| Transporty lokalne i zdalne | `stdio` (proces potomny), `Streamable HTTP`, subskrypcje i zadania długotrwałe | `stdio`, `Streamable HTTP`, wsteczny `SSE` | Wymaga własnej implementacji framingu `stdio` i `Streamable HTTP` |
| Uwierzytelnianie i poświadczenia | Obsługa nagłówków / OAuth po stronie klienta; sekret przekazywany z zewnętrznego `Vault` AI Home | Wbudowane helpery OAuth (PKCE / refresh) + własne nagłówki | Własne nagłówki z `Vault` |
| Izolacja profili, usług i `Vault` | Zapewniana przez adapter AI Home (osobna instancja klienta i `ScopeStamp` na parę `(profile_id, service_id)`) | Zapewniana przez adapter AI Home (osobna instancja na `(profile_id, service_id)`) | Zapewniana przez adapter AI Home |
| Anulowanie i recovery | Integracja z `tokio` / tokenami anulowania; wynik niepewny musi pozostać `uncertain` w ledgerze AI Home | Integracja z `tokio`; wynik niepewny musi pozostać `uncertain` w ledgerze AI Home | Pełna kontrola nad `CallBudget` / `Cancellation` z `providers/transport.rs` |
| Zależności i rozmiar binarki | Średnie (z wyłączonymi feature serwera i makr `rmcp-macros` rozmiar pozostaje umiarkowany) | Większy zestaw pomocniczych crate'ów (`rust-mcp-schema`, `rust-mcp-transport`, opcjonalnie `axum`/`actix`) | Minimalny (wykorzystuje istniejący `reqwest` i `serde_json`) |
| Utrzymanie i ryzyko | **Najniższe ryzyko długoterminowe** — oficjalne repozytorium organizacji `modelcontextprotocol`, referencyjne aktualizacje protokołu | Aktywne i kompletne, ale utrzymywane poza organizacją `modelcontextprotocol` (wyższe ryzyko rozjazdu API między 1.x/2.x) | Wysoki koszt własnego utrzymania przy zmianach specyfikacji MCP |
| Koszty | 0 USD (OSS, brak płatnych usług chmurowych) | 0 USD (OSS, brak płatnych usług chmurowych) | 0 USD, ale najwyższy koszt czasu utrzymania |

---

## 2. Rekomendacja wyboru i kompromisy

**Rekomendowany kandydat:** **`rmcp` (`modelcontextprotocol/rust-sdk`, seria 3.x — oceniana wersja `3.11.0`)**, włączany **wyłącznie z feature klienta** (bez serwera, bez makr serwerowych, z przypiętą wersją w `Cargo.lock`).

**Uzasadnienie:**
1. Jest oficjalnym Rust SDK organizacji `modelcontextprotocol`, wspiera zarówno najnowszą stabilną specyfikację `2026-07-28`, jak i `2025-11-25`, oraz automatycznie negocjuje wersję protokołu z serwerami.
2. Licencja **MIT OR Apache-2.0** jest w pełni zgodna z licencją MIT AI Home (wymaga wyłącznie dopisania atrybucji i tekstu licencji do `Third-Party Notices` / `Credits` przed pierwszą dystrybucją z tym crate'em).
3. Zgodnie z wpisem 1.89 preferujemy ponowne użycie utrzymywanego komponentu zamiast pisania własnego parsera i transportu MCP od zera.

**Kompromisy i ryzyka do opanowania:**
- Zależność `rmcp` może wciągać własne typy transportu HTTP (`reqwest`) — podczas próby prototypowej należy zweryfikować brak konfliktu wersji `reqwest` / `rustls` z obecnym `src-tauri/Cargo.toml` oraz wyłączyć domyślne redirecty, aby nigdy nie wysłać sekretu do obcego hosta.
- Transport lokalny `stdio` uruchamia proces potomny — w AI Home musi być domyślnie **wyłączony** i dopuszczony wyłącznie po jawnej konfiguracji użytkownika (allowlista binarnych ścieżek, brak dowolnego shella, twarde czyszczenie zmiennych środowiskowych, żeby proces potomny nie odziedziczył obcych sekretów).

---

## 3. Granica integracji i niezmienniki AI Home (Standard + Autonomous)

Niezależnie od wybranego SDK, klient MCP w AI Home zostanie odizolowany za własnym traitem wewnętrznym `McpClientPort` (nigdy nie wystawiamy typów SDK bezpośrednio do UI, IPC ani `kernel.rs`):

1. **Brak self-grant w skillach i treściach zewnętrznych:** definicja narzędzia zwrócona przez serwer MCP lub treść skilla nigdy nie nadaje uprawnień ani nie staje się instrukcją systemową. Każde wywołanie przechodzi natywną kontrolę `(profile_id, direction, capability, service_id, destination)`.
2. **Izolacja poświadczeń:** tokeny/klucze dla serwerów MCP są przechowywane wyłącznie w `Vault` pod oddzielnym `ScopeStamp` per `(profile_id, service_id)`. Zmiana hosta lub przekierowanie HTTP nigdy nie przenosi sekretu.
3. **Brak automatycznej publikacji prywatnej pamięci:** prywatne notatki, rozmowy oraz prywatne refleksje Autonomous (`experimental_private_reflections`, `experimental_self_records`) nie są przekazywane do narzędzi MCP bez jawnej akcji/decyzji.
4. **Poszanowanie Autonomous (refusal / sleep / uncertain):**
   - Aktywna odmowa (`RefusalRecord`) i wybrany sen (`WakePlan`) mają pierwszeństwo przed jakąkolwiek sugestią użycia narzędzia MCP.
   - Błąd narzędzia MCP, timeout lub odmowa serwera są stanami wykonania (`execution_failed` / `uncertain`), nigdy odmową agenta.
   - Niepewny skutek wywołania narzędzia mutującego zapisuje stan `uncertain` w ledgerze akcji i aktywuje blokadę `reconciliation_required` (bez ślepego ponowienia).

---

## 4. Plan minimalnej próby na lokalnym fake i testy odbioru (warunek przed kodowaniem produkcyjnym)

Zgodnie z wpisem 1.89 przed włączeniem `rmcp` do produkcyjnego `src-tauri/Cargo.toml` należy wykonać:

1. **Minimalny prototyp na lokalnym fake serwerze MCP (`Streamable HTTP` na `127.0.0.1:0`):**
   - `initialize` → `tools/list` → `tools/call` (sukces, błąd narzędzia, timeout/zerwanie połączenia → `uncertain`).
   - Weryfikacja kompilacji z `--locked` i sprawdzenie drzewa zależności (`cargo tree`) pod kątem licencji i duplikatów `reqwest`/`tokio`.
2. **Obowiązkowe testy odbioru (z wpisu 1.88 i 1.89):**
   - izolacja dwóch profili i dwóch usług MCP (brak wycieku tokena między profilami/usługami),
   - odrzucenie próby `self-grant` z opisu narzędzia/skilla,
   - ochrona sekretu przy zmianie celu / przekierowaniu HTTP (`redirect::Policy::none()`),
   - restart z niepewnym wywołaniem narzędzia (`uncertain` → `reconciliation_required`, brak ślepego retry),
   - brak automatycznej publikacji prywatnej pamięci oraz zachowanie odmowy i snu w trybie Autonomous.
