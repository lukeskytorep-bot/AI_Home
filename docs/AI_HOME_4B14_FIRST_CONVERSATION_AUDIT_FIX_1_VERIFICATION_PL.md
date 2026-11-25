# 4B-14 Pierwsza rozmowa (Audit Fix 1) — weryfikacja commita `92ab720`

Baza: commit `92ab72053c9c0b3e9e42bb945ad8931943edf1fa` (`4b14`), data sprawdzenia: 25.11.2026.

## 1. Status ustaleń P2 i rekonstrukcji (plan 1.98, sekcja 2)

1. **P2 #1 — korekta adresu Custom OpenAI:** ZAMKNIĘTE W KODZIE (w zakresie testów lokalnych).
   - `src/ui/firstConversation.ts` wykrywa zmianę `normalizedEndpoint`, wymaga świeżego klucza, wywołuje `connection.update` na bieżącej rewizji, następnie `connection.credential.set` na podbitej rewizji i ponownie uruchamia `connection.test`.
   - `src-tauri/src/connections.rs` przy `connection.update` unieważnia status połączenia (`unverified`) i zwiększa rewizję; normalizacja URL w JS i w `src-tauri/src/providers/adapters.rs` jest spójna.
   - Pokryte regresją w `tests/first-conversation.test.tsx` (zachowany jeden profil, jedno połączenie i jedna pierwsza rozmowa po restarcie).
2. **P2 #2 — stary profil `draft` (`Finish setup`):** ZAMKNIĘTE W KODZIE (w zakresie testów lokalnych).
   - `src/ui/Conversations.tsx` wyświetla jawny przycisk `Finish setup` dla profili `draft`, a `prepareExistingSetup` w `src/ui/firstConversation.ts` otwiera kreator na istniejącym profilu bez tworzenia profilu zastępczego i odmawia (`onboarding.existingConfig`), jeśli profil ma bardziej złożoną konfigurację.
   - Gotowe profile (`ready`) otwierają przestrzeń roboczą bez ponownego kreatora i bez zapisów konfiguracyjnych.
3. **Rekonstrukcja źródła (`src-tauri/tests/backup.rs`):** ZGODNE.
   - `sha256sum src-tauri/tests/backup.rs` = `a25ddce38f6821e021d43611c1be19ed8ffcd2cf6d0b2185e793f328dcc4beab` (zgodny co do bajtu z oczekiwanym hashem z planu 1.98; wcześniejsza rozbieżność wynikała z pominięcia commita `96ccb61` „Windows CI Fix 3”).
   - `python3 scripts/verify-source.py` na `92ab720`: PASS (1076 hashy manifestu, 1077 wpisów `SHA256SUMS.txt`).

## 2. Wyniki własnej walidacji (Linux, 25.11.2026)

Środowisko: Linux, Node.js `v22.23.5`, npm `12.2.2`, Rust `1.100.0`, Python `3.12.16`.

- `npm ci && npm test`: **24/24 plików, 241/241 testów PASS** (w tym 6/6 w `tests/first-conversation.test.tsx`).
- `npm run build` (`tsc --noEmit && vite build`): **PASS**.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-default-features --locked` (Linux headless, wszystkie 23 zestawy): **662/662 PASS**, 0 failed, 0 ignored (`lib`: 319, `backup`: 7, `chat`: 36, `chat_memory`: 19, `community_host`: 13, `connections`: 24, `conversations`: 10, `diagnostics`: 1, `experimental_continuity`: 10, `experimental_contracts`: 3, `experimental_internal_state`: 5, `experimental_reflection`: 26, `experimental_runtime`: 18, `export`: 4, `foundation`: 8, `imports`: 23, `lifecycle`: 10, `memory`: 33, `memory_history`: 14, `memory_layers`: 25, `memory_restore`: 22, `memory_runtime`: 30, `standard_closure`: 2).
- `python3 check_contracts.py`: **193/193 PASS**.
- `node scripts/verify-boundaries.mjs`: **38/38 + 14 asercji migracji SQLite v16 PASS**.
- `python3 scripts/verify-command-parity.py`: **64/64 PASS**.
- `python3 scripts/verify-rust-source-text.py`: **PASS**.
- Wszystkie 9 skryptów `verify-standard-*`, `verify-experimental-*`, `verify-community-*`: **PASS**.

## 3. Checklista UX (sekcja 3 planu 1.98) i uwagi P3

- Settings na górnym pasku, lewa nawigacja, pełna szerokość rozmowy, zwijany panel kontekstu, wygląd/dostawcy/pamięć+Scribe/kopie/eksport/diagnostyka+About & Privacy w Settings: **obecne**.
- Uwagi nieblokujące (P3 / dalszy zakres):
  1. W `src/ui/App.tsx` brak przycisku anulowania/powrotu po kliknięciu `Finish setup`, a `onOpen` nie zeruje `initial` (`setInitial(undefined)`).
  2. Okno kontekstu rozmowy (`chat.settings.set`, tabela `chat_policies`) i Vision są dostępne w zwijanej sekcji w `ChatPanel.tsx`, osobno od `memory_contexts` w `Settings → Memory and Scribe`.
  3. Budżety startowe w `src/ui/firstConversation.ts` pozostają stałe (`outputLimitTokens: 1024`, `contextTokens: 8192`), ponieważ `connection.models.list` nie pobiera jeszcze rzeczywistych limitów modelu z API dostawcy.
  4. Walidacja formularza w `src/ui/FirstConversation.tsx` akceptuje dowolne `http://`, podczas gdy backend w `src-tauri/src/providers/adapters.rs` (słusznie) ogranicza `http://` wyłącznie do `localhost`/`127.0.0.1`/`[::1]`.

## 4. Granice weryfikacji (OPEN)

- **NIE WYKONANO** w tym środowisku: pełnej kompilacji Tauri z default features (`webkit2gtk-4.1` niedostępne w VM), nowego przebiegu **Windows CI (`build_desktop=true`)**, rzeczywistej próby użytkownika na Windows/WebView2 z prawdziwym kluczem dostawcy, OS vault oraz odbioru live forum Community 4A / TLS. Te bramki pozostają **OPEN**.
