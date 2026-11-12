# T6 — Legacy migration, import/export, backup, and restore

## STATUS

Implementation complete and **uncommitted**. Git commands were not run because
the repository `AGENTS.md` requires all git operations to be performed by the
user.

The synthetic fixture pass covers legacy DB mapping, read-only source
integrity, dry-run behavior, idempotent rerun, rollback, settings mapping,
manifest validation, export/import round-trip, backup/restore, compact, and a
10k synthetic throughput/memory run.

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-migration`

## BRANCH

Dispatch context: branch based on `taohy/rewrite-core` @ `7069fcc`, with
`rewrite/tauri` @ `847f966`. The branch and HEAD were not re-read locally
because git operations are prohibited.

## HEAD

Not re-read. Use the coordinator's integration commit for the final SHA.

## FILES

Added or modified:

- `crates/ditto-storage/src/lib.rs`
- `crates/ditto-storage/src/store.rs`
- `crates/ditto-storage/src/legacy.rs`
- `crates/ditto-migration/Cargo.toml`
- `crates/ditto-migration/src/lib.rs`
- `crates/ditto-migration/src/error.rs`
- `crates/ditto-migration/src/source.rs`
- `crates/ditto-migration/src/format.rs`
- `crates/ditto-migration/src/ini.rs`
- `crates/ditto-migration/src/migrate.rs`
- `crates/ditto-migration/src/package.rs`
- `crates/ditto-migration/src/types.rs`
- `crates/ditto-migration/src/bin/ditto-migrate.rs`
- `crates/ditto-migration/tests/legacy_fixture.rs`
- `crates/ditto-migration/tests/bench_10k_migration.rs`
- `tools/ditto-migrate/run.ps1`
- `tools/ditto-migrate/README.md`
- `tests/fixtures/legacy/schema.sql`
- `tests/fixtures/legacy/Ditto.Settings.fixture`
- `tests/fixtures/legacy/README.md`
- `docs/rewrite/MIGRATION-REPORT.md`
- `docs/rewrite/DATA-MODEL.md` (T6 implementation delta only)
- `Cargo.lock` (generated for the new `zip` dependency)

No legacy C++, `LICENSE`, Tauri app, popup app, or existing T2 API/schema
semantics were modified.

## STORAGE API

The coordinator approved a narrow, additive exception for full-fidelity legacy
insertion in `crates/ditto-storage/src/legacy.rs`.

Public additions:

- `Storage::insert_legacy_many(&[LegacyClipRecord])`
- `Storage::insert_legacy_batch(&LegacyBatch)`
- `LegacyBatch { groups, clips, copy_buffers }`
- `LegacyInsertResult` with legacy-to-new group/clip ID maps and insert/skip
  counts

The legacy records carry:

- clip: `legacy_id`, `legacy_crc`, `legacy_meta`, shortcut/global/move-to-group
  fields, `last_pasted_at_ms`, pin state/order, group legacy ID
- group: `legacy_id`, parent legacy ID, ordering, timestamps
- format: `legacy_data_id`
- copy buffer: normalized slot 1..5, `legacy_id`, mapped clip legacy ID

Behavior:

- one row is inserted for every legacy `Main` clip, regardless of hash
  collisions; T2 capture dedupe is bypassed
- inserts run in one SQLite transaction per call
- existing FTS triggers, blob writer, thumbnail writer, and format storage are
  reused
- existing `legacy_id` rows are skipped and returned in the mapping, making
  reruns idempotent
- copy-buffer references resolve clips from earlier batches in the same DB
- rollback removes all rows if any group, clip, format, or buffer write fails

## LEGACY MAPPING

| Legacy | New |
|---|---|
| `Main.lID` | `clips.legacy_id` |
| `Main.lDate` | `clips.created_at_ms` (`seconds * 1000`) |
| `Main.mText` | `clips.preview_text` / `clips.description`; fallback text content |
| text formats | `clips.text_content` and raw `clip_formats` |
| `Main.lShortCut` | `clips.shortcut` |
| `Main.globalShortCut` | `clips.global_shortcut` |
| `Main.lDontAutoDelete` | `clips.dont_auto_delete` |
| `Main.CRC` | `clips.legacy_crc`; SHA-256 hashes are recomputed |
| `Main.bIsGroup=1` | `groups`, with `groups.legacy_id` |
| `Main.lParentID` | `groups.parent_id` or `clips.group_id`, mapped by legacy ID |
| `Main.QuickPasteText` | `clips.quick_paste_text` |
| `Main.clipOrder` | `clips.sort_order` |
| `Main.clipGroupOrder` | `clips.group_sort_order` |
| sticky columns | `clips.is_pinned` / `pin_order`; sentinel, 0, and NULL are unpinned |
| `Main.lastPasteDate` | `clips.last_pasted_at_ms` (`seconds * 1000`) |
| `Main.MoveToGroupShortCut` | `clips.move_to_group_shortcut` |
| `Main.GlobalMoveToGroupShortCut` | `clips.global_move_to_group_shortcut` |
| `Data.lID` | `clip_formats.legacy_data_id`; derived PNG uses NULL |
| `Data.lParentID` | `clip_formats.clip_id` through the legacy mapping |
| `Data.strClipBoardFormat` | exact `clip_formats.format_name` |
| `Data.ooData` | inline data or content-addressed blob |
| `Types.TypeText` | `clipboard.supported_types` JSON array |
| `CopyBuffers.lCopyBuffer` 0..4 | `copy_buffers.slot` 1..5 |
| `CopyBuffers.lClipID` | mapped `copy_buffers.clip_id`; `-1` is empty |

Duplicate copy-buffer rows for one slot keep the row with the greatest legacy
`lID` and produce a report warning. Missing or cyclic group parents are attached
to root with warnings. Group depth is bounded at 64.

## FORMAT HANDLING

The migration reuses the T2 clipboard decoders and hash implementation:

- `CF_UNICODETEXT`: UTF-16LE decode; raw bytes preserved
- `CF_TEXT` / `CF_OEMTEXT`: ANSI decode; raw bytes preserved
- `HTML Format`: fragment/preview extraction; raw bytes preserved
- `Rich Text Format`: safe plain-text preview; raw bytes preserved
- `CF_DIB` / `CF_DIBV5`: raw DIB preserved and converted to PNG when valid
- `PNG`: raw preserved and thumbnail generated
- `CF_HDROP`: paths decoded with bounds; raw bytes preserved
- custom formats: raw bytes preserved without interpretation

Empty format rows are skipped with warnings. A clip whose formats are all
unreadable is still inserted as a metadata-only custom clip so legacy clip
counts are preserved. Malformed DIB data remains a raw custom clip with a
warning.

## SETTINGS

`Ditto.Settings` supports UTF-8 with/without BOM, UTF-16LE BOM, and UTF-16BE
BOM. Comments and blank lines are ignored. Only the `[Ditto]` section is read.

Mapped keys use `ditto_model::legacy_setting_to_new` and T2 conversion helpers.
Additional handling covers:

- `QuickPasteSize` -> popup width/height, clamped
- `LinesPerRow` -> 1..5
- `PlaySoundOnCopy` / `CopyBufferPlaySound_*`
- copy-buffer hotkeys 0..4 -> slots 1..5
- types -> `clipboard.supported_types`

Unmapped non-secret keys are preserved in `meta.legacy_settings` as JSON.
Password/token/secret-looking values are not copied; the report records only
that the key was redacted.

## CLI

The binary target is `ditto-migrate` in the existing workspace member
`crates/ditto-migration`, so the root workspace membership does not change.

```powershell
pwsh tools/ditto-migrate/run.ps1 migrate --source <old.db> --dest <data-root> --dry-run
pwsh tools/ditto-migrate/run.ps1 migrate --source <old.db> --dest <data-root> --apply
pwsh tools/ditto-migrate/run.ps1 export --source <data-root> --dest <package> [--zip]
pwsh tools/ditto-migrate/run.ps1 import --source <package> --dest <data-root>
pwsh tools/ditto-migrate/run.ps1 backup --source <data-root> --dest <backup-dir> [--zip]
pwsh tools/ditto-migrate/run.ps1 restore --source <backup> --dest <data-root>
pwsh tools/ditto-migrate/run.ps1 verify --source <package-or-data-root>
pwsh tools/ditto-migrate/run.ps1 compact --dest <data-root>
```

All commands support `--json`. Any validation, checksum, unsupported-version,
or I/O failure exits non-zero.

`migrate --apply` builds a sibling temporary data root, verifies it, then swaps
it into place. A non-empty destination requires `--replace-existing` and is
snapshotted as a verified backup before replacement. Failure before the swap
leaves the destination untouched. The source DB is never a writable input.

## PACKAGE FORMAT

An export/backup is a directory or ZIP containing:

- `manifest.json` (format version 1)
- `ditto.db` snapshot
- `blobs/` when present
- `thumbs/` when present

The manifest records schema/app versions, platform, creation time, counts,
and SHA-256 plus size for the DB, every blob, and every thumbnail.

Import/restore verifies:

- manifest format version and schema compatibility
- DB/package SHA-256 and sizes
- ZIP entry containment and uncompressed-size bounds
- `quick_check`, schema version, and manifest counts
- every DB-referenced blob SHA-256 and thumbnail existence

Import is snapshot replacement, not merge. Exact duplicate removal/remapping
is therefore not applicable in v1; a non-empty destination is refused unless
the caller explicitly allows replacement, and the current root is backed up
first. This is intentionally explicit to avoid silent history loss.

## BUILD

Commands run successfully during implementation:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo clippy -p ditto-storage -p ditto-migration --all-targets --locked -- -D warnings
```

