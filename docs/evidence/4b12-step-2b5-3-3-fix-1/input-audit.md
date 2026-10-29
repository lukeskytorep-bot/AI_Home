# Audyt główny — AI Home 4B12 / 2B5.3.3

Data: 28.10.2026

## Werdykt

**DO POPRAWY — F1/P2: nieprawidłowe mapowanie utraty oryginalnych kluczy.**

Problem potwierdzono własnym uruchomieniem dodatkowego testu Rust. Nie powoduje uznania nieprawidłowego credential za użyteczny, ale łamie zadeklarowaną semantykę nowej komendy: spodziewany brak dostępnego materiału to localCredentialUsable=false, a nie awaria przechowywania registry.

Poprawki implementacji nie wykonywałem. Całe 2B5.3, 2B i 4B12 pozostają otwarte.

## F1/P2 — błędy kluczy są opakowane inaczej niż błędy tokenów

Miejsce: src-tauri/src/community_host/local.rs, gałąź LocalCapability::AccessStatus, match wyniku RegistrationFinish::current.

Adapter zamienia FinishError::Identity(_) i RecoveryRequired na false. Brak lub niezgodność oryginalnych kluczy idzie jednak następującą ścieżką:

1. current -> credential -> archive.
2. archive wywołuje registry.load_intent.
3. load_intent wywołuje keys.load; IdentityError staje się RegistryError::Identity.
4. Konwersja do FinishError zachowuje opakowanie: FinishError::Registry(RegistryError::Identity(...)).
5. Nowy match nie obejmuje tego wariantu; wildcard zwraca STORAGE_ERROR.

Niezgodność załadowanych publicznych kluczy z oryginalnym oczekiwaniem także jest jawnie zwracana przez load_intent jako RegistryError::Identity(ScopeMismatch), więc ma tę samą ścieżkę.

To jest różnica względem odczytu tokena, gdzie IdentityError jest konwertowany bezpośrednio na FinishError::Identity. Dostarczony nowy test obejmuje brak tokena i jego nieprawidłowy scope, lecz nie brak kluczy przy gotowych metadanych finish.

### Własny test odtwarzający

Uruchomiłem cztery dostarczone testy filtrem access_status: 4 PASS. Następnie tymczasowo dodałem piąty test, kopiując istniejący access_status_validates_actual_token_then_detects_loss_and_scope_tampering i nadając mu nazwę audit_access_status_missing_original_keys_reports_false.

Po pierwszym potwierdzonym wyniku localCredentialUsable=true dodałem:

```rust
f.secret.data.lock().unwrap().remove(prep.key_reference());
let missing = call(&h, req.clone(), &s).await;
assert_eq!(
    missing,
    json!({
        "contractVersion": 1,
        "requestId": req["requestId"],
        "ok": true,
        "result": {
            "localCredentialUsable": false,
            "onlineStatusKnown": false
        }
    })
);
```

Polecenie:
cargo test --manifest-path src-tauri/Cargo.toml --lib --no-default-features --locked --offline audit_access_status_missing_original_keys_reports_false

Wynik: **0 PASS / 1 FAIL**. Rzeczywiste ok=false, error.code=STORAGE_ERROR; oczekiwane ok=true i localCredentialUsable=false. Klucze usunięto z deterministycznego testowego vault; finish i token pozostały. Test używa rzeczywistych baz SQLite i dispatcher adaptera.

Po próbie przywrócono oryginalny plik testów z paczki. Ponowny verify-source: PASS. Dodatkowy test nie jest częścią dostarczonego SOURCE ani poprawką implementacji.

### Wymagana korekta

