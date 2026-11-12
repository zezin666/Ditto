# DATA-MODEL — SQLite schema, search, blobs, and legacy migration

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Scope: new SQLite schema, FTS5, blob storage, hashing/dedupe, groups, pinning,
  copy buffers, settings, legacy `Ditto.db` / `Ditto.Settings` mapping,
  migration/backup/rollback.
- Implementation owner: T2 (`crates/ditto-storage`, `crates/ditto-clipboard`)
  and T6 (`crates/ditto-migration`).
- Units: timestamps are Unix epoch milliseconds (`_ms`) unless stated; legacy
  `Main.lDate` / `lastPasteDate` are seconds and must be multiplied by 1000.

---

## 1. Principles

1. **One writer, WAL readers.** Only `Ditto.exe` opens `ditto.db`. The core uses
   one writer connection plus a small read-only pool in WAL mode. The popup and
   settings webview never open SQLite.
2. **Never modify legacy data.** Migration works on a private copy of the old
   DB; the original is opened read-only and checksummed before/after.
3. **Preserve history exactly.** Migration inserts one new clip per legacy
   `Main` row, even if content hashes collide. Dedupe applies to new captures,
   not to imported history.
4. **Raw format fidelity.** Every clipboard format is stored as raw bytes
   (inline or in the blob store) plus a derived preview/thumbnail. We do not
   re-encode the only copy of user data.
5. **Search is indexed, not scanned.** FTS5 with a trigram tokenizer supports
   English and Chinese substrings; short queries (1–2 characters) fall back to
   indexed/LIKE scans.
6. **All SQL is parameterized.** Legacy code built SQL with string formatting;
   the rewrite must never do that. Dynamic identifiers are chosen from a fixed
   allow-list, never from user input.
7. **Crash-safe.** WAL, transactions per capture/mutation, `quick_check` on
   unclean start, verified backups.
8. **Local and private.** No encryption in v1 (deferred), but the data root is
   ACL-restricted to the current user. Logs never contain clip content.

---

## 2. Database settings

At every open:

| Pragma | Value | Why |
|---|---|---|
| `journal_mode` | `WAL` | Concurrent readers, crash recovery |
| `synchronous` | `NORMAL` | WAL-safe and fast |
| `foreign_keys` | `ON` | Cascade correctness |
| `busy_timeout` | `5000` | Tolerate the brief writer lock |
| `auto_vacuum` | `INCREMENTAL` | Reclaim space without a full VACUUM |
| `wal_autocheckpoint` | `1000` pages | Bound WAL growth |
| `temp_store` | `MEMORY` | Search/sort scratch |
| `mmap_size` | `0` or 64 MiB (T2 decides) | Measure; do not assume |
| `analysis_limit` | `400` | `PRAGMA optimize` quality |
| `foreign_keys` | `ON` | Re-stated because it is per-connection |

- `PRAGMA user_version` is the authoritative schema version.
- `PRAGMA quick_check` runs only when `state.json` says the previous shutdown
  was unclean. A full `integrity_check` runs from the Data page and before a
  backup restore.
- `PRAGMA optimize` runs on clean close and after migration/import.
- `PRAGMA wal_checkpoint(PASSIVE)` runs periodically; `TRUNCATE` on clean exit.
- FTS5 must be compiled in. T2 must assert `PRAGMA compile_options` contains
  `ENABLE_FTS5`; if not, startup fails with an actionable error.
- The trigram tokenizer requires SQLite 3.34+. The bundled `rusqlite` version is
  expected to satisfy this; T2 verifies and, if not, falls back to `unicode61`
  plus a CJK `LIKE` path (documented in `MIGRATION-REPORT.md`/`QA-REPORT.md`).

---

## 3. Schema (v1)

The DDL below is normative. `STRICT` tables require SQLite 3.37+; the bundled
version is expected to satisfy it. If T2's bundled version is older, drop
`STRICT` (record the deviation) but keep the types and checks.

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE schema_version (
  version       INTEGER PRIMARY KEY,
  applied_at_ms INTEGER NOT NULL,
  app_version   TEXT    NOT NULL
) STRICT;

CREATE TABLE meta (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT;
-- rows: state_rev, theme_rev, settings_rev, install_id, last_vacuum_ms

CREATE TABLE groups (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT    NOT NULL,
  parent_id     INTEGER REFERENCES groups(id) ON DELETE CASCADE,
  sort_order    REAL    NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  legacy_id     INTEGER UNIQUE
) STRICT;

CREATE UNIQUE INDEX groups_unique_name
  ON groups (COALESCE(parent_id, -1), name COLLATE NOCASE);
CREATE INDEX groups_parent_sort ON groups (parent_id, sort_order, name);

CREATE TABLE clips (
  id                    INTEGER PRIMARY KEY AUTOINCREMENT,
  legacy_id             INTEGER UNIQUE,
  kind                  TEXT    NOT NULL
                          CHECK (kind IN ('text','html','rtf','image','files','custom')),
  primary_format        TEXT,
  text_content          TEXT,
  text_truncated        INTEGER NOT NULL DEFAULT 0 CHECK (text_truncated IN (0,1)),
  preview_text          TEXT    NOT NULL DEFAULT '',
  quick_paste_text      TEXT    NOT NULL DEFAULT '',
  source_app            TEXT,
  source_window_title   TEXT,
  content_hash          BLOB    NOT NULL,
  formats_hash          BLOB    NOT NULL,
  hash_algo             TEXT    NOT NULL DEFAULT 'sha256',
  size_bytes            INTEGER NOT NULL DEFAULT 0,
  is_pinned             INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0,1)),
  pin_order             REAL    NOT NULL DEFAULT 0,
  group_id              INTEGER REFERENCES groups(id) ON DELETE SET NULL,
  sort_order            REAL    NOT NULL DEFAULT 0,
  group_sort_order      REAL    NOT NULL DEFAULT 0,
  is_deleted            INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0,1)),
  deleted_at_ms         INTEGER,
  created_at_ms         INTEGER NOT NULL,
  updated_at_ms         INTEGER NOT NULL,
  last_pasted_at_ms     INTEGER,
  paste_count           INTEGER NOT NULL DEFAULT 0,
  dont_auto_delete      INTEGER NOT NULL DEFAULT 0 CHECK (dont_auto_delete IN (0,1)),
  shortcut              INTEGER,
  global_shortcut       INTEGER NOT NULL DEFAULT 0 CHECK (global_shortcut IN (0,1)),
  move_to_group_shortcut      INTEGER,
  global_move_to_group_shortcut INTEGER NOT NULL DEFAULT 0
                                  CHECK (global_move_to_group_shortcut IN (0,1)),
  has_image             INTEGER NOT NULL DEFAULT 0 CHECK (has_image IN (0,1)),
  thumb_path            TEXT,
  legacy_crc            INTEGER,
  legacy_meta           TEXT
) STRICT;