The full workspace clippy/build also required the Tauri sidecars and frontend
dist that are already part of the repository build procedure:

```text
cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh -NoProfile -File scripts/prepare-sidecars.ps1 -Profile debug
npm ci
npm run build
```

These generated `apps/ditto/ui/node_modules/`, `apps/ditto/ui/dist/`, and
sidecar files, which are ignored build inputs rather than source changes.

## TEST

```text
cargo test -p ditto-migration -- --nocapture
```

Result:

- 10 library tests passed
- 3 legacy fixture integration tests passed
- 1 ignored 10k benchmark (run separately in release)

Fixture coverage includes Unicode/Chinese text, multi-format HTML/RTF/text,
PNG source bytes, valid DIB-to-PNG, HDROP, custom format, group tree, pinned
sticky clips, legacy shortcuts, CRC, last-paste time, copy-buffer conflicts,
empty Data, malformed DIB, settings mapping, unknown settings, redacted
password-like values, and duplicate content hashes.

Release performance command:

```text
cargo test --release -p ditto-migration --test bench_10k_migration -- --ignored --nocapture
```

Measured on the current Windows development machine:

```text
T6_10K_MIGRATE elapsed_ms=1211.897 clips_per_sec=8251.5 db_bytes=9846784 peak_working_set_mib=13.4 clips=10000
```

