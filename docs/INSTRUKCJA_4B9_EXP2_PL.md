# Instrukcja — AI Home 4B-9 EXP-2

## Baza
Nakładać wyłącznie na dokładny `AI_HOME_4B9_EXP1_AUDIT_FIX_3_SOURCE.zip` o SHA-256:
`ce1f02c5af5bc8773df8663c9e8c02eb62467ecbdeccca2f04a738fa4ade1097`.

## Overlay
Użyć `AI_HOME_4B9_EXP2_CHANGED_FILES.zip` albo zastąpić repo pełnym `AI_HOME_4B9_EXP2_SOURCE.zip`.

## Po nałożeniu uruchomić
1. pełny GitHub Actions / Rust tests;
2. `npm test`;
3. `npm run build`;
4. `python3 check_contracts.py`;
5. `npm run verify:boundaries`;
6. `npm run verify:command-parity`;
7. `npm run verify:rust-source-text`;
8. `npm run verify:standard`;
9. `npm run verify:standard-release`;
10. `npm run verify:experimental-foundation`;
11. `npm run verify:experimental-runtime`;
12. `npm run verify:experimental-continuity`;
13. `python3 scripts/verify-source.py .`.

## Krytyczne scenariusze Rust
Szczególnie sprawdzić `src-tauri/tests/experimental_continuity.rs` oraz test deletion source refs. EXP-2 nie jest zaakceptowane tylko dlatego, że statyczne 20/20 jest zielone.

## Zakres, którego nie rozszerzać przy poprawkach
Nie dodawać ConsciousCore, live Experimental IPC, production background scheduler, MCP, forum, Messenger ani A2A. Ewentualne poprawki po audycie mają pozostać w Work Continuity / context rollover / recovery.