- Rozpoznać zagnieżdżone FinishError::Registry(RegistryError::Identity(...)) w semantyce nowej komendy, zgodnie z zadeklarowaną obsługą niedostępnych lub niezgodnych kluczy.
- Nie mapować wszystkich RegistryError na false: Corrupt/Storage i inne rzeczywiste uszkodzenia metadanych muszą nadal pozostać błędami.
- Dodać testy dla braku oryginalnych kluczy, niedostępnego backendu kluczy oraz niezgodnego materiału kluczy przy gotowym finish.
- Zachować brak generacji/zapisu/usuwania sekretów i brak receipts. Wynik false nadal musi mieć onlineStatusKnown=false.
- Zachować test corrupt_finish -> STORAGE_ERROR i istniejące pozytywne przypadki kluczy/tokenów.
- Przekazać poprawkę, logi regresji i nowe CI do ponownego audytu.

Nie zmieniać bazowego mechanizmu tożsamości lub regenerować kluczy jako sposobu naprawy tej komendy.

## Integralność — własna weryfikacja

- PACKAGE_SHA256SUMS oraz CRC BASE/SOURCE/OVERLAY: PASS.
- SHA-256 bazy: 214eca73cc3d2e37f782918bcc7e9af1c86c9859f9aab76f964bfbac334a02f6 — poprzedni SOURCE 2B5.3.2.
- SOURCE: 42b388f1c3a92cd7c69b693a4263ecec8202172ec177e4a2cb8c02558378a611.
- BASE + overlay = SOURCE bajtowo: PASS; baza 974 pliki, SOURCE 983, nakładka 16; bez usunięć.
- Manifest: 981 hashy / 982 wpisy SHA256SUMS, PASS.
- Wszystkie 3 hashe wskazanych testowanych plików zgodne.

## Własne wykonane kontrole

| Kontrola | Wynik |
|---|---|
| Dostarczone testy Rust access_status | 4/4 PASS |
| Dodatkowy test audytowy utraty kluczy | FAIL — potwierdza F1 |
| Frontend Vitest | 219/219 PASS, 21 plików |
| TypeScript/Vite build | PASS |
| P2 | 193/193 PASS |
| Granice / migration assertions | 34/34 PASS / 14 przy v16 |
| Command parity | 59/59 PASS |
| Community identity / reader static guards | 14/14 i 14/14 PASS |

W tym turnie dostępny jest już toolchain Rust/cargo 1.99.1. Własne polecenie dla dostarczonych testów używało --no-default-features --locked --offline i filtra access_status. Skompilowało również targety integracyjne, ale filtr uruchomił tylko cztery testy. Nie jest to własny wynik całego zestawu 634 testów.

Log autora zawiera 23 zestawy, 634 PASS i 0 FAIL. Pełnej regresji 634 nie powtarzano; nie było potrzeby poszerzania testów po uzyskaniu bezpośredniego dowodu F1. Nie odebrano default-feature Tauri, Windows/WebView, realnego OS vault ani live forum.

## Pozostały zakres i plan 1.86

Rzeczywisty zakres jest węższy niż pierwotnie oczekiwany adapter HTTP: dodano wyłącznie community.registration.access.status. Raport i plan 1.86 opisują to jawnie; start/complete/retry/recovery HTTP i desktop enablement są nadal przyszłą pracą.

Komenda ma osobne AccessStatus capability, pusty ścisły payload i korzysta z dotychczasowej kontroli profilu, rewizji, kierunku, Experimental binding, deletion i host binding. Nie zapisuje trwałego receipt; ponowienie odczytuje bieżący stan. Nie wykonuje żądania HTTP. RegistrationFinish::current sprawdza oryginalne klucze i scoped token; projekcja ujawnia tylko dwa booleany i nie przekazuje tokena.

onlineStatusKnown=false prawidłowo zapobiega utożsamieniu lokalnego credential z aktualną akceptacją serwera. Późniejsza operacja sieciowa musi ponownie przeprowadzić autoryzację i odczyt credential; wynik tej komendy nie jest grantem do późniejszego HTTP.

W sprawdzonym zakresie nie znaleziono dodatkowego blokera. Najpierw naprawić F1 i przeprowadzić ponowny audyt; sam zielony wynik obecnego CI nie zamyka tego ustalenia.