## EVIDENCE

### Synthetic fixture counts

| Item | Source | Imported |
|---|---:|---:|
| Clips | 8 | 8 |
| Groups | 2 | 2 |
| Non-empty source formats | 9 | 9 |
| Empty Data rows skipped | 1 | 0 |
| Pinned clips | 2 | 2 |
| Normalized copy-buffer slots | 5 | 5 |
| Types | 7 | 7 |

The two clips with identical content/format hashes remain two rows in the new
DB. Group `11` maps to group `10`; copy-buffer slots map 0..4 to 1..5; slot 1
keeps the later conflicting legacy row.

### Source integrity

Focused CLI synthetic run:

```text
SOURCE_SHA_BEFORE=d23e160cdbbed5afb90f4658777bd7f22088a8634ac6b637aea5223d493559a8
SOURCE_SHA_AFTER=d23e160cdbbed5afb90f4658777bd7f22088a8634ac6b637aea5223d493559a8
```

The source was opened with `SQLITE_OPEN_READ_ONLY`; fingerprint includes
SHA-256, size, and modification time. `-wal` and `-shm` sidecars are copied
only for the backup artifact.

### Dry-run and apply

```text
dry-run: 1 clip, 0 groups, 1 format, 0 warnings
apply: 1 clip, 0 groups, 1 format, 0 warnings
COUNTS=clips=1,groups=0,formats=1,pinned=1,buffer1=1
```

Dry-run did not create the destination data root. Apply reported source and
post-migration DB backups with hashes.

### Rollback

The integration test creates an existing valid destination with a sentinel,
forces a group uniqueness failure during the staged migration, and verifies:

- migration returns an error
- the destination sentinel remains byte-for-byte unchanged
- no `.ditto-migrate` staging directory remains

### Round-trip

The integration test exports a ZIP, verifies the manifest and all package
files, imports it into a new root, verifies counts, creates a backup, restores
it, and compacts the restored DB. A mutated manifest DB hash is rejected by a
separate unit test.

## UNVERIFIED

- No real user DB was read. Real legacy schema variations beyond the observed
  columns were not exercised.
- Registry fallback settings were not migrated or tested; this task implements
  the INI path.
- Hot WAL/shm recovery, locked source DBs, network shares, failing disks, and
  interrupted process termination were not exercised.
- Very large single formats, damaged SQLite page structures, and salvage mode
  were not exercised. The fixture covers empty and malformed format rows.
- CF_HTML offsets, RTF edge cases, ANSI code pages, uncommon DIB variants, and
  HDROP malformed-path variants beyond the synthetic cases remain unverified.
- Progress is exposed as a Rust callback; IPC migration-progress messages and
  Data-page cancel UI are outside the allowed T6 file surface and remain for
  integration.
- Import merge is intentionally not implemented; only verified snapshot
  replacement is supported.
- Real-world 10k migration on the QA hardware profile and network/defender
  timing were not measured.

## RISKS

- Duplicate group names under one parent cannot be represented by the T2 unique
  index. Migration fails safely and leaves the destination unchanged rather
  than silently renaming groups.
- A source DB that is actively being written by legacy Ditto can produce a
  moving target. Migration rechecks SHA-256 and fails if the source changed.
- Imports/exports/restores should be run while the new Ditto core is closed;
  concurrent blob GC or writers can invalidate snapshot assumptions.
- Package files are unencrypted and may contain full clipboard history.
- Unsigned binaries may produce Defender/SmartScreen warnings. No security
  settings were changed.

## NEXT

1. Coordinator to inspect and commit the uncommitted worktree.
2. T7 to integrate progress/cancel wiring if required by the Data page.
3. T7/T8 to exercise a user-provided legacy DB copy, hot WAL, locked file,
   large single blob, and real Windows-version variants without retaining the
   copy or reporting content.
