# PERFORMANCE — budgets, measurement protocol, and pass/fail criteria

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Scope: idle memory, popup first-frame latency, 10k-clip search/scroll,
  startup, settings WebView2 lifecycle, migration throughput.
- Primary owners: T1 (spike/startup), T2 (storage), T3 (popup), T7 (end-to-end
  evidence). T8 reviews the numbers.
- Rule: **no success claim without raw evidence.** Debug builds, debuggers, and
  "it feels fast" do not count.

---

## 1. Budget summary

| ID | Metric | Budget | Gate | Owner |
|---|---|---|---|---|
| PF-1 | Idle memory, settings closed, popup hidden | **< 100 MB** median of 3 cold samples | Must | T1/T7 |
| PF-2 | Hotkey → popup first presented frame | **p95 < 50 ms**, no sample > 150 ms | Must | T1/T3/T7 |
| PF-3 | 10k-clip search | **p95 < 50 ms** | Must | T2/T3/T7 |
| PF-4 | 10k-clip scroll | **60 fps** (p99 frame ≤ 16.7 ms), no frame > 50 ms | Must | T3/T7 |
| PF-5 | Scroll memory growth | **< 5 MB** over 60 s of continuous scroll | Must | T3/T7 |
| PF-6 | Core ready (tray + hotkeys + clipboard + IPC) | **< 1 s p95**, < 2 s max | Should | T1/T7 |
| PF-7 | Popup pre-warm complete after login | **< 2 s p95**, not on the hotkey path | Should | T1/T3 |
| PF-8 | Settings window open | **< 1.5 s p95** | Should | T5/T7 |
| PF-9 | Settings close releases WebView2 | No `msedgewebview2.exe` after **5 s**; memory within 10 MB of pre-open | Must | T5/T7 |
| PF-10 | Migration 10k clips + 20k formats | **< 60 s** on the QA SSD, with progress | Should | T6/T7 |
| PF-11 | DB size for 10k text clips | Record; no hard limit; warn if > 250 MB | Informational | T2/T7 |

`Must` failures block v1. `Should` failures require a written exception in
`QA-REPORT.md` and coordinator approval.

---

## 2. Definitions

- **Idle**: settings window never opened (or closed and fully released), popup
  hidden, no clipboard activity, no migration/backup running, 10 minutes after
  the process starts.
- **Cold sample**: a fresh sign-in or reboot, with the app started by the
  autostart entry, then the idle period. A process restart on a warm cache is
  not cold.
- **Private working set**: the Task Manager "Memory" column; the primary budget
  metric. Sum the core + popup + helper (helper only if alive for a test; idle
  should have none).
- **Working set**: total resident pages; report as a secondary number.
- **Commit size**: private commit; report as a secondary number.
- **First presented frame**: the first frame whose pixels are visible to the
  user after the hotkey. Instrument the core at hotkey receipt, the popup at
  message receipt, and the popup renderer at the first swap/present after show.
  If the renderer cannot expose present, use PresentMon or a DWM frame counter,
  and document the approximation.
- **p95**: the 95th percentile over the sample set, nearest-rank method.
- **QA machine**: record CPU, RAM, GPU, OS build, display resolution/DPI, power
  plan, and whether it is a VM. Shared CI runners are informational only.

---

## 3. PF-1 — Idle memory

### 3.1 Protocol (T1 spike and T7)

1. Reboot or sign out/in; log in and let the desktop settle for 5 minutes.
2. Confirm the build is **release**, no debugger attached, no WPR/ETW capture
   running.
3. Start Ditto via the autostart entry (`--background`), or let it start at
   login.
4. Do not open Settings. Do not open the popup. Do not copy anything. Do not
   run a backup, migration, or indexing job.
5. Wait 10 minutes.
6. Sample the process tree three times, 30 seconds apart:
   - `Ditto.exe` private working set, working set, commit;
   - `DittoPopup.exe` private working set, working set, commit;
   - any `DittoUacHelper.exe` (should be absent at idle);
   - any `msedgewebview2.exe` (must be absent).
7. Record the sample values as CSV/JSON. The **median of the three private
   working-set sums** is PF-1.