CREATE INDEX clips_recent
  ON clips (is_deleted, is_pinned DESC, pin_order DESC, sort_order DESC, id DESC);
CREATE INDEX clips_group
  ON clips (group_id, is_deleted, is_pinned DESC, pin_order DESC, group_sort_order DESC, id DESC);
CREATE INDEX clips_hash
  ON clips (content_hash, formats_hash, is_deleted);
CREATE INDEX clips_shortcut
  ON clips (shortcut) WHERE shortcut IS NOT NULL;
CREATE INDEX clips_global_shortcut
  ON clips (global_shortcut) WHERE global_shortcut = 1;
CREATE INDEX clips_deleted
  ON clips (is_deleted, deleted_at_ms) WHERE is_deleted = 1;

CREATE TABLE clip_formats (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  clip_id       INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
  format_name   TEXT    NOT NULL,
  seq           INTEGER NOT NULL DEFAULT 0,
  size_bytes    INTEGER NOT NULL DEFAULT 0,
  data          BLOB,
  data_path     TEXT,
  sha256        BLOB,
  is_primary    INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0,1)),
  legacy_data_id INTEGER UNIQUE,
  UNIQUE (clip_id, format_name, seq),
  CHECK (data IS NOT NULL OR data_path IS NOT NULL)
) STRICT;

CREATE INDEX clip_formats_clip ON clip_formats (clip_id, seq);
CREATE INDEX clip_formats_name ON clip_formats (format_name);
CREATE INDEX clip_formats_sha  ON clip_formats (sha256);
CREATE INDEX clip_formats_legacy ON clip_formats (legacy_data_id);

CREATE TABLE copy_buffers (
  slot          INTEGER PRIMARY KEY CHECK (slot BETWEEN 1 AND 5),
  clip_id       INTEGER REFERENCES clips(id) ON DELETE SET NULL,
  name          TEXT    NOT NULL DEFAULT '',
  updated_at_ms INTEGER,
  legacy_id     INTEGER
) STRICT;

CREATE TABLE settings (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,          -- JSON value
  updated_at_ms INTEGER NOT NULL,
  revision      INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE migration_state (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  source_path     TEXT    NOT NULL,
  source_sha256   BLOB    NOT NULL,
  dry_run         INTEGER NOT NULL DEFAULT 0 CHECK (dry_run IN (0,1)),
  phase           TEXT    NOT NULL,
  started_at_ms   INTEGER NOT NULL,
  updated_at_ms   INTEGER NOT NULL,
  completed_at_ms INTEGER,
  counts_json     TEXT,
  error           TEXT
) STRICT;

CREATE VIRTUAL TABLE clips_fts USING fts5(
  preview_text,
  quick_paste_text,
  text_content,
  content = 'clips',
  content_rowid = 'id',
  tokenize = 'trigram'
);
```

### 3.1 FTS triggers

External-content FTS5 requires explicit triggers. The `delete` command is the
special FTS5 external-content form; do not use a plain `DELETE FROM clips_fts`
for a contentless/external table.

```sql
CREATE TRIGGER clips_ai AFTER INSERT ON clips WHEN new.is_deleted = 0 BEGIN
  INSERT INTO clips_fts(rowid, preview_text, quick_paste_text, text_content)
  VALUES (new.id, new.preview_text, new.quick_paste_text, coalesce(new.text_content, ''));
END;

CREATE TRIGGER clips_ad AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, preview_text, quick_paste_text, text_content)
  VALUES ('delete', old.id, old.preview_text, old.quick_paste_text, coalesce(old.text_content, ''));
END;

CREATE TRIGGER clips_au AFTER UPDATE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, preview_text, quick_paste_text, text_content)
  VALUES ('delete', old.id, old.preview_text, old.quick_paste_text, coalesce(old.text_content, ''));
  INSERT INTO clips_fts(rowid, preview_text, quick_paste_text, text_content)
  SELECT new.id, new.preview_text, new.quick_paste_text, coalesce(new.text_content, '')
  WHERE new.is_deleted = 0;
