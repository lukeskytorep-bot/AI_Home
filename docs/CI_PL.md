# Codzienne CI AI Home

Push/PR: frontend i Rust/SQLite/Tauri na Ubuntu 24.04, bez upload-artifact. Cache przyspiesza kompilację. Ręczne check_windows=true dodaje testy Windows; build_desktop=true testuje obie platformy i buduje kandydaty bez instalatora/publikacji. Obie opcje domyślnie false.

Cargo.lock istnieje. Nie generować go ponownie przy zmianie dokumentacji lub workflow. Prepare Cargo lockfile służy do rozstrzygania zmian zależności i wymaga osobnego przeglądu/commitu.

Edward potwierdził udany przebieg po poprawce async 12.10.2026 o 00:20. Nie dostarczono linku, commit ID ani logów tego sukcesu; zakres Windows nie został potwierdzony. Wynik użytkownika nie zastępuje niezależnej weryfikacji CI ani prób rzeczywistego okna.
