# `artifacts/ui/before/` — baseline screenshots

**Status: NOT CAPTURED — `BLOCKED: build toolchain missing`.**

This directory intentionally contains no screenshots. The machine/worktree used for T0 has no MSBuild, MSVC `cl`, `vswhere`, Visual Studio installation, or NuGet CLI (see `docs/ui/ENVIRONMENT.md` §1), and there is no built `Release64\Ditto.exe` or `Debug64\Ditto.exe`. Fabricating screenshots or reusing images from another build would not represent commit `a80fd3536bc407cac8cce43b5682022282ace983`.

## How to capture the baseline once a toolchain is available

```bat
:: From the T0 worktree root
nuget restore CP_Main_10.sln
msbuild CP_Main_10.sln /p:Configuration=Release /p:Platform=x64

:: Portable test layout — never use %APPDATA%\Ditto
set RUN=%TEMP%\ditto-ui-before
if exist "%RUN%" rmdir /s /q "%RUN%"
mkdir "%RUN%"
copy Release64\Ditto.exe        "%RUN%"
copy Release64\ICU_Loader.dll   "%RUN%"
xcopy /E /I Release64\Addins    "%RUN%\Addins"
xcopy /E /I Debug\Language      "%RUN%\Language"
xcopy /E /I Debug\Themes        "%RUN%\Themes"
type nul > "%RUN%\portable"
```

Then:

1. Write `%RUN%\Ditto.Settings` with `Theme2=Modern Dark` (or `DarkerDitto` for the current dark theme), `DBPath=%RUN%\Ditto.db`, `LanguageFile=English`.
2. Start `%RUN%\Ditto.exe`, copy several text/URL/image/colour/file clips, run `Ditto.exe -open`.
3. Capture at least:
   - light popup: normal, search focused, group tree open, empty results;
   - dark popup: same states;
   - options: General, Keyboard, QuickPaste Keyboard, AdvGeneral;
   - at least two ordinary dialogs (e.g., Group Name, About, Copy Properties).
4. Save PNGs here as `before-<surface>-<theme>-<dpi>.png`, with a manifest listing DPI/theme/scale.
5. Repeat at 100%, 150%, and 200% display scaling if the desktop supports it; record the actual scale.
6. Restore any temporary registry changes in a `finally` block. Never leave `DBPath` pointing at the real database.

If there is no interactive desktop, report `BLOCKED: no interactive desktop` instead.

## Why this matters

The prompt requires before/after visual evidence. Without a baseline, T5 cannot prove what changed. A later task may populate this directory; it must use real screenshots from this source tree and a portable profile, not mockups.
