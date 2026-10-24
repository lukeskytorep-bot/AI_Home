# AI Home — 4B-9 EXP-0 AUDIT FIX — raport implementacyjny

Data: 24.10.2026.

## Podstawa

Baza: dokładny `AI_HOME_4B9_EXP0_SOURCE.zip`. Poprawka odpowiada wyłącznie na E1/E2 z głównego audytu EXP-0. EXP-1 nie został rozpoczęty.

## E1 — lifecycle permanent deletion

- `profile.delete.permanent` odczytuje status powiązanej `experimental_instances` przed usuwaniem.
- Jeżeli istnieje instance w stanie innym niż `inactive`, operacja zwraca `POLICY_DENIED`. Nie ma wymuszonego shutdownu, zmiany statusu ani obejścia dobrowolnego lifecycle Experimental.
- Dla `inactive` jawnie usuwane jest całe drzewo dziewięciu tabel `experimental_*` w kolejności respektującej FK: semantic checkpoints, technical checkpoints, event cursors, actions, refusals, decisions, wake plans, work episodes, instances.
- Dotychczasowy `PRAGMA foreign_key_check` pozostaje końcową kontrolą transakcji.
- Dodano native regression: archived profile + pełne inactive Experimental tree -> permanent delete -> zero zależnych rekordów, czyste FK, drugi profil nienaruszony.
- Dodano native regression: archived profile + `running` Experimental instance -> `POLICY_DENIED`, profil i instance pozostają.

## E2 — ActionRecord evidence semantics

`ActionRecord` jest teraz `oneOf` z trzema wariantami:

1. `prepared | sent | failed | uncertain | cancelled` — receipt może być pusty;
2. `accepted` — `receiptRef` ma `minLength: 1`;
3. `reconciled` — `receiptRef` ma `minLength: 1` i obowiązkowe `reconciliationOutcome`:
   - `confirmed_accepted`,
   - `confirmed_not_executed`,
   - `still_unknown`.

To zamraża zasadę, że `accepted` nigdy nie istnieje bez referencji do dowodu, a reconciliation nie jest synonimem sukcesu. Warstwa domenowa przyszłego executora nadal będzie musiała sprawdzić istnienie/rodzaj/powiązanie wskazanego dowodu; sam EXP-0 nie wykonuje connectorów.

Dodano zgodne regresje Rust + TypeScript dla accepted bez/ze receipt oraz reconciled bez/ze outcome.

## Granice

Nie dodano live schedulera, start/impulse IPC, ExperimentalDecisionService, provider calls, ConsciousCore, MCP, Community, Messenger ani A2A. SQLite pozostaje v13. Frozen P2 nie został zmieniony.

## Lokalne bramki wykonane

- boundaries: 31/31 PASS;
- command parity: 54/54 PASS;
- Rust source-text: PASS;
- Standard closure: 8/8 PASS;
- Standard release: 8/8 PASS;
- Experimental foundation: 12/12 PASS.

Pełne Cargo/Vitest/typecheck/Vite nie były wykonywane lokalnie z powodu braku Rust toolchain i `node_modules`. Wymagane pełne GitHub Actions.
