# AI Home 4B-6 krok 3 AUDIT FIX — raport

## Baza i zakres

Poprawka została wykonana wyłącznie na `AI_HOME_4B6_KROK_3_SOURCE.zip` o SHA-256 `7055dd774695979cc6b5aa186cacf268dd06f2fa039555703eafa62a63b8f964`. Nie rozpoczęto permanent deletion ani 4B-7. SQLite pozostaje w wersji 12 i nie dodano migracji.

## E1 — TypeScript BlobPart

`ExportPanel.download()` nie przekazuje już `Uint8Array<ArrayBufferLike>[]` bezpośrednio do `Blob`. Chunks są gromadzone jako `ArrayBuffer[]`, tworzone z jawnie typowanych `Uint8Array<ArrayBuffer>`. Nie użyto `any` i nie osłabiono typecheck.

## E2 — izolacja widoku list eksportów

`loadExports()` posiada generation guard i sprawdzenie aktualnego `profileRef`. Zmiana profilu zwiększa generację, czyści listę i błąd, a późny wynik lub błąd starego profilu nie modyfikuje aktualnego widoku. Render i download dodatkowo wymagają zgodności `ExportItem.profileId` z bieżącym profilem. Refresh i odczyt po create przechodzą przez tę samą ochronę.

## E3 — definitive rejection vs ambiguous transport

Pending create jest mapą per profile. `ok:false` z native jest jednoznacznie rozliczane i usuwa pending requestId dla tego profilu. Tylko exception transportu zachowuje dokładny requestId do retry. Inne profile nie są blokowane. Dodano testy definitive rejection oraz ambiguous retry z tym samym requestId.

## E4 — samowystarczalny zakres danych

Eksport zawiera teraz:

- `data/routes.json`,
- `data/connections.portable.json` z providerKind, endpointem, label, rewizjami/status i `requiresCredentialRebind:true`, bez `credentialRef`,
- `data/memory-proposals.json`,
- `data/memory-decisions.json`.

Dzięki temu profile route IDs i memory decision links są rozwiązywalne w paczce w deklarowanym zakresie. Nie dodano technicznych receipts, chat replay ani raw provider payloads.

## Odczyt chunków

`profile.export.read` używa `File::seek` i `read_exact` dla pojedynczego zakresu do 1 MiB zamiast `fs::read()` całego ZIP przy każdym wywołaniu. Metadata nadal określa `totalBytes`, nazwę i SHA całego pliku.

## Testy dodane

Frontend:
- opóźniona lista profilu A nie trafia do widoku B,
- definitive native rejection nie blokuje wyboru B,
- ambiguous retry dla A zachowuje ten sam requestId bez blokowania B.

Native:
- skonfigurowany profil eksportuje route i sanitizowaną connection bez credentialRef,
- `MemoryVersion -> Decision -> Proposal` rozwiązuje się w ZIP,
- manifestowe size/SHA odpowiadają rzeczywistym bajtom wpisów,
- oryginalny import source blob zachowuje bajty i SHA.

## Lokalne bramki

- P2: 193/193 PASS,
- boundaries: 29/29 PASS,
- command parity: 49/49 PASS,
- Rust source-text normalization: PASS,
- schema expectation sweep: PASS,
- SQLite: v12, bez migracji.

`npm ci` w sandboxie nie zakończyło się przed limitem czasu. Częściowa instalacja nie zawierała kompletu `vite/client` i `vitest/globals`, dlatego lokalnego `npm run typecheck/build` nie przedstawia się jako wynik poprawki. Pełny frontend CI, Cargo i desktop/WebView smoke pozostają wymagane.