### 3.2 Pass/fail

| Result | Action |
|---|---|
| Median < 100 MB and no WebView2 | Pass Plan A |
| Median ≥ 100 MB | T1 spike switches to Plan B (headless core + on-demand settings) and re-measures |
| Plan B also ≥ 100 MB | Escalate to the coordinator/user: relax the budget or change the architecture; do not silently ship |
| Any `msedgewebview2.exe` at idle | Fail; find what created it (Tauri startup must not create a window) |

### 3.3 T1 spike

T1 must record in `docs/rewrite/PHASE0-NOTES.md`:

- build profile and commit;
- process list with PIDs and parent PIDs;
- private working set / working set / commit for core and popup;
- whether any WebView2 process exists;
- the A/B decision and the raw samples.

The spike is a **decision gate**, not a one-off benchmark. T7 repeats the same
protocol on the integrated branch.

### 3.4 Common causes of false failure

- Debug build, debugger, or `cargo run` with a console;
- Settings opened once and hidden (WebView2 stays alive);
- A popup that preloads all thumbnails/fonts at startup;
- ETW/WPR/PresentMon still running;
- Antivirus scanning the data directory;
- Windows Search indexing the DB/blobs;
- Laptop battery saver or a VM with dynamic memory.

Record and eliminate these before declaring a failure.

---

## 4. PF-2 — Popup first-frame latency

### 4.1 Protocol

1. Release build; popup pre-warmed and hidden; core and popup running.
2. Use a scripted harness (T7 `tools/qa`) that:
   - records `t0` immediately before injecting the global hotkey (or calls the
     same code path as the hotkey handler);
   - records `t1` when the core sends `ShowPopup`;
   - records `t2` when the popup receives it;
   - records `t3` at the first presented frame after show;
   - records `t4` when the popup is hidden for the next sample.
3. Run **100 samples** with a 1–3 s hide/show interval, from a realistic target
   app (Notepad, browser, Explorer), on the primary monitor at 100% DPI.
4. Repeat a 10-sample smoke set at 150% and 200% DPI and on a secondary
   monitor.
5. Report p50/p95/p99, min/max, and the raw CSV. The gate is **p95 < 50 ms**
   and **no sample > 150 ms**.

### 4.2 Latency budget

| Segment | Target p95 | Notes |
|---|---:|---|
| Hotkey receipt → core handler | 3 ms | Message loop must not block on DB/clipboard |
| Core → popup IPC send | 2 ms | Pre-serialized summary page; no 10k payload |
| Popup receive → show window | 10 ms | Pre-warmed window; no process/font/texture init |
| Show → first presented frame | 25 ms | egui layout/draw; cached textures |
| Margin | 10 ms | Scheduling/DPI/driver |
| **Total** | **50 ms** | |

### 4.3 Design rules that protect the budget

- The popup process and window are created at login, hidden. The hotkey path
  must not spawn a process or create a window.
- The core sends a **snapshot** of the current view (last query/group, first
  page, cached) with `ShowPopup`; the popup paints immediately and refreshes
  asynchronously.
- No synchronous DB open, migration, thumbnail decode, or font atlas rebuild on
  the show path.
- The search field must be ready to accept input on the first frame.
- The optional fade starts after the first frame; it must not gate the measured
  present.
- If the popup is not responding, the core shows it anyway (it owns the window
  show) or fails with a clear error; it must not wait >50 ms before showing.

### 4.4 Failure handling

| Result | Action |
|---|---|
| p95 50–80 ms | Profile the five segments; fix the dominant one; re-measure |
| p95 > 80 ms | Consider a pre-rendered first frame, a smaller initial page, or a native first paint; do not hide the regression |
| Any sample > 150 ms | Treat as a fail; find the blocking call (DB, thumbnail, font, AV) |

---

## 5. PF-3 — 10k-clip search

### 5.1 Fixture

Generate a synthetic DB with **10,000** clips:

- 70% English text (10–2,000 chars), 20% Chinese text (10–2,000 chars), 10%
  mixed;
