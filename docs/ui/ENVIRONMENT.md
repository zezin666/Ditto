# ENVIRONMENT — Ditto UI modernization (T0)

Status: **BLOCKED: build toolchain missing**

- Date: 2026-11-10
- Worktree: `C:\Users\zezin\orca\workspaces\Ditto\ui-design`
- Branch: `taohy/ui-design`
- Base branch: `master`
- Base / HEAD commit: `a80fd3536bc407cac8cce43b5682022282ace983`
- OS: Windows 11 Home China, build `10.0.26200`
- Shell used for checks: Git Bash (MSYS2)
- Audit/design agent: `claude` (T0)

This file is evidence, not an apology. It records exactly what could and could not be verified. No build log, binary, or screenshot in this worktree is real unless a path is explicitly listed under a successful build in a later task.

---

## 1. Pre-build toolchain check (executed)

Commands and actual results:

```text
$ where msbuild
INFO: Could not find files for the given pattern(s).

$ where cl
INFO: Could not find files for the given pattern(s).

$ where vswhere
INFO: Could not find files for the given pattern(s).

$ ls "/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
ls: cannot access ...: No such file or directory

$ ls "/c/Program Files/Microsoft Visual Studio"
ls: cannot access ...: No such file or directory

$ ls "/c/Program Files (x86)/Microsoft Visual Studio"
ls: cannot access ...: No such file or directory

$ where nuget
INFO: Could not find files for the given pattern(s).

$ where dotnet
C:\Program Files\dotnet\dotnet.exe       # present, but cannot build MFC/C++/v145

$ ls Release64 Debug64
ls: cannot access 'Release64': No such file or directory
ls: cannot access 'Debug64': No such file or directory
```

The coordinator's independent pre-check reached the same result: `msbuild`, `cl`, `vswhere`, and Visual Studio in the usual install/registry locations are absent. Orca has only the `local` host; no remote Windows build server/environment is connected.

**Conclusion:** this machine cannot run `nuget restore` + `msbuild CP_Main_10.sln`. The required workload is Visual Studio with **MFC for desktop C++**, MSVC **v145** (x64/x86; ARM64 uses v143), the Windows 10/11 SDK, and NuGet CLI.

---

## 2. Consequences for T0

| Deliverable | Status | Why |
|---|---|---|
| `docs/ui/CURRENT-STATE.md` | Done | Source audit is possible without a compiler. Every claim cites `file:line`. |
| `docs/ui/DESIGN-SYSTEM.md` | Done | Frozen tokens/specs are source-independent. |
| `docs/ui/IMPLEMENTATION-PLAN.md` | Done | File ownership and acceptance are defined from the source audit. |
| `artifacts/ui/before/` screenshots | **Not produced** | No executable exists; building would require the missing toolchain. |
| `artifacts/ui/mockups/` | Done | HTML/SVG reference mockups, clearly labeled as mockups (not app screenshots). |
| Build verification | **Not performed** | BLOCKED. |
| 100/150/200% DPI visual verification | **Not performed** | BLOCKED. Source-level DPI review only. |
| Light/dark/high-contrast visual verification | **Not performed** | BLOCKED. Token contrast values are calculated, not screenshotted. |

No prebuilt Ditto binary was downloaded or used. A binary from another build would not represent this commit, and using a normal user profile would touch real settings/database — both are out of scope.

`artifacts/ui/before/README.md` records the blocker and the exact recipe T5 must use once a toolchain is available. It does not pretend to contain screenshots.

---

## 3. How to unblock

Any one of the following, in order of preference:

1. **Install Visual Studio Build Tools / Visual Studio** with the *Desktop development with C++* workload and *MFC for v145* (plus Windows 10/11 SDK and NuGet CLI). Then re-run:
   ```bat
   nuget restore CP_Main_10.sln
   msbuild CP_Main_10.sln /p:Configuration=Release /p:Platform=x64
   ```
   Expected outputs: `Release64\Ditto.exe`, `Release64\ICU_Loader.dll`, `Release64\Addins\DittoUtil.dll`, `Release64\focus.dll`.
2. **Connect a Windows build host** to Orca and dispatch build/QA tasks there.
3. **Explicitly accept code-only changes** with the user building and visually verifying locally. In that case T1–T6 may produce source diffs, but must mark every build/screenshot claim UNVERIFIED and must not claim acceptance evidence.

Until one of these happens, implementation tasks T1–T4b should not be started: their specs require build logs and screenshots as acceptance evidence.

---

## 4. Reproduction recipe for a future environment

```bat
:: Toolchain
where msbuild
where cl
where vswhere
nuget restore CP_Main_10.sln
msbuild CP_Main_10.sln /p:Configuration=Release /p:Platform=x64

:: Portable run layout (never touch %APPDATA%\Ditto)
set RUN=%TEMP%\ditto-ui-qa
mkdir "%RUN%"
copy Release64\Ditto.exe        "%RUN%"
copy Release64\ICU_Loader.dll   "%RUN%"
xcopy /E /I Release64\Addins    "%RUN%\Addins"
xcopy /E /I Debug\Language      "%RUN%\Language"
xcopy /E /I Debug\Themes        "%RUN%\Themes"
type nul > "%RUN%\portable"
```

Then follow prompt §7 for the portable settings file, `Ditto.exe -open`, and screenshots. Screenshot files must go to `artifacts/ui/before/` (T0 baseline) or `artifacts/ui/after/` (T5).

---

## 5. Evidence rules for downstream tasks

- A build claim requires the exact command, exit code 0, and a log path.
- A visual claim requires a real screenshot path and the DPI/theme it was taken at. Mockups are never accepted as "after" screenshots.
- A DPI claim at 150%/200% requires actually changing display scaling (or a remote session at that scale). Otherwise mark **UNVERIFIED**.
- High-contrast claims require enabling a high-contrast theme; otherwise code review + **UNVERIFIED**.
- If a worker's shell has no interactive desktop, report `BLOCKED: no interactive desktop`; do not fabricate screenshots.

---

## 6. Known environment risk for implementation

Even after the toolchain is installed, this repo has **no automated tests** (`CLAUDE.md`). Verification is build + portable run + manual regression. The UI is DPI-aware (`DeclareDPIAware.manifest`: `permonitorv2,permonitor`), so visual QA must include a real per-monitor DPI change, not just a window resize.
