# AI Home 4B-12/13 — Paczka 4A: Granica klienta MCP (`McpClientPort`) i lokalny prototyp fake (Streamable HTTP)

Data: 2026-11-27  
Baza repozytorium: `92ab720` (`main`)  
Tryb weryfikacji: Linux headless (`cargo test --no-default-features --locked`, `npm test`, `npm run build`, wszystkie skrypty `verify-*`)

## 1. Zakres zmian (Paczka 4A — kumulatywnie nad 3D Fix 1)

Zgodnie z przeglądem OSS ([`docs/AI_HOME_4B12_MCP_CLIENT_OSS_REVIEW_PL.md`](AI_HOME_4B12_MCP_CLIENT_OSS_REVIEW_PL.md), sekcje 3–4) oraz wpisami 1.88/1.89:
- **Zero nowych zależności zewnętrznych w `Cargo.toml` / `Cargo.lock`** — pełna zgodność z `--locked` i brak przedwczesnego wciągania zewnętrznego SDK przed ostateczną decyzją odbiorczą.
- Zaimplementowano warstwę izolacji [`src-tauri/src/mcp_client.rs`](../src-tauri/src/mcp_client.rs) (eksportowaną z [`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs)):
  1. **`McpClientPort`** — wewnętrzny trait izolujący AI Home od konkretnego SDK/transportu (w przyszłości można podpiąć `rmcp` 3.11.0 pod ten sam port bez zmian w politykach, `Vault`, UI czy `kernel.rs`).
  2. **`McpServerBinding` + `Vault` `ScopeStamp`** — walidacja adresu (`https://` lub jawny loopback `http://127.0.0.1`/`localhost`/`[::1]`, brak userinfo i znaków kontrolnych) oraz ścisła izolacja sekretów per `(profile_id, service_id, normalized_endpoint)`.
  3. **Ochrona przed wyciekiem sekretu na redirectach** — `reqwest::redirect::Policy::none()` + natychmiastowe odrzucenie kodów 3xx jako `McpError::RedirectBlocked`.
  4. **`McpToolPolicy` + `sanitize_tool_descriptor`** — natywna per-profilowa allowlista narzędzi, rozróżnienie `read_only` / `allow_mutating`, bezwzględne odrzucanie prób `self-grant` w metadanych narzędzia i argumentach (`untrusted_external_metadata: true`) oraz blokada automatycznej publikacji prywatnych refleksji (`experimental_private_reflections`, `experimental_self_records`, `private_scratchpad`).
  5. **Obsługa niepewnego skutku (`Uncertain`)** — przerwanie połączenia/timeout na narzędziu mutującym zwraca `McpToolCallOutcome::Uncertain` -> `ActionOutcome::Uncertain`, aktywując blokadę `recovery_required` / `reconciliation_required` w `ExperimentalRuntime` bez ślepego ponowienia.
  6. **Brak procesów `stdio` / shell** w tej fazie (zero `std::process` / `tokio::process`).

## 2. Testy odbioru prototypu (`src-tauri/tests/mcp_client.rs` — 6/6 PASS)

1. `mcp_streamable_http_initialize_list_and_call_tool_succeeds_against_local_fake_server` — pełny przebieg `initialize` -> `tools/list` -> `tools/call` przeciwko lokalnemu fake serwerowi MCP na `127.0.0.1:0`.
2. `mcp_isolates_vault_credentials_between_two_profiles_and_two_services` — brak możliwości użycia poświadczenia innego profilu lub innej usługi.
3. `mcp_rejects_self_grant_from_tool_metadata_and_enforces_allowlist_and_read_only` — odrzucenie metadanych `self-grant`, narzędzi spoza allowlisty i mutacji w trybie `read_only`.
4. `mcp_blocks_http_redirect_so_secret_never_leaks_to_foreign_host` — HTTP 302 na obcy serwer zablokowany (`RedirectBlocked`), obcy host otrzymuje 0 żądań.
5. `mcp_mutating_disconnect_records_uncertain_and_blocks_restart_until_reconciled` — zerwanie połączenia na narzędziu mutującym zapisuje `uncertain`, utrzymuje `reconciliation_required` po restarcie bazy SQLite i odblokowuje dopiero po jawnym `autonomous.action.reconcile`.
6. `mcp_respects_autonomous_refusal_and_sleep_and_blocks_private_reflection_publication` — aktywna odmowa i sen AI blokują wykonanie narzędzia, a źródła prywatnej refleksji są odrzucane przez politykę.

Dodano także statyczny strażnik [`scripts/verify-mcp-client.py`](../scripts/verify-mcp-client.py) (9/9 kontroli).

## 3. Otwarte bramki zewnętrzne (OPEN)

- **Próby na realnym Windows** (WebView2, OS Vault, żywy klucz API) — odłożone na koniec zgodnie z ustaleniem użytkownika.
- **Decyzja odbiorcza nt. włączenia `rmcp` 3.11.0** vs pozostawienia lekkiego transportu `StreamableHttpMcpClient` pod `McpClientPort` (wraz z ewentualną aktualizacją `Cargo.lock` i wpisem w Third-Party Notices).
- **Community 4A live forum/TLS** — akceptacja wobec zewnętrznego forum pozostaje OPEN.
