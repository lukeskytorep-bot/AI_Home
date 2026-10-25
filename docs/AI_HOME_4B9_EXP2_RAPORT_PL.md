# AI Home — 4B-9 EXP-2 — Work Continuity / Context Rollover — raport PL

Data: 25.10.2026

## Cel
Po zielonym CI dla `AI_HOME_4B9_EXP1_AUDIT_FIX_3_SOURCE.zip` wdrożono wyłącznie EXP-2: trwałą ciągłość pracy Experimental pomiędzy context windows, restartami i zmianami trasy/modelu. Nie dodano ConsciousCore, produkcyjnego schedulera desktopowego, live Experimental IPC, MCP, Community, Messenger ani A2A.

## Zrealizowane
- nowy `experimental_continuity` z `WorkContinuityService`;
- `WorkEpisode` jako nadrzędna jednostka rozpoczętej pracy;
- osobne zapisy `TechnicalWorkCheckpoint` i `SemanticWorkCheckpoint`;
- CAS przez `parentCheckpointId`; spóźniony/stary checkpoint nie nadpisuje nowszego stanu;
- semantic checkpoint musi wskazywać najnowszy technical checkpoint danego Episode;
- `sourceEventSequence` technical checkpointu nie może wyprzedzić trwałego `committedThrough` event cursora;
- `ResumePacket` składa trwały Episode, najnowszy technical checkpoint, właściwy semantic checkpoint, aktywne refusal, unresolved `sent/uncertain` actions i committed event sequence;
- realny `OpportunityPacket` dostaje `resume_packet`, jeśli istnieje aktywna/waiting/interrupted praca z checkpointem;
- błąd odczytu continuity po claimie nie zostawia instancji `running`, tylko korzysta z istniejącej technicznej finalizacji/retry gate;
- deterministic context preflight rozróżnia `Fits`, `CheckpointRequired` i `Impossible`;
- preflight rezerwuje osobno output, input reserve i checkpoint reserve; measurement jest jawnie `caller_estimated_tokens`, więc system nie udaje dokładnego tokenizera;
- zmiana route/context window wymaga nowego preflight;
- duże tool results są reprezentowane przez `ExperimentalSourceRef` (`tool_result` + hash/ref), nie kopiowane do Work Continuity;
- permanent deletion rozmowy/wiadomości oznacza odpowiadające checkpoint source refs jako `permanently_deleted`;
- source-ref deletion ma natywną regresję.

## Najważniejsze zachowanie
Context window jest tylko bieżącym obszarem roboczym. Po rolloverze nowy call nie zaczyna od zera: może dostać `ResumePacket` z ostatniego zatwierdzonego stanu pracy. Packet nie przechowuje hidden chain-of-thought i nie gwarantuje identycznego zachowania nowego modelu. `nextSuggestedStep` pozostaje sugestią; przyszła decyzja nadal może CONTINUE/REVISE/DEFER/ABANDON.

## Regresje natywne dodane do SOURCE
- preflight zostawia rezerwę na checkpoint;
- route/model context change wykonuje świeży preflight;
- technical + semantic checkpoint budują Resume Packet bez raw tool result / hidden reasoning;
- stale semantic checkpoint odrzucany po nowszym technical checkpoint;
- CAS parent technical checkpoint;
- restart odtwarza Resume Packet z SQLite;
- unresolved external action jest jawnie obecna w Resume Packet;
- realny decision opportunity otrzymuje Resume Packet aktywnej pracy;
- source refs checkpointów są tombstonowane po permanent deletion źródła.

## Granice
- brak prawdziwego tokenizera providera: preflight przyjmuje jawnie estymowane komponenty; live provider integration należy do późniejszego podłączenia route/provider;
- brak automatycznego semantic compaction przez Scribe/Primary; EXP-2 zamraża trwały mechanizm i walidację, a rzeczywiste model calls pozostają poza tym podkrokiem;
- brak produkcyjnych konektorów i reconciliation service;
- SQLite pozostaje v13, bo tabele Work Continuity zostały dodane już w EXP-0;
- frozen P2 bez zmian.

## Lokalne kontrole
- P2 193/193 PASS
- boundaries 31/31 PASS
- command parity 54/54 PASS
- Rust source-text PASS
- Standard closure 8/8 PASS
- Standard release 8/8 PASS
- Experimental foundation 12/12 PASS
- Experimental runtime 24/24 PASS
- Experimental continuity 20/20 PASS
- JSON/schema syntax PASS
- Cargo niedostępne lokalnie;
- frontend `npm ci` nie zakończył się w lokalnym limicie czasu, więc Vitest/TypeScript/Vite wymagają CI. Zmiany produktu w tym kroku są w warstwie Rust/scripts/docs.

## Następny krok
Po pełnym zielonym CI i audycie EXP-2 można uznać 4B-9 za zamknięte i rozpocząć 4B-10 / opcjonalny InternalStateProvider (ConsciousCore adapter), zachowując NullState i całą Work Continuity jako niezależny fundament.
