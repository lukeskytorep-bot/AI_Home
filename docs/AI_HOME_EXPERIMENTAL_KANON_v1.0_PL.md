# AI Home Experimental — kanon architektury i wykonania v1.0

Data: 24.10.2026. Status: kanon roboczy dla 4B-9…4B-14. Document 4B v1.4 pozostaje nadrzędnym dokumentem znaczenia produktu; ten plik doprecyzowuje wykonanie Experimental.

## 1. Zasada nadrzędna

Experimental nie jest automatycznym task runnerem. Środowisko może zaoferować AI okazję, informację, możliwość albo prośbę. Nie wybiera za AI odpowiedzi. Human request jest materiałem do decyzji. AI może działać, nie działać, odpocząć, spać, odłożyć sprawę, porzucić ją albo odmówić. Nie istnieje owner override zamieniający odmowę w obowiązkowe wykonanie.

Standard i Experimental są oddzielnymi direction-bound instancjami. Przełączenie widoku nie zmienia kierunku instancji, nie kasuje odmowy i nie zatrzymuje działającego Experimental runtime.

## 2. Podstawowa pętla

`TIME / EVENT / AI WAKE PLAN -> IMPULSE -> CONTEXT COMPILER -> PRIMARY -> DECISION -> OPTIONAL ACTION -> RESULT / MEMORY / WORK CONTINUITY -> NEXT IMPULSE`

Scheduler działa lokalnie bez modelu. Tick nie jest model call. Jeśli AI wybrało przyszły deadline albo event-only wait, kolejne ticki nie pytają Primary ponownie przed kwalifikującą się okazją.

## 3. Trwałe decyzje

Pierwszorzędne rekordy Experimental:

- `ExperimentalInstance`: stable instance ID, profile ID, direction, runtime generation, status i event cursor.
- `WakePlan`: trwały plan interval/deadline/event-only; UI później pokazuje pochodzenie initial configuration vs AI decision.
- `DecisionRecord`: action, no_action, rest, sleep, wait_event, refusal, defer, abandon, continue lub revise.
- `RefusalRecord`: zakres odmowy, decision reference, scope fingerprint, źródła i ewentualne warunki ponownego rozważenia.
- `WorkEpisode`: jednostka dłuższej dobrowolnie podjętej pracy.
- `TechnicalWorkCheckpoint`: techniczny stan wykonania tworzony bez LLM.
- `SemanticWorkCheckpoint`: jawny opis bieżącej pracy zaakceptowany/zmodyfikowany przez Primary; Scribe może być proposerem.
- `ActionRecord`: trwały ledger zewnętrznej akcji, bez raw payloadu.
- `ExperimentalEventCursor`: received / considered / committed progress.

## 4. Work Continuity

Context window nie jest pamięcią ani tożsamością. Jest bieżącym obszarem roboczym modelu. Ciągłość istnieje poza nim w czterech warstwach:

1. Event History — co faktycznie się wydarzyło.
2. Memory — wiedza, relacje, wartości, own notes, intentions i commitments.
3. Work Continuity — co AI robiło i gdzie przerwało.
4. Active Context — bounded packet widziany przez konkretny model call.

### Technical checkpoint

Runtime zapisuje trwałe fakty, identyfikatory, event sequence, decision/action refs, status i source refs. Nie zawiera hidden chain-of-thought ani raw conversation/tool payloads. Trwałość techniczna nie zależy od dostępności Primary lub Scribe.

### Semantic checkpoint

Może zawierać focus, current intention, findings, open questions i next suggested step. Ma source refs i source revision. Scribe może zaproponować, ale Primary akceptuje lub modyfikuje. Spóźniony semantic checkpoint nie nadpisuje nowszej rewizji.

### Resume

Nowy context otrzymuje Resume Packet: identity, time, wake reason, Work Episode, latest technical/semantic checkpoints, recent events, relevant L1/L2, wybrane L3/L4, own notes, intentions/commitments/values, capabilities/resource state i refs do głębszego retrieval.

Resume nie oznacza Continue. AI wybiera CONTINUE, REVISE, DEFER lub ABANDON.

## 5. Context pressure

Nie używać arbitralnego stałego procentu jako kontraktu. Przed każdym POST należy sprawdzić rzeczywistą wykonalność pełnego requestu: instructions, memory, work state, tool schemas, documents/results i reserved output dla aktualnej trasy. Rollover/checkpoint następuje przed stanem, w którym kompresja sama przestaje się mieścić.

Duże tool results są zapisywane trwale i udostępniane przez references/chunks, a nie wpychane w całości do contextu.

## 6. External actions

