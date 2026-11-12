# Synthetic legacy Ditto fixture

`schema.sql` is the legacy `DatabaseUtilities.cpp::CreateDB` schema.
`Ditto.Settings.fixture` contains mappable, unknown, and secret-looking keys.
The `.fixture` suffix keeps it visible to source control because the repository
ignores `*.Settings` runtime files.

The integration test in `crates/ditto-migration/tests/legacy_fixture.rs`
populates this schema with synthetic rows covering:

- Unicode text, HTML, RTF, PNG, DIB, HDROP, and custom formats
- one multi-format clip
- a two-level group tree
- sticky and non-sticky clips
- copy-buffer slots 0..4 mapped to 1..5
- shortcuts, CRC, last-paste timestamps, and quick-paste text
- empty and malformed data rows

No real user database, `%APPDATA%\Ditto`, or real clipboard content is used.
