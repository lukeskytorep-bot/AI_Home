# Bieżące przekazanie: 4B-9 / EXP-2 Work Continuity / Context Rollover

Na bazie zielonego `AI_HOME_4B9_EXP1_AUDIT_FIX_3_SOURCE.zip` wdrożono trwałą ciągłość pracy Experimental bez dodawania ConsciousCore ani produkcyjnych konektorów. Work Episode, technical/semantic checkpoints i Resume Packet pozwalają odtworzyć ostatni zatwierdzony stan pracy po context rollover lub restarcie.

Context preflight jawnie używa estymowanych tokenów i rezerwuje miejsce na output, input reserve oraz checkpoint. Duże tool results pozostają źródłami referencyjnymi zamiast surowej treści w context/ledgerze. Realny OpportunityPacket dostaje Resume Packet aktywnej pracy. Permanent deletion oznacza checkpoint refs jako `permanently_deleted`.

SQLite pozostaje v13, P2 193/193 bez zmian. Brak live Experimental IPC, background schedulera, ConsciousCore, MCP, Community, Messenger i A2A. Raport: `docs/AI_HOME_4B9_EXP2_RAPORT_PL.md`; plan treść 1.67. EXP-2 wymaga pełnego CI i audytu przed 4B-10.