- 20% with a second format (HTML or RTF);
- 5% images (small PNG/DIB) for preview tests;
- 50 groups in a 3-level tree;
- 200 pinned clips;
- 5 copy buffers assigned;
- 100 clips with a quick-paste text;
- timestamps spread over 2 years;
- deterministic seed; the fixture generator lives in `tests/fixtures` (T6) or
  `tools/qa` (T7), not in production code.

### 5.2 Queries

Run each query 30 times after 5 warm-ups; report p50/p95/p99:

| Query | Expected path |
|---|---|
| Empty query, recent, no group | index scan on `clips_recent` |
| Empty query, group filter | `clips_group` |
| English word (≥3 chars), description | FTS trigram |
| Chinese substring (≥3 chars) | FTS trigram |
| 2-character English/CJK query | `LIKE` fallback with limit |
| Full-text phrase | FTS on `text_content` |
| Quick-paste query | FTS on `quick_paste_text` |
| Regex (if shipped) | bounded scan; document the cost |
| Pinned-first sort | same index, pinned branch |
| Deep page (cursor) | keyset, no OFFSET |

### 5.3 Pass/fail

- **p95 < 50 ms** for the required queries (empty browse, description, CJK,
  full-text, quick paste).
- The count query must not double the time; if it does, use a cached count or
  a deferred count badge.
- Regex/wildcard are `Should`; if shipped, document their p95 and a guard
  (maximum scan/time) rather than freezing the UI.
- No query may return a different result set between runs on the same fixture.

### 5.4 Instrumentation

- Core logs `query_id`, mode, `took_ms`, rows returned, and `state_rev` (no
  query text if it may contain sensitive content; log a hash/length instead).
- The popup shows the result count; a slow query shows a progress indicator
  after 150 ms.
- Use `EXPLAIN QUERY PLAN` to confirm index usage for the default sorts and
  group filters. Save the plans in the QA artifacts.

---

## 6. PF-4/PF-5 — 10k scroll and memory growth

### 6.1 Protocol

1. Load the 10k fixture, open the popup, select "All clips".
2. Scroll continuously for 60 seconds (keyboard PgDn/End and mouse wheel),
   including a jump to the top/bottom and a group switch.
3. Record frame times with the egui frame-time hook or PresentMon. Report p50,
   p95, p99, max, and the number of frames > 16.7 ms / > 50 ms.
4. Sample popup private working set at t=0, 30 s, 60 s. Growth must be **< 5 MB**
   and must not trend upward after the first full pass.
5. Repeat with a search active and with images in view.

### 6.2 Pass/fail

- **60 fps**: p99 frame ≤ 16.7 ms, no frame > 50 ms (a single first-frame
  hitch during initial load is excluded if documented).
- **No memory growth**: < 5 MB over 60 s; no unbounded texture/font/row cache.
- No handle growth (check process handle count before/after).

### 6.3 Design rules

- Virtualize rows: only visible + 3 overscan.
- Cache fonts, icon atlas, and thumbnails; bound the thumbnail cache by bytes
  and evict LRU.
- Do not allocate a new texture/brush per row per frame.
- Do not lay out all 10k rows on every frame; reuse layout for unchanged rows.
- Decode thumbnails off the UI thread; show a placeholder until ready.

---

## 7. PF-6/PF-7 — Startup and pre-warm

### 7.1 Core ready

- `t0` = process start; `t1` = tray icon visible + hotkeys registered +
  clipboard listener active + IPC accepting.
- Target **< 1 s p95**, max < 2 s, on the QA machine, release build, no
  migration.
- A first run with migration is excluded from this budget; migration shows its
  own progress and is measured separately.
- Log timestamps at each startup step (`ARCHITECTURE.md` §5.1) and export them
  in the diagnostics bundle.

### 7.2 Popup pre-warm

- The core spawns the popup after the core is ready. Pre-warm is complete when
  the popup has created its hidden window, loaded fonts/icon atlas, and
  answered `HelloAck`.
- Target **< 2 s p95** after login; it must not run on the hotkey path.
- If pre-warm fails, the first hotkey pays the cost; log it and retry in the
  background. The popup latency gate uses the pre-warmed state, but a cold
  fallback must still show a window (acceptance: cold first-show < 300 ms,
  documented, not the budget).

---