END;
```

- Soft delete (`is_deleted = 1`) removes the row from FTS; restore re-inserts it.
- `text_content` is capped at **1 MiB UTF-8** for FTS. Larger text is stored in
  the blob store and `text_truncated = 1`; search beyond 1 MiB is out of scope.
- `preview_text` is capped at 8 KiB; `quick_paste_text` at 8 KiB.
- The trigram index is larger than the text (roughly 3–5×). The DB budget in
  `PERFORMANCE.md` accounts for it.

### 3.2 Why a normalized `copy_buffers` table

Legacy stores copy-buffer assignments in `CopyBuffers(lID, lClipID,
lCopyBuffer)`, where `lCopyBuffer` is **0-based** (0..4). The new table uses
**1-based** slots (1..5) to match the UI labels. It is the single source of
truth; `ClipSummary.copy_buffer` is derived by joining `copy_buffers` on
`clip_id`. This avoids two places that can disagree. The coordinator's appendix
sketches `clips.copy_buffer`; the normalized table is the recommended shape
(recorded in `DECISIONS.md`).

### 3.3 Why `dont_auto_delete` is separate from `pinned`

The coordinator's appendix maps `Main.lDontAutoDelete` to `clips.pinned`.
Those are different legacy concepts:

- `lDontAutoDelete` protects a clip from retention cleanup.
- `stickyClipOrder` / `stickyClipGroupOrder` (non-sentinel) make a clip sticky
  to the top, which is what users call "pinned".

The new schema preserves both: `dont_auto_delete` from `lDontAutoDelete`, and
`is_pinned`/`pin_order` from the sticky columns. If the coordinator prefers the
appendix's simplification, map both legacy flags to `is_pinned` and keep
`dont_auto_delete`; no data is lost either way. `DECISIONS.md` records the
recommendation.

---

## 4. Clip kinds, formats, and blob storage

### 4.1 Kinds

| `kind` | Primary evidence | Primary format |
|---|---|---|
| `text` | CF_UNICODETEXT or CF_TEXT | `CF_UNICODETEXT` |
| `html` | `HTML Format` | `HTML Format` |
| `rtf` | `Rich Text Format` | `Rich Text Format` |
| `image` | PNG, CF_DIB, CF_DIBV5 | `PNG` if present, else `CF_DIB` |
| `files` | CF_HDROP | `CF_HDROP` |
| `custom` | Anything else | first registered format |

A clip is one logical copy event with one or more formats. The same logical
copy may include text + HTML + RTF + PNG; all are stored in `clip_formats` and
the clip's `kind` is the dominant one for display.

### 4.2 Format names

Store the exact Windows clipboard format name using the same stable names as
legacy `GetFormatName`:

`CF_TEXT`, `CF_OEMTEXT`, `CF_UNICODETEXT`, `CF_DIB`, `CF_DIBV5`, `CF_HDROP`,
`CF_HTML`? (legacy uses `HTML Format`), `Rich Text Format`, `PNG`,
`Ditto File Data`, and registered custom names.

Do not localize format names; they are data keys.

### 4.3 Inline vs blob

| Data | Storage |
|---|---|
| Text/preview/quick paste | In `clips` columns (capped as above) |
| Format bytes ≤ 64 KiB | Inline `clip_formats.data` |
| Format bytes > 64 KiB | `clip_formats.data_path` → `blobs/<aa>/<bb>/<sha256>.<ext>` |
| Images (any size above the inline threshold) | Blob store; original bytes preserved |
| Thumbnails | `thumbs/<sha256>-<w>.png`; `clips.thumb_path` |
| Large text | Blob store for the CF_UNICODETEXT bytes; FTS text capped at 1 MiB |

- `data_path` is **relative** to the data root, using `/` separators in the DB;
  resolve with the data root at read time. This keeps the DB portable.
- Blob file name is the SHA-256 of the bytes; the extension is from the format
  (`.png`, `.dib`, `.html`, `.rtf`, `.txt`, `.bin`).
- Blob GC: delete a blob only when no `clip_formats` row references its
  `sha256`/path. Run at startup and weekly, in a background task.
- Thumbnails are derived; they can be regenerated and are not part of export
  integrity.
- Do not store the only copy of a format only as a thumbnail. The raw bytes
  are authoritative.

### 4.4 Image conversion

- CF_DIB / CF_DIBV5 are converted to PNG for preview/thumbnail and, when
  practical, stored as a PNG format **in addition to** the original DIB bytes.
  The original DIB is preserved for paste fidelity.
- A DIB without a valid header is stored raw with `kind='custom'` and a
  migration warning; do not crash.
- Maximum decode dimensions are bounded (e.g., 16384×16384) to prevent
  decompression bombs; oversized images are stored raw without a thumbnail.

---

## 5. Hashing, dedupe, and ordering

### 5.1 Hashes

- Algorithm: **SHA-256** (32-byte BLOB). BLAKE3 is acceptable if T2 prefers it,
  but pick one and keep the algorithm in `hash_algo`; the coordinator's default
  allows either. SHA-256 is recommended for content-addressed paths and
  interoperability.
- `content_hash` = SHA-256 of a deterministic serialization of the **primary
  format**:
  `format_name || 0x00 || raw_bytes`.
  Primary selection order: `CF_UNICODETEXT` → `HTML Format` → `Rich Text
  Format` → `PNG` → `CF_DIB` → `CF_HDROP` → first format by sequence.
- `formats_hash` = SHA-256 over the sorted list of `(format_name, sha256)` pairs
  for every stored format. This distinguishes "same text, different formats".
- For text, normalize line endings for the **dedupe key only** (CRLF/CR → LF);
  never rewrite the stored bytes. Trim a single trailing NUL in
  CF_UNICODETEXT for hashing.
- Legacy `CRC` (32-bit) is preserved in `legacy_crc`; it is not used for new
  dedupe.

### 5.2 Duplicate policy

When a new clip arrives:

1. If `allow_duplicates` is true, insert a new row.
2. If the same `content_hash` and `formats_hash` exist in a non-deleted row:
   - Update `updated_at_ms` and `sort_order` (and `group_id` if captured into
     the current group), broadcast `ClipUpdated` with reason `duplicate`.
   - Increment `paste_count` only on paste, not on duplicate capture.
   - Do not insert a second row.
3. Back-to-back duplicate (same hash as the immediately previous clip within
   the configured window) is ignored even if `allow_duplicates` is true, unless
   `allow_back_to_back_duplicates` is enabled. This mirrors legacy behavior and
   prevents copy loops.
4. Migration **bypasses** dedupe: one new clip per legacy `Main` row, so
   historical counts and duplicates are preserved.

Self-write suppression is in-memory: before Ditto writes the clipboard, it
records the expected `(content_hash, formats_hash)`; the next clipboard
notification with that pair is ignored and the record is cleared.

### 5.3 Ordering and pinning

| Sort | SQL shape |
|---|---|
| Recent (default) | `is_pinned DESC, pin_order DESC, sort_order DESC, id DESC` |
| Oldest | `is_pinned DESC, pin_order DESC, sort_order ASC, id ASC` |
| Pinned first | same as recent |
| Group order | `is_pinned DESC, pin_order DESC, group_sort_order DESC, id DESC` |

- `sort_order` is a REAL. New captures use `created_at_ms` as a REAL; migrated
  top-level clips use legacy `clipOrder` when present, else `lDate * 1000`.
- `group_sort_order` is used only for clips inside a group; migrated clips use
  legacy `clipGroupOrder`.
- `is_pinned` / `pin_order` come from the legacy sticky columns; new pins get
  `pin_order = max(pin_order) + 1` (or a fractional midpoint for insert).
- Tie-break with `id DESC` for deterministic pagination.
- Pinned clips are never auto-deleted by retention.

### 5.4 Soft delete and retention

- `Delete` marks `is_deleted = 1`, `deleted_at_ms = now`, removes the FTS row,
  and broadcasts `ClipDeleted`. It does not delete blobs immediately.
- Undo within 7 days restores `is_deleted = 0` and re-inserts the FTS row.
- Purge (hard delete) after `deleted_retention_days`; `ON DELETE CASCADE`
  removes formats; blob GC runs after.
- Retention cleanup selects rows where:
  `is_deleted = 0 AND is_pinned = 0 AND dont_auto_delete = 0 AND slot is null`
  and ordered by `sort_order ASC`, beyond `max_entries` or older than
  `expire_days`. A clip in a copy buffer or with a global shortcut is
  protected.
- Group rows are never deleted by retention. Deleting a group sets its clips'
  `group_id` to NULL (`UNGROUPED`) after user confirmation; child groups are
  deleted by cascade in the same transaction.

---

## 6. Settings

`settings.value` is a JSON value. Only overrides are stored; defaults live in
code (`settings_defaults`) and are merged by `GetSettings`. Keys are stable and
namespaced.

| Namespace | Keys (v1) |
|---|---|
| `general` | `language` (`en`/`zh-CN`), `run_at_startup`, `start_minimized`, `show_tray_icon`, `startup_message`, `default_paste_mode` |
| `appearance` | `theme` (`system`/`light`/`dark`/`hc`), `accent` (`system`/`ditto`/`custom`), `accent_value`, `density`, `popup_width`, `popup_height`, `popup_position` (`caret`/`cursor`/`previous`), `lines_per_clip` (1–5), `font_scale` (100–150), `preview` (`off`/`auto`/`f3`), `animations`, `always_on_top`, `hide_on_paste`, `zebra_rows`, `show_grouped_in_main` |
| `hotkeys` | `open`, `open_alt`, `plain_text_paste`, `first_ten_enabled`, `first_ten_start`, `first_ten_show_text`, `first_ten_send_paste`, `copy_buffer.1.copy/paste/cut` … `copy_buffer.5.*` |
| `clipboard` | `capture_formats`, `max_clip_bytes`, `max_entries`, `expire_days`, `enforce_max_entries`, `enforce_expire`, `allow_duplicates`, `allow_back_to_back_duplicates`, `update_time_on_paste`, `ignore_cf_dib`, `include_apps`, `exclude_apps`, `save_delay_ms`, `process_delay_ms`, `paste_delay_ms`, `send_paste_after_selection`, `restore_clipboard`, `restore_delay_ms`, `paste_as_admin`, `multi_paste_separator`, `supported_types` |
| `groups` | `show_grouped_in_main`, `prompt_for_name`, `default_group_id` |
| `copy_buffers` | `play_sound`, `slot_names` |
| `search` | `mode`, `description`, `fulltext`, `quickpaste`, `regex_case_insensitive` |
| `data` | `backup_count`, `backup_interval_hours`, `last_backup_ms`, `deleted_retention_days` |
| `diagnostics` | `log_level` |
| `migration` | `completed`, `source_path`, `source_sha256`, `completed_at_ms`, `dry_run` |

- `data.path` is **not** a user-editable setting in v1; the data root is
  resolved at startup. The Data page shows it and offers "open folder", not an
  arbitrary path picker. Import/export/backup paths are chosen through the
  native file dialog and validated by the core.
- Settings writes increment `meta.settings_rev` and the row `revision`, and
  broadcast `SettingsChanged`.
- Restart-required keys: `general.language`, `data.path`, `general.run_at_startup`
  (path repair only). The UI must show it.
- Secret values are not stored in v1. If future network features return, use
  DPAPI and never put plaintext in `settings`.

---

## 7. Legacy `Ditto.db` mapping

### 7.1 Legacy schema (as observed at `master` @ `4719eea`)

`src/DatabaseUtilities.cpp::CreateDB` creates:

| Table | Columns |
|---|---|
| `Main` | `lID`, `lDate`, `mText`, `lShortCut`, `lDontAutoDelete`, `CRC`, `bIsGroup`, `lParentID`, `QuickPasteText`, `clipOrder`, `clipGroupOrder`, `globalShortCut`, `lastPasteDate`, `stickyClipOrder`, `stickyClipGroupOrder`, `MoveToGroupShortCut`, `GlobalMoveToGroupShortCut` |
| `Data` | `lID`, `lParentID`, `strClipBoardFormat`, `ooData` |
| `Types` | `lID`, `TypeText` |
| `CopyBuffers` | `lID`, `lClipID`, `lCopyBuffer` |
| `MainDeletes` | `clipID`, `modifiedDate` (legacy trigger bookkeeping) |

Older databases may lack later columns. Migration must inspect
`PRAGMA table_info` and default missing columns rather than assuming the newest
schema.

### 7.2 Field mapping

| Legacy | New | Transform / notes |
|---|---|---|
| `Main.lID` | `clips.id` | New autoincrement id; preserve old id in `clips.legacy_id` |
| `Main.lDate` | `clips.created_at_ms` | seconds × 1000 |
| `Main.mText` | `clips.preview_text` | UTF-8 text; cap 8 KiB for storage; derive preview if empty |
| `Data` text formats | `clips.text_content` | Prefer `CF_UNICODETEXT`, then `CF_TEXT`; cap 1 MiB for FTS |
| `Main.lShortCut` | `clips.shortcut` | raw integer; preserve |
| `Main.globalShortCut` | `clips.global_shortcut` | 0/1 |
| `Main.lDontAutoDelete` | `clips.dont_auto_delete` | 0/1; **not** the same as pinned (see §3.3) |
| `Main.CRC` | `clips.legacy_crc` | 32-bit; recompute `content_hash`/`formats_hash` |
| `Main.bIsGroup` | `groups` vs `clips` | 1 → a row in `groups`; 0 → a row in `clips` |
| `Main.lParentID` | `groups.parent_id` / `clips.group_id` | `-1` → NULL (root/ungrouped); otherwise map old group id |
| `Main.QuickPasteText` | `clips.quick_paste_text` | cap 8 KiB |
| `Main.clipOrder` | `clips.sort_order` | REAL; fallback `lDate * 1000` |
| `Main.clipGroupOrder` | `clips.group_sort_order` | REAL; fallback `lDate * 1000` |
| `Main.stickyClipOrder` | `clips.is_pinned`, `clips.pin_order` | non-sentinel → pinned; sentinel `-2147483647` / NULL → not pinned |
| `Main.stickyClipGroupOrder` | pinned for clips in groups | same sentinel rule |
| `Main.lastPasteDate` | `clips.last_pasted_at_ms` | seconds × 1000; 0/NULL → NULL |
| `Main.MoveToGroupShortCut` | `clips.move_to_group_shortcut` | raw; UI deferred |
| `Main.GlobalMoveToGroupShortCut` | `clips.global_move_to_group_shortcut` | 0/1; UI deferred |
| `Data.lID` | `clip_formats.legacy_data_id` | preserve for idempotent reruns |
| `Data.lParentID` | `clip_formats.clip_id` | map via `Main.lID` → `clips.id` |
| `Data.strClipBoardFormat` | `clip_formats.format_name` | exact stable name |
| `Data.ooData` | `clip_formats.data` / `data_path` | raw bytes; inline ≤ 64 KiB, else blob |
| `Types.TypeText` | `settings.clipboard.supported_types` | JSON array; no separate table |
| `CopyBuffers.lClipID` | `copy_buffers.clip_id` | `-1` → NULL (empty slot); map via `legacy_id` |
| `CopyBuffers.lCopyBuffer` | `copy_buffers.slot` | legacy 0..4 → new 1..5 (add 1) |
| `MainDeletes` | not migrated | delete bookkeeping; no equivalent needed |

### 7.3 Group tree reconstruction

1. Insert all `Main.bIsGroup = 1` rows into `groups` in parent-before-child
   order. Use `legacy_id` to map.
2. For a group, `parent_id = lParentID == -1 ? NULL : map[lParentID]`.
   A missing parent is a data error; attach to NULL and report it.
3. Insert clips: `group_id = lParentID == -1 ? NULL : map[lParentID]`.
4. Preserve `groups.sort_order` from the group's `clipOrder`/`lDate`; use
   `clipGroupOrder` when present.
5. Detect cycles (a parent chain that loops) and break them by attaching the
   offending group to NULL; report in the migration report. Never recurse
   without a depth limit.

### 7.4 Pin and copy-buffer details

- Legacy "not sticky" sentinel is `-(2147483647)` (and `NULL`/`0` in some
  versions). Treat `NULL`, `0`, and the sentinel as not pinned.
- For a top-level clip use `stickyClipOrder`; for a clip in a group use
  `stickyClipGroupOrder`. If both are non-sentinel, prefer the one that matches
  the clip's parent (group vs top-level).
- Legacy copy buffers are 0-based: `0..4`. New slots are 1..5. Empty slots are
  `lClipID = -1` or no row. Preserve slot names as empty by default; the legacy
  UI had no names.
- If two legacy rows claim the same slot, keep the latest by `CopyBuffers.lID`
  and report a conflict; the DB primary key would otherwise reject it.

### 7.5 Legacy settings mapping (`Ditto.Settings` INI, registry fallback)

The legacy file is INI-like, section `[Ditto]`, usually UTF-16LE with a BOM
(`CreateIniFile`); registry fallback is `HKCU\Software\Ditto` with the same key
names. Migration reads, never writes, both.

| Legacy key | New setting | Transform |
|---|---|---|
| `DittoHotKey` | `hotkeys.open` | raw `RegisterHotKey` value; preserve |
| `DittoHotKey2`, `DittoHotKey3` | `hotkeys.open_alt` | array; preserve if present |
| `DBPath3` / `DBPath` | `migration.source_path` | source detection only; new data root is fixed |
| `Theme` | `appearance.theme` | map `Modern Light`/`Monolight`/light → light; dark variants → dark; empty/unknown → system; custom XML is not migrated |
| `LanguageFile` | `general.language` | `English` → `en`; `Chinese Simplified` → `zh-CN`; other → `en` + note |
| `MaxEntries` | `clipboard.max_entries` | integer |
| `ExpiredEntries` | `clipboard.expire_days` | integer |
| `CheckForMaxEntries` | `clipboard.enforce_max_entries` | bool |
| `CheckForExpiredEntries` | `clipboard.enforce_expire` | bool |
| `AllowDuplicates` | `clipboard.allow_duplicates` | bool |
| `UpdateTimeOnPaste` | `clipboard.update_time_on_paste` | bool |
| `MaxClipSizeInBytes` | `clipboard.max_clip_bytes` | integer |
| `SaveClipDelay` | `clipboard.save_delay_ms` | integer |
| `ProcessDrawClipboardDelay` | `clipboard.process_delay_ms` | integer |
| `PasteAsAdmin` | `clipboard.paste_as_admin` | bool |
| `MultiPasteSeparator` | `clipboard.multi_paste_separator` | string; `[LF]` → `\r\n` |
| `SendPasteAfterSelection` | `clipboard.send_paste_after_selection` | bool |
| `RestoreClipboardDelay` | `clipboard.restore_delay_ms` | integer |
| `HideDittoOnPaste` | `appearance.hide_on_paste` | bool |
| `ShowIconInSystemTray` | `general.show_tray_icon` | bool |
| `LinesPerRow` | `appearance.lines_per_clip` | clamp 1–5 |
| `QuickPastePosition` | `appearance.popup_position` | 1/2/3 → caret/cursor/previous |
| `QuickPasteSize` | `appearance.popup_width/height` | parse `(w,h)`; clamp to new min/max |
| `UseCtrlNumForFirstTenHotKeys` | `hotkeys.first_ten_enabled` | bool |
| `FirstTenHotKeysStart` | `hotkeys.first_ten_start` | integer |
| `ShowTextForFirstTenHotKeys` | `hotkeys.first_ten_show_text` | bool |
| `SendPasteOnFirstTenHotKeys` | `hotkeys.first_ten_send_paste` | bool |
| `CopyBufferCopyHotKey_0..4` | `hotkeys.copy_buffer.{1..5}.copy` | 0-based → 1-based |
| `CopyBufferPasteHotKey_0..4` | `hotkeys.copy_buffer.{1..5}.paste` | 0-based → 1-based |
| `CopyBufferCutHotKey_0..4` | `hotkeys.copy_buffer.{1..5}.cut` | 0-based → 1-based |
| `CopyBufferPlaySound_0..4` | `copy_buffers.play_sound` | bool; aggregate |
| `SearchDescription` | `search.description` | bool |
| `SearchFullText` | `search.fulltext` | bool |
| `SearchQuickPaste` | `search.quickpaste` | bool |
| `SimpleTextSearch` | `search.mode` | true → `contains`; false + regex false → `wildcard` |
| `RegExTextSearch` | `search.mode` | true → `regex` |
| `ShowAllClipsInMainList` / `ShowGroupsInMainList` | `groups.show_grouped_in_main` | bool |
| `PromptForNewGroupName` | `groups.prompt_for_name` | bool |
| `EnableDebugLogging` | `diagnostics.log_level` | true → `debug`, false → `info` |
| `PlaySoundOnCopy` | `copy_buffers.play_sound` | sound file path or bool; v1 uses a system sound |
| `IncludeApps` / `ExcludeApps` | `clipboard.include_apps` / `exclude_apps` | split list; process names |
| `DittoRestoreClipboardDelay` | `clipboard.restore_delay_ms` | integer |
| `NoFormatsRetryDelay` | `clipboard.no_formats_retry_ms` | integer |
| `MainDeletesDeleteCount` | `data.deleted_retention_days` | approximate; keep default 7 |
| Network/friends keys (`Port`, `Password`, `SendClients*`, `AllowFriends`) | not migrated | out of v1 |
| Add-in / ChaiScript keys | not migrated | out of v1 |
| Update keys | not migrated | out of v1 |
| `MainHWND`, window handles, `LastUpdate`, `UpdateFilePath` | not migrated | machine/session-specific |

Rules:

- Unknown keys are ignored and listed in the migration report.
- Do not migrate the legacy DB path into the new data root; the new data root is
  resolved by `ARCHITECTURE.md` §6. Keep the old path in
  `migration.source_path`.
- Do not migrate passwords, window handles, add-in/script paths, theme/language
  XML, or network settings.
- Booleans accept `0/1`, `true/false`, and empty/absent → default.

---

## 8. Legacy format decoding

Migration must preserve raw bytes and derive only what the UI needs.

| Format | Raw handling | Derived |
|---|---|---|
| `CF_UNICODETEXT` | UTF-16LE bytes, often NUL-terminated | `text_content` (strip one trailing NUL), preview |
| `CF_TEXT` / `CF_OEMTEXT` | ANSI bytes (system code page) | Decode with Windows-1252/ACP via `encoding_rs`; if invalid, keep bytes and mark a warning |
| `HTML Format` | CF_HTML header + fragment | Extract fragment using `StartFragment`/`EndFragment`; strip tags for preview; preserve raw |
| `Rich Text Format` | RTF bytes | Extract plain text if a safe parser is available; otherwise derive preview from CF_UNICODETEXT/CF_TEXT; preserve raw |
| `CF_DIB` / `CF_DIBV5` | BITMAPINFOHEADER + pixels | Decode to PNG for preview/thumbnail; preserve raw DIB |
| `PNG` | PNG bytes | Dimensions + thumbnail; preserve raw |
| `CF_HDROP` | DROPFILES struct | File list (wide flag aware), paths for display; preserve raw |
| Custom | Registered format bytes | Length + hex/hash; no interpretation |

- Never execute or load content from a format (no HTML rendering with scripts,
  no RTF objects, no shell paths).
- If a conversion fails, the clip is still migrated with raw bytes and a
  `legacy_meta.conversion_warnings` entry. Migration does not drop the clip.
- CF_UNICODETEXT may contain embedded NULs in malformed data; truncate at the
  first NUL for preview, preserve bytes for paste.
- CF_HDROP parsing must bound `pFiles` and the number of paths; malformed
  structures are preserved raw and reported.

---

## 9. Migration plan

### 9.1 Preconditions

- The old DB is discovered from:
  1. `Ditto.Settings` `DBPath3`/`DBPath` (resolve environment variables);
  2. `%APPDATA%\Ditto\Ditto.db`;
  3. `<exe_dir>\Ditto.db` for portable;
  4. `%LOCALAPPDATA%\Ditto_WindowsApp\Ditto.db` or
     `%LOCALAPPDATA%\Ditto_ChocolateyApp\Ditto.db` for those variants.
- The user may also choose a DB file explicitly in Settings → Data.
- Never use the real `%APPDATA%\Ditto` during development/QA. Use a synthetic
  fixture or a user-provided copy; delete temp copies after the test.
- Check free disk space (at least 2× the old DB size + blob estimate).
- Check the source DB is not locked by a running legacy Ditto; if it is, ask the
  user to exit it, or copy the file with a retry. Do not kill the process.

### 9.2 Phases

| Phase | Action | Commit |
|---|---|---|
| `preflight` | Resolve source, check size/space, `PRAGMA quick_check` on a copy | — |
| `copy_source` | Copy DB + `-wal` + `-shm` to a temp dir; SHA-256 the original and copy | — |
| `validate` | Inspect `PRAGMA table_info`; require `Main`/`Data`; detect optional tables/columns | — |
| `groups` | Insert groups parent-before-child; map legacy ids | transaction per batch |
| `clips` | Insert clips; map parent/group, timestamps, pin, shortcut, flags | transaction per 500 |
| `formats` | Insert formats; decode/derive; write blobs/thumbnails | transaction per 500 clips |
| `copy_buffers` | Insert slots 1..5; map legacy 0..4 | one transaction |
| `types` | Read `Types.TypeText` into settings | one transaction |
| `settings` | Map `Ditto.Settings`/registry keys | one transaction |
| `fts_verify` | Rebuild/verify FTS; smoke search | transaction |
| `verify` | Count/hash/sampling checks; write report | — |
| `done` | Mark `migration_state.completed_at_ms`; set `migration.completed` | — |

- `migration_state` records the phase, counts, and last error. A rerun skips
  rows whose `legacy_id`/`legacy_data_id` already exist (idempotent).
- The source copy is read-write only in the temp dir if WAL recovery is needed;
  the original is read-only. The original's SHA-256 before/after must match.
- Progress events: `StartMigration`, `GetMigrationStatus`, and
  `MigrationProgress` (additive IPC messages) report phase, processed, total,
  percent, and a non-content message. The settings Data page shows a progress
  bar and a cancel button. Cancel rolls back the current batch and leaves the
  new DB in a resumable state.
- Dry run: perform `preflight`/`validate` + count/hash sampling; write a report;
  do not insert. The report includes table counts, group counts, pinned counts,
  copy-buffer slots, format distribution, and estimated blob size.
- Migration is not automatic on first launch without consent. The first run
  shows a welcome page: "Migrate existing Ditto data" / "Start fresh". The
  default is migrate if a legacy DB is found, with a dry-run summary first.

### 9.3 Validation

- Counts:
  - clips: `SELECT COUNT(*) FROM Main WHERE bIsGroup = 0` vs new non-group
    clips with `legacy_id IS NOT NULL`;
  - groups: `COUNT(*) WHERE bIsGroup = 1`;
  - formats: `COUNT(*) FROM Data` vs imported `clip_formats`;
  - copy buffers: non-empty legacy slots vs new slots;
  - types: distinct `TypeText` vs settings array.
- Content:
  - Sample at least 100 clips (or 1% for large DBs), deterministic by id;
    compare raw format SHA-256, text content, pinned, group, copy buffer, and
    timestamps.
  - Recompute `content_hash`/`formats_hash`; verify no empty primary format.
  - Verify every group parent exists and the tree has no cycles.
- Search smoke:
  - A CJK query of ≥3 characters, an English word, and a quick-paste query
    return the expected sample.
- Storage:
  - Every `data_path` exists and its SHA-256 matches; no orphan blob is
    required (orphans are allowed and GC'd).
  - DB `PRAGMA quick_check` passes after migration.
- Report: `docs/rewrite/MIGRATION-REPORT.md` (T6) with counts, samples,
  warnings, source checksum, duration, and the exact commands used.

### 9.4 Backup, rollback, and restore

- Before migrating into a non-empty new DB, create a verified backup:
  `backups/ditto.<utc>.db` via the SQLite backup API or `VACUUM INTO`, then
  `PRAGMA quick_check` on the backup.
- If migration fails:
  - Fresh DB: close, move the partial DB and blobs to
    `backups/failed-<utc>/`, and leave the legacy data untouched; the user can
    retry.
  - Existing DB: close, replace with the pre-migration backup, verify, restart.
- Keep the last 3 verified backups; rotate by age. A backup is never deleted
  until its replacement passes verification.
- Restore is a Data-page action with a confirmation dialog; it requires a
  restart. Restore validates the manifest/schema before replacing anything.
- The source legacy DB is never restored/overwritten; it is read-only.

### 9.5 Import/export (new format)

- Export is a ZIP or directory containing:
  - `ditto.db` (consistent snapshot; `VACUUM INTO` or backup API),
  - `manifest.json` with `schema_version`, app version, created_at, counts,
    SHA-256 of the DB and each blob,
  - `blobs/` and `thumbs/` (optional; thumbs can be regenerated).
- Import validates the manifest and schema, checks hashes, and imports into a
  new DB or merges with id remapping. It never overwrites an existing DB
  without a backup. Merge policy: skip exact `(content_hash, formats_hash)`
  duplicates, remap groups by name/path, preserve timestamps.
- Export/import paths are chosen in the native file dialog; the core validates
  they are absolute and within the selected dialog result. No arbitrary path
  is accepted over IPC.

### 9.6 Legacy DB corruption

- If `PRAGMA quick_check` fails on the source copy:
  - Do not modify the original.
  - Offer a read-only salvage mode that copies readable rows table-by-table,
    reporting failures; never claim full fidelity.
  - If salvage is declined, abort with the checksum and path so the user can
  back up the file.
- If the source has a hot WAL, copy `-wal`/`-shm` and open the copy to recover;
  if the WAL is corrupt, ignore it and open the main DB read-only, reporting
  the loss of uncheckpointed rows.
- Never run `VACUUM`, `REINDEX`, or write pragmas on the original.

---

## 10. Concurrency and transactions

- Writer: one connection, serialized by a `Mutex`/actor. Each capture is one
  transaction: insert clip, insert formats, write blobs (before commit or after
  with a pending state), update FTS via triggers.
- Readers: 2–4 read-only connections in WAL mode for search/preview. If T2
  finds a single connection meets the p95 budget, it may use one; the API does
  not change.
- Blob writes are content-addressed: write to a temp file, fsync, rename into
  place, then commit the DB row. If the DB commit fails, the orphan blob is
  GC'd later. Never reference a not-yet-written blob.
- `state_rev` increments in the same transaction as the mutation; events are
  emitted after commit.
- Long migrations batch commits (500 clips) to bound WAL growth; a cancel
  leaves a consistent prefix and `migration_state` phase.
- `busy_timeout` handles the brief writer/reader overlap; no busy-loop retries
  in UI threads.

---

## 11. Performance notes

- Search: FTS5 trigram for ≥3 characters; 1–2 character queries use `LIKE`
  with a `LIMIT` and the recent/group indexes. Do not run an unbounded `LIKE`
  over `text_content`; cap and document.
- Pagination: keyset on `(is_pinned, pin_order, sort_order, id)`, not `OFFSET`
  for large sets.
- Indexes cover the default sorts and group filters. Avoid sorting 10k rows in
  memory; let SQLite use the index.
- `ANALYZE` after migration/import and `PRAGMA optimize` on close.
- Thumbnail generation is lazy and off the UI thread; the popup requests a
  thumbnail by clip id and gets a blob reference.
- Blob GC and retention run in a low-priority background task, never during a
  hotkey show.
- FTS rebuild after migration is a single transaction or a controlled batch;
  measure and report.

---

## 12. Security and privacy

- The data root and DB inherit a user-only ACL. No `Everyone`/`Users` write.
- No encryption in v1; the threat model is a local, same-user attacker. This is
  stated in Settings → About/Privacy and in `SECURITY.md`.
- Clip content is never logged. Migration reports counts and hashes, not text.
- The DB is not a network service. No SQL is accepted from IPC; only fixed
  operations with bound parameters.
- HTML/RTF/files are stored as data, never executed. Preview renders escaped
  text or a decoded image, never a live webview.
- The legacy DB copy in the temp directory is deleted after migration (or on
  the next startup if the process crashed); it contains user data and must not
  be left behind.
- Export files are user-created and may contain clip content; the UI warns that
  they are unencrypted.

---

## 13. Evidence

Legacy facts were read from:

- `src/DatabaseUtilities.cpp` — `CreateDB`, `ValidDB`, index/trigger creation,
  `RemoveOldEntries`, backup/compact/repair.
- `src/Clip.cpp` — `AddToDataTable`, `LoadFormats`, CRC generation, format
  mapping, description extraction.
- `src/ClipIds.cpp` — export DB schema and id deletion.
- `src/DittoCopyBuffer.cpp` — `CopyBuffers` 0-based slot behavior.
- `src/Options.cpp` — `Ditto.Settings`, registry fallback, `DBPath3`, defaults,
  copy-buffer keys, language/theme keys.
- `src/MultiLanguage.cpp` — language file loading (not migrated).
- `src/Misc.cpp` — `GetFormatName`/`GetFormatID` stable format names.
- `docs/ui/CURRENT-STATE.md` — legacy UI/data behavior audit.
- `CLAUDE.md` — data layer and lifecycle summary.
- Coordinator rewrite prompt appendix A — required mapping targets; deviations
  are called out in §3.3, §3.2, and `DECISIONS.md`.

Related: `ARCHITECTURE.md`, `DESIGN-SYSTEM.md`, `FEATURE-MATRIX.md`,
`PERFORMANCE.md`, `SECURITY.md`, `RISK-REGISTER.md`, `OWNERSHIP.md`,
`DECISIONS.md`.

---

## 14. T2 implementation deltas

T2 implemented the v1 schema with three additive fields needed by the clipboard
and popup contracts:

| Area | Added | Reason |
|---|---|---|
| `clips.description` | `TEXT NOT NULL DEFAULT ''` | The task requires the FTS search fields `preview/text/quick_paste_text/description`; the T0 DDL listed only the first three. |
| `clip_formats.mime` | `TEXT` | Persist the MIME type alongside `data_path` for PNG/DIB/HTML/RTF/file/custom formats. |
| `clips_fts` | `description` column plus trigger values | Keep the new description field searchable through the same external-content FTS5 table. |

`ClipPage`/the `GetClips` and `Search` response now include
`total_is_estimate`. When another page exists, or when a cursor is used, the
core avoids a second full FTS/LIKE scan for `COUNT(*)` and reports the returned
page count as an estimated total. This follows `PERFORMANCE.md` §5.3, which
requires a cached/deferred count when an exact count doubles query time.
