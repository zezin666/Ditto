# ditto-migrate

The CLI binary is built as the `ditto-migrate` target of the workspace member
`crates/ditto-migration`, so `cargo build --workspace --locked` includes it
without changing the root workspace membership.

Run from the repository root:

```powershell
pwsh tools/ditto-migrate/run.ps1 migrate --source .\fixture\Ditto.db --dest .\data-v2 --dry-run
pwsh tools/ditto-migrate/run.ps1 migrate --source .\fixture\Ditto.db --dest .\data-v2 --apply
pwsh tools/ditto-migrate/run.ps1 export --source .\data-v2 --dest .\export.zip --zip
pwsh tools/ditto-migrate/run.ps1 verify --source .\export.zip
```

Commands: `migrate`, `export`, `import`, `backup`, `restore`, `verify`, and
`compact`. Every command exits non-zero on validation, checksum, or I/O failure.