## 8. PF-8/PF-9 — Settings WebView2 lifecycle

### 8.1 Open

- `t0` = user clicks Settings; `t1` = the settings window is visible and the
  first page is rendered.
- Target **< 1.5 s p95** on the QA machine. WebView2 cold start may be slower on
  first ever open; report both cold and warm.
- The core must not block the popup or clipboard while the webview starts.

### 8.2 Close and release

- Close the window; wait 5 s; confirm no `msedgewebview2.exe` child remains.
- Compare core private working set before open and 10 s after close; the delta
  must be **< 10 MB**. If WebView2 keeps a shared process alive, document which
  process and why; do not kill unrelated WebView2 processes.
- The idle memory gate (PF-1) is measured with settings never opened. If a
  session opened settings and closed it, re-run PF-1 only after confirming
  release.

---

## 9. PF-10 — Migration throughput

- Fixture: 10,000 clips + ~20,000 formats + groups/pins/copy buffers + 100
  small images.
- Measure: total wall time, phases (copy, validate, groups, clips, formats,
  copy buffers, settings, FTS, verify), peak private working set, DB size, blob
  size.
- Target **< 60 s** on the QA SSD; no phase may be quadratic.
- Progress events must fire at least every 500 ms; the UI must remain
  responsive (settings can cancel).
- Validate `PRAGMA quick_check` and counts after migration; time the validation
  separately.
- Record the source and target checksums.

---

## 10. Tooling

| Need | Tool | Notes |
|---|---|---|
| Process memory | `GetProcessMemoryInfo` via a small QA helper; Task Manager / Process Explorer for cross-check | Record private working set, working set, commit |
| Frame times | egui frame-time hook; PresentMon as an independent check | Save raw CSV |
| Latency | `QueryPerformanceCounter` timestamps at the five points in §4.1 | Same machine, same run |
| CPU/IO traces | Windows Performance Recorder + Windows Performance Analyzer | Use for regressions, not every run |
| SQL plans | `EXPLAIN QUERY PLAN`, `.timer on` in the SQLite shell; core query timing | Save plans in artifacts |
| Fixtures | Deterministic generator in `tests/fixtures` / `tools/qa` | No real user data |
| CI | Windows runner; fmt/clippy/test/build; performance informational | Shared runners are not authoritative |

No new production dependency is added for measurement. QA tools live under
`tools/qa/**` (T7) or `tests/fixtures/**` (T6) and are not shipped.

---

## 11. Evidence format

T1/T7 must save:

- `artifacts/rewrite/after/perf/idle-<date>.csv` — three samples, process rows,
  raw values, median.
- `artifacts/rewrite/after/perf/popup-latency-<date>.csv` — 100 rows with
  t0..t4, p50/p95/p99, DPI/monitor.
- `artifacts/rewrite/after/perf/search-10k-<date>.csv` — query, mode, run,
  took_ms, rows, plan.
- `artifacts/rewrite/after/perf/scroll-10k-<date>.csv` — frame times, p99, max,
  memory samples.
- `artifacts/rewrite/after/perf/startup-<date>.log` — step timestamps.
- `artifacts/rewrite/after/perf/settings-lifecycle-<date>.csv` — open/close
  times, process list, memory delta.
- `docs/rewrite/QA-REPORT.md` — summary tables, pass/fail, hardware, build SHA,
  and any exceptions.

Screenshots or traces supplement, but do not replace, the raw numbers.

---

## 12. Regression policy

- Every PR/merge that touches the popup, list, storage, search, IPC, or settings
  must run the relevant smoke measurement, even if only informational.
- T7 runs the full protocol on the integrated branch. A `Must` fail blocks the
  v1 verdict.
- A `Should` fail needs a written exception with the measured number, the cause,
  and the user-visible impact.
- If PF-1 fails, apply the Plan B architecture from `ARCHITECTURE.md` §3; do not
  lower the budget without the user's decision.
- If PF-2 fails because of a platform limitation, document the fallback (e.g.,
  native search field, pre-rendered first frame) and re-measure.
- Performance numbers are versioned with the commit SHA and hardware profile.
  Do not compare numbers across different machines without saying so.
