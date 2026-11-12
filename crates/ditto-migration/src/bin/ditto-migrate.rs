use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::PathBuf,
    process::ExitCode,
};

use ditto_migration::{
    ExportOptions, ImportOptions, MigrationOptions, MigrationSource, backup, compact,
    create_export, dry_run, import_package, migrate, restore, verify_package,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ditto-migrate: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print_help();
        return Ok(());
    };
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(());
    }
    let parsed = ParsedArgs::parse(arguments.collect())?;
    match command.as_str() {
        "migrate" => command_migrate(&parsed),
        "export" => command_export(&parsed),
        "import" => command_import(&parsed),
        "backup" => command_backup(&parsed),
        "restore" => command_restore(&parsed),
        "verify" => command_verify(&parsed),
        "compact" => command_compact(&parsed),
        _ => Err(format!("unknown command {command:?}").into()),
    }
}

fn command_migrate(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = args.required_path("source")?;
    let destination = args.required_path("dest")?;
    let settings = args.optional_path("settings");
    let source = MigrationSource {
        database: source_path,
        settings,
    };
    let dry = args.flags.contains("dry-run");
    let apply = args.flags.contains("apply");
    if dry == apply {
        return Err("migrate requires exactly one of --dry-run or --apply".into());
    }
    if dry {
        let report = dry_run(&source)?;
        emit(
            args.json,
            &report,
            format!(
                "dry-run: {} clips, {} groups, {} formats, {} warnings; source {}",
                report.source_counts.clips,
                report.source_counts.groups,
                report.source_counts.formats,
                report.warnings.len(),
                report.source_sha256
            ),
        )?;
        return Ok(());
    }
    let options = MigrationOptions {
        batch_size: args.optional_u32("batch-size")?.unwrap_or(500),
        backup_dir: args.optional_path("backup-dir"),
        keep_backup: !args.flags.contains("no-keep-backup"),
        replace_existing: args.flags.contains("replace-existing"),
    };
    let outcome = migrate(&source, &destination, &options)?;
    emit(
        args.json,
        &outcome.report,
        format!(
            "migrated: {} clips, {} groups, {} formats, {} warnings -> {}",
            outcome.report.imported_counts.clips,
            outcome.report.imported_counts.groups,
            outcome.report.imported_counts.formats,
            outcome.report.warnings.len(),
            outcome.destination.display()
        ),
    )
}

fn command_export(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source = args.required_path("source")?;
    let destination = args.required_path("dest")?;
    let report = create_export(
        &source,
        &destination,
        &ExportOptions {
            zip: args.flags.contains("zip"),
            replace_existing: args.flags.contains("replace-existing"),
        },
    )?;
    emit(
        args.json,
        &report,
        format!(
            "exported: {} clips, {} files verified -> {}",
            report.counts.clips, report.files_verified, report.package_path
        ),
    )
}

fn command_import(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let package = args.required_path("source")?;
    let destination = args.required_path("dest")?;
    let report = import_package(&package, &destination, &import_options(args)?)?;
    emit(
        args.json,
        &report,
        format!(
            "imported: {} clips, {} groups -> {}",
            report.counts.clips, report.counts.groups, report.package_path
        ),
    )
}

fn command_backup(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source = args.required_path("source")?;
    let destination = args.required_path("dest")?;
    let report = backup(&source, &destination, args.flags.contains("zip"))?;
    emit(
        args.json,
        &report,
        format!(
            "backup: {} clips, {} files verified -> {}",
            report.counts.clips, report.files_verified, report.package_path
        ),
    )
}

fn command_restore(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let package = args.required_path("source")?;
    let destination = args.required_path("dest")?;
    let report = restore(&package, &destination, &import_options(args)?)?;
    emit(
        args.json,
        &report,
        format!(
            "restored: {} clips, {} groups -> {}",
            report.counts.clips, report.counts.groups, report.package_path
        ),
    )
}

fn command_verify(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let package = args.required_path("source")?;
    let report = verify_package(&package)?;
    emit(
        args.json,
        &report,
        format!(
            "verified: {} clips, {} groups, {} files -> {}",
            report.counts.clips, report.counts.groups, report.files_verified, report.package_path
        ),
    )
}

fn command_compact(args: &ParsedArgs) -> Result<(), Box<dyn std::error::Error>> {
    let destination = args.required_path("dest")?;
    let report = compact(&destination)?;
    emit(
        args.json,
        &report,
        format!(
            "compacted: {} clips at {}",
            report.counts.clips, report.package_path
        ),
    )
}

fn import_options(args: &ParsedArgs) -> Result<ImportOptions, Box<dyn std::error::Error>> {
    Ok(ImportOptions {
        replace_existing: args.flags.contains("replace-existing"),
        backup_dir: args.optional_path("backup-dir"),
    })
}

fn emit<T: serde::Serialize>(
    json: bool,
    value: &T,
    human: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{human}");
    }
    Ok(())
}

fn print_help() {
    println!(
        "ditto-migrate\n\
         \n\
         migrate --source <old.db> --dest <data-root> --dry-run|--apply\n\
           [--settings <Ditto.Settings>] [--batch-size <n>]\n\
           [--backup-dir <dir>] [--keep-backup|--no-keep-backup]\n\
           [--replace-existing] [--json]\n\
         export --source <data-root> --dest <package> [--zip] [--replace-existing] [--json]\n\
         import --source <package> --dest <data-root> [--replace-existing] [--backup-dir <dir>] [--json]\n\
         backup --source <data-root> --dest <backup-dir> [--zip] [--json]\n\
         restore --source <backup> --dest <data-root> [--replace-existing] [--backup-dir <dir>] [--json]\n\
         verify --source <package> [--json]\n\
         compact --dest <data-root> [--json]"
    );
}

struct ParsedArgs {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
    json: bool,
}

impl ParsedArgs {
    fn parse(arguments: Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let value_options = ["source", "dest", "settings", "batch-size", "backup-dir"];
        let flag_options = [
            "dry-run",
            "apply",
            "zip",
            "replace-existing",
            "json",
            "keep-backup",
            "no-keep-backup",
        ];
        let mut values = BTreeMap::new();
        let mut flags = BTreeSet::new();
        let mut index = 0_usize;
        while index < arguments.len() {
            let argument = &arguments[index];
            let Some(name) = argument.strip_prefix("--") else {
                return Err(format!("unexpected argument {argument:?}").into());
            };
            if value_options.contains(&name) {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| format!("--{name} requires a value"))?;
                if values.insert(name.to_string(), value.clone()).is_some() {
                    return Err(format!("--{name} was supplied more than once").into());
                }
                index += 2;
            } else if flag_options.contains(&name) {
                if !flags.insert(name.to_string()) {
                    return Err(format!("--{name} was supplied more than once").into());
                }
                index += 1;
            } else {
                return Err(format!("unknown option --{name}").into());
            }
        }
        let json = flags.contains("json");
        Ok(Self {
            values,
            flags,
            json,
        })
    }

    fn required_path(&self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.values
            .get(name)
            .map(PathBuf::from)
            .ok_or_else(|| format!("--{name} is required").into())
    }

    fn optional_path(&self, name: &str) -> Option<PathBuf> {
        self.values.get(name).map(PathBuf::from)
    }

    fn optional_u32(&self, name: &str) -> Result<Option<u32>, Box<dyn std::error::Error>> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|error| format!("invalid --{name}: {error}").into())
            })
            .transpose()
    }
}
