# AI Home — 4B-4 krok 4 AUDIT FIX

22 października 2026. Baza: `AI_HOME_4B4_KROK_4_SOURCE.zip`, SHA-256 `72fbdb8c85f4ff4aa2148687dcab3d5ade7f3e4d3188e97f659f675aacaa809c`. Korekta odpowiada niezależnemu audytowi U1–U4. Nie zawiera 4B-5.

## U1 — preflight po zmianie profile context

`AttachmentsPanel` przekazuje profile-scoped zmianę do rodzica po Accepted profilu importu i po zmianie enable knowledge/guidance. `Conversations` utrzymuje `profileContextRevision` niezależny od conversation remount i włącza go do `memoryContextKey` przekazywanego do `ChatPanel`. Istniejący mechanizm `preflightKey` usuwa wtedy widoczny preflight i ignoruje spóźniony rezultat starego klucza. Native nadal ponownie składa kontekst przed send/POST/commit.

## U2 — śledzenie memory candidate

`candidateOperation` ma osobny polling `operation.get` do terminal state, odświeża profile/memory po zakończeniu, pokazuje błąd i posiada STOP przez istniejący profile-scoped `operation.cancel`. Sam UI nadal nie może ustawić ACCEPT/MODIFY/REJECT.

## U3 — więcej niż 100 rekordów

`loadMemoryRecords()` przechodzi po `nextCursor` do końca z ochroną przed zapętlonym cursorem. Nie ma już niewidocznej granicy pierwszych 100 rekordów.

## U4 — profilowy import po zmianie rozmowy

Dodano własny suplement `profile.import.latest`. Native wybiera ostatnią operację importu posiadającą `profile_import_materials` w bieżącym profilu. `AttachmentsPanel.latest()` uzgadnia latest conversation import i latest profile import, preferuje operację nadal queued/running i dzięki temu po remount odzyskuje status i STOP.

## Testy dodane

- UI: paginacja >100 rekordów.
- UI: odzyskanie running profile import i STOP po remount.
- UI: callback zmiany profile knowledge do unieważnienia preflight.
- UI: polling kandydatury pamięci do terminal Primary operation.
- Rust: `profile_import_latest_recovers_profile_scoped_operation`.

Pełna ścieżka modelowa memory candidate, knowledge/guidance POST/commit i migracja 10→11 pozostają objęte istniejącymi integracjami i powinny zostać ponownie wykonane w pełnym CI zgodnie z audytem. Ta korekta nie deklaruje runtime PASS przed nowym CI/audytem.

## Plan v1.4

Plan etapów podniesiono do treści 1.32 i dodano nadrzędną erratę: Standard/Experimental są odrębnymi direction-bound instancjami zgodnie z Document 4B v1.4; historyczne Governor/Pause Autonomy/Conscious nie są wymaganiami dla przyszłych etapów Experimental.

## Status

**IMPLEMENTED — REQUIRES NEW CI AND RE-AUDIT. 4B-4 pozostaje otwarte do PASS korekty. Nie rozpoczynać 4B-5 na tej linii przed odbiorem.**
