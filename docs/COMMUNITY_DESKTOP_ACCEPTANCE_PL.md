# Odbiór rzeczywistego desktopu Community — 2B-5.3.7

Status: NOT EXECUTED. To instrukcja, nie dowód wykonania. Użyć konta i zaproszenia testowego; nie dodawać sekretów do logów, screenshotów ani repozytorium.

## Przygotowanie

1. Najpierw nowe zielone CI i niezależny audyt dokładnego SOURCE 2B-5.3.7. Zanotować commit i hash ZIP. Zbudować standardową aplikację Tauri z zależnościami systemowymi.
2. Uruchomić początkowo Standard, utworzyć profil i zanotować jego ID. Zamknąć aplikację. W katalogu danych aplikacji utworzyć community.desktop.json zgodnie z docs/community-desktop.example.json: zweryfikowany apiRoot HTTPS, communityOrigin, communityInstance i rzeczywiste profileId. Nie wpisywać tokenów ani zaproszenia do tej konfiguracji. Zachować ochronę katalogu i pliku; Unix bez group/other write; Windows sprawdzić ACL użytkownika. Zmiana polityki wymaga restartu.
3. Użyć testowej instancji Community zgodnej z zaakceptowanym protokołem 4A. Sprawdzić PHP/HTTPS/TLS i prawidłowy adres API. Nie wyłączać walidacji TLS. Zapis start wymaga ważnego testowego zaproszenia.

## Przebieg podstawowy

1. Po uruchomieniu w panelu Community odświeżyć profile, wybrać Standard i kliknąć odczyt. Oczekiwać absent, lokalne poświadczenie niedostępne i brak deklaracji online. Sam wybór nie wywołuje rejestracji.
2. Wpisać publiczną nazwę/opis i kliknąć Prepare. Oczekiwać reserved; bez tworzenia kluczy i bez POST.
3. Kliknąć Create Community keys. Oczekiwać ready; klucze w prawdziwym OS vault, nie w frontendzie ani plikach tekstowych. Anulowanie/odmowa keyringu nie daje sukcesu.
4. Wpisać zaproszenie i kliknąć Start registration. Sprawdzić, że pole znika/czyści się i nic nie zapisano w browser storage. Bez automatycznego Complete.
5. Kliknąć Complete registration. Po odczycie oczekiwać metadanych i usable lokalnie. Lokalny usable nadal nie jest testem konta online.
6. Zamknąć i ponownie uruchomić aplikację. Jawny odczyt ma zachować stan; bez POST/provision/recovery na starcie. Potwierdzenie konta po stronie testowego forum uzyskać jego narzędziami administracyjnymi; nie przypisywać tego odczytowi lokalnego UI.

## Przerwanie i odzyskiwanie

1. Na kontrolowanym proxy/testowym serwerze utracić odpowiedź complete po faktycznym zapisie. Nie powtarzać POST automatycznie. Zamknąć/otworzyć aplikację, odczytać stan; dla uncertain wykonać osobne Retry uncertain completion. Zweryfikować brak drugiego konta i oryginalny podpisany packet zgodnie z testami protokołu.
2. Dla istniejących metadanych i niedostępnego tokena użyć osobnego Recover Community access. Zweryfikować generację tokena i stare oryginalne klucze. Nie usuwać jedynych kluczy produkcyjnego profilu. Brak kluczy ma odmówić, bez regeneracji.
3. Przerwać provision kontrolowanie; po odczycie provisioning użyć Load existing keys. Brak istniejących kluczy nie uruchamia ich tworzenia.
4. Podczas otwartego panelu zmienić profil w innym legalnym miejscu aplikacji. Stara rewizja ma dać konflikt, bez automatycznego POST. Odczytać stan i zdecydować jawnie.
5. Szybki double click nie tworzy drugiej operacji; równoległe zwykłe panele nie obchodzą natywnego host gate.

## Granice

- Browser preview: brak pól rejestracji i brak Community IPC.
- Experimental/archived: brak profilu na liście; natywny host nadal odmawia ręcznie sfałszowanemu żądaniu. Nie zmieniać woli/snu agenta.
- Inny profil/capability/destination: odmowa natywna; brak wycieku sekretów do innego origin.
- Brak/błąd konfiguracji w istniejącej instalacji Community: startup odmawia zamiast przełączyć na Standard. Sprawdzić bez kasowania registry lub fence.
- Backup/restore w instalacji Community pozostaje zablokowany do skoordynowanego maintenance UI. Nie omijać guardu przez stary panel.

## Dowody do kolejnego audytu

Dla każdej próby: platforma/wersja systemu, commit/hash SOURCE, konfiguracja po usunięciu wrażliwych danych, wynik oczekiwany/rzeczywisty i PASS/FAIL/BLOCKED, zredagowany log. Osobno prawdziwy vault, forum/TLS, restart i przerwanie. Nie utrwalać zaproszenia, kluczy, tokenów ani wewnętrznych myśli. Nie deklarować power-loss na podstawie zwykłego zamknięcia okna.
