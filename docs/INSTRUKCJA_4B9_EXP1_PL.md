# Instrukcja — AI Home 4B-9 EXP-1

## Baza

Nakładać wyłącznie na pełne źródło:

`AI_HOME_4B9_EXP0_AUDIT_FIX_SOURCE.zip`

## Zakres overlay

Overlay zawiera EXP-1 deterministic runtime, testy, verifier, evidence oraz aktualizację planu. Nie usuwa plików.

## Po nałożeniu

Uruchomić pełne GitHub Actions, w szczególności:

```text
npm test
npm run build
npm run verify:boundaries
npm run verify:command-parity
npm run verify:standard
npm run verify:standard-release
npm run verify:experimental-foundation
npm run verify:experimental-runtime
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked
```

## Oczekiwany zakres testów EXP-1

- brak decision call przed due time;
- no-action po due i brak natychmiastowego kolejnego call;
- AI sleep respektowany mimo niekwalifikującego eventu;
- event-only budzi tylko wybrany event;
- refusal trwa i jest widoczny w późniejszym packet;
- fake peer action kończy się receipt;
- restart zamienia running na interrupted i sent na uncertain;
- resource failure nie jest refusal;
- missed interval daje jedną okazję, nie lawinę calls.

Nie rozpoczynać EXP-2 przed zielonym CI i audytem EXP-1.
