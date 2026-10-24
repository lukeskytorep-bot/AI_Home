# Instrukcja — AI Home 4B-8 AUDIT FIX 2

Baza: `AI_HOME_4B8_AUDIT_FIX_SOURCE.zip`.

1. Nałóż `AI_HOME_4B8_AUDIT_FIX_2_CHANGED_FILES.zip` na repozytorium.
2. Upewnij się, że istnieje `src/ui/DiagnosticsPanel.tsx`.
3. Uruchom pełne GitHub Actions: Vitest, typecheck/Vite build i Cargo.
4. Nie przechodź do 4B-9, dopóki 4B-8 nie jest zielone i nie przejdzie desktop smoke wymagany przez audyt.

Repair overlay celowo zawiera ponownie komplet nowych plików runtime/testowych 4B-8, nawet jeśli część ma identyczne bajty jak poprzedni pełny SOURCE.
