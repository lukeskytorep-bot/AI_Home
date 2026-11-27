# AI Home 4B-12/13 — Paczka 4B: Podpięcie klienta MCP do SQLite (v18), IPC i UI (`Settings`)

Data: 2026-11-27  
Baza repozytorium: `92ab720` (`main`)  
Tryb weryfikacji: Linux headless (`npm test`, `npm run build`, `check_contracts.py`, `verify-*.py`/`mjs`, `cargo test --no-default-features --locked`)

## 1. Zakres zmian (Paczka 4B — kumulatywnie nad 4A Fix 1)

1. **Migracja SQLite v18 (`mcp_servers`) i trwałe usuwanie profilu**
   - W [`src-tauri/src/persistence.rs`](../src-tauri/src/persistence.rs) dodano tabelę `mcp_servers(profile_id, id, revision, body)` (`PRIMARY KEY(profile_id, id)`, `FOREIGN KEY(profile_id) REFERENCES profiles(id)`) i podbito `CURRENT_SCHEMA` / `PRAGMA user_version` do `18`.
   - W [`src-tauri/src/profile_deletion.rs`](../src-tauri/src/profile_deletion.rs) dodano `mcp_servers` do `delete_rows` — trwałe usunięcie profilu usuwa wszystkie powiązane serwery MCP i tombstonuje wszystkie `receipts` z komend `mcp.*`.

2. **Kontrakt IPC [`contracts/mcp.ipc.json`](../contracts/mcp.ipc.json) i usługa `McpService`**
   - Dodano 5 komend IPC (`mcp.server.list`, `mcp.server.save`, `mcp.server.delete`, `mcp.server.discover`, `mcp.tool.call`) w [`src-tauri/src/mcp_client.rs`](../src-tauri/src/mcp_client.rs) i podpięto routing w [`src-tauri/src/application.rs`](../src-tauri/src/application.rs) (wraz z ochroną `restore_pending_for_store` i parytetem **80/80** w `verify-command-parity.py`).
   - Pełna idempotencja `receipts` (koperta IPC kompatybilna z tombstoningiem, bez zapisywania sekretu `apiKey` w paragonach — haszowany w fingerprincie).
   - Zmiana `endpoint` bez podania nowego `apiKey` czyści stare powiązanie poświadczenia (sekret nigdy nie przechodzi na nowy host).
   - `mcp.server.discover` uruchamia `initialize` + `tools/list` z politykami 4A Fix 1 (bez automatycznego dodawania narzędzi do allowlisty).
   - `mcp.tool.call` egzekwuje natywną allowlistę, `allowMutating`, zakaz `self-grant`, zakaz publikacji prywatnych refleksji oraz bramki stanu Autonomous (pauza `inactive` i `reconciliation_required`).
   - **Zero nowych zależności w `Cargo.toml` / `Cargo.lock`** (pełna zgodność z `--locked`).

3. **UI `McpPanel` w Settings i lokalizacja EN/PL**
   - Nowy komponent [`src/ui/McpPanel.tsx`](../src/ui/McpPanel.tsx) podpięty w [`src/ui/App.tsx`](../src/ui/App.tsx) (sekcja Settings), obsługujący profile Standard i Autonomous: dodawanie serwera MCP (token w uncontrolled `ref` czyszczony przy zapisie), `Discover tools`, jawne `Allow tool` / `Revoke tool`, `Remove`.
   - Symetryczne klucze EN/PL w [`src/locales/en.json`](../src/locales/en.json) i [`src/locales/pl.json`](../src/locales/pl.json).

## 2. Testy i weryfikacja

- Rust (`src-tauri/tests/mcp_client.rs`): **11/11 PASS** (w tym 2 nowe testy regresyjne Fix 1: trwała blokada `pendingReconciliation` po restarcie + egzekwowanie `declaredMutating` oraz blokada `mcp.tool.call` podczas snu AI).
- Frontend (`tests/mcp-panel.test.tsx` + pełne `npm test`): **27/27 plików, 253/253 testów PASS**, `npm run build` PASS.
- Kontrakty i strażniki: `check_contracts.py` (193/193), `verify-boundaries.mjs` (42/42 + 14 asercji v18), `verify-command-parity.py` (**80/80**), `verify-mcp-client.py` (9/9), wszystkie pozostałe skrypty `verify-*` oraz `scripts/verify-source.py`.
- Zamrożone kontrakty P2 (`contracts/p2-frozen.sha256.json`), `src-tauri/capabilities/main.json` oraz `Cargo.toml`/`Cargo.lock` pozostały **nienaruszone**.

## 3. Poprawki po audycie Paczki 4B (Fix 1)

- **`F1 / P2` (trwała blokada po `Uncertain`, `declaredMutating` oraz sen AI w `mcp.tool.call`):**
  - `call_tool_ipc` wylicza `mutating = payload.mutating || discovered.declaredMutating`, uniemożliwiając ominięcie polityki `allowMutating: false`.
  - Wynik `McpToolCallOutcome::Uncertain` trwale zapisuje `pendingReconciliation` w `mcp_servers` (a dla profilu Autonomous także `recovery_required` w `experimental_instances`), blokując kolejne wywołania z nowym `requestId` także po restarcie SQLite — do czasu jawnego uzgodnienia (`reconcileReceiptRef` w `mcp.server.save`).
  - `autonomous_execution_fence` sprawdza aktywny `WakePlan` z `origin == "ai_decision"` (`deadline` w przyszłości lub `event_only`) i blokuje `mcp.tool.call` podczas snu AI.
- **`P3-1` (UX w `McpPanel.tsx`):**
  - Lista wykrytych narzędzi wyświetla etykietę `(read-only)` / `(mutating)` obok nazwy narzędzia oraz baner uzgodnienia, gdy `pendingReconciliation` jest aktywne.