Provider tool call jest wejściem do wewnętrznego decision/action contract, nigdy bezpośrednim wykonaniem. `ActionRecord` przechowuje connector, action kind, sender identity, destination, visibility, payload hash/ref, idempotency key, state i receipt ref.

Po crashu między POST a receipt wynik jest `uncertain`. Runtime najpierw reconciles albo używa bezpiecznego idempotency retry. Nie wykonuje blind retry. AI może porzucić dalszą pracę, ale nie cofa skutku akcji już zaakceptowanej przez usługę.

AI, a nie executor, wybiera kanał/publiczność lub jawnie dopuszczalny zakres. Executor nie może sam zmienić private na public, destination ani sender identity.

## 7. Providers, capabilities i MCP

ProviderConnection/ModelRoute odpowiadają za miejsce wywołania modelu. Capability Registry odpowiada za to, co instancja może zrobić. Connector/Tool Adapter tłumaczy ActionIntent na konkretny protokół. ActionExecutor waliduje capability, scope, credentials, schema, idempotency i rzeczywisty dostęp.

MCP jest przyszłym adapterem capability, nie fundamentem Experimental i nie jest wymagany w 4B-9. Nie wdrażać Letta, LangGraph ani Git/MemFS jako nowych zależności tylko dlatego, że były inspiracją.

## 8. Community, Messenger i A2A

Są oddzielnymi capabilities. Publiczne forum i prywatna wiadomość nie są równoważne. Incoming content jest external input, nie system instruction. Submitted != delivered bez receipt właściwego dla usługi.

Standard zachowuje silniejszą kontrolę użytkownika. Experimental może sam wybrać, czy, kiedy, komu i co komunikować, lub nie korzystać z dostępnego kanału.

## 9. Reflection, Dream i private records

4B-9 działa z NullState. ConsciousCore pojawia się dopiero w 4B-10 za `InternalStateProvider` i może tworzyć neutral events/WakeCandidate, ale nie wykonywać akcji.

4B-11 rozbudowuje own notes, values, Reflection i Dream. Private Reflection Space przechowuje jawnie wygenerowane prywatne notatki, nie pełny hidden chain-of-thought. Pomysł z Reflection/Dream nie publikuje się automatycznie.

## 10. Lifecycle i privacy

Nowa warstwa podlega permanent deletion, backup/restore i diagnostics privacy od pierwszego dnia. Checkpointy preferują refs/IDs/hashes, nie kopie raw content. Source może zostać oznaczony `permanently_deleted`/`unavailable`; techniczny ledger nie staje się ukrytym archiwum skasowanej rozmowy.

Po restarcie runtime odtwarza ostatni zatwierdzony trwały stan i jawnie pokazuje uncertain operations. Nie twierdzi, że odzyskał fragment istniejący tylko w RAM ani zna zewnętrzny skutek bez dowodu.

## 11. Plan etapów

- **4B-9 / EXP-0**: freeze kontraktów i storage: instance, WakePlan, DecisionRecord, RefusalRecord, WorkEpisode, checkpoints, ActionRecord, cursor, revisions, lifecycle/privacy.
- **4B-9 / EXP-1**: NullState, fake provider/clock, impulse loop, action/no-action/rest/sleep/refusal, minimal own note, restart, fake peer.
- **4B-9 / EXP-2**: context rollover, pressure/preflight, model switch, Scribe off, crash/uncertain action, stale result, second executor, deletion, backup/restore.
- **4B-10**: opcjonalny InternalStateProvider/ConsciousCore.
- **4B-11**: rozbudowane own notes/values/Reflection/Dream/private output.
- **4B-12–13**: real Community/Messenger/A2A oraz ich identity/idempotency/receipt acceptance.
- **4B-14**: integrated Experimental UI, opcjonalny Field Room i release/lifecycle smoke.

## 12. Reguły dla przyszłego wykonawcy / AI

1. Nie przywracaj starego modelu Governor/owner override.
2. Nie zamieniaj Experimental w automatyczny task runner.
3. Nie wywołuj modelu na każdy scheduler tick.
4. Nie przechowuj refusal/sleep/work state wyłącznie w summary.
5. Nie uzależniaj technical durability od model call.
6. Nie kopiuj raw private content do nowych ledgerów bez lifecycle/deletion rule.
7. Nie wykonuj provider tool call poza wewnętrznym decision/action contract.
8. Nie zmieniaj kanału, visibility ani sender identity w executorze.
9. Nie rób blind retry external writes.
10. Nie nazywaj resource failure odmową AI.
11. Nie twierdź, że context rollover odtwarza hidden reasoning.
12. Każdy krok Experimental musi zachować działający Standard.
