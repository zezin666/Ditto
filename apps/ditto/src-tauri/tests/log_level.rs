//! Proves the runtime log level is reloadable: a debug record that is dropped
//! at `info` is emitted after the level is raised to `debug` in the same
//! process. The installed logger uses the same predicate that
//! `tauri-plugin-log` receives through `Builder::filter`.

use std::sync::Mutex;

use ditto_tauri_lib::logging;
use log::{LevelFilter, Log, Metadata, Record};

static RECORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct CapturingLogger;

impl Log for CapturingLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        logging::enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            RECORDS.lock().expect("records mutex").push(format!(
                "{}:{}",
                record.level(),
                record.args()
            ));
        }
    }

    fn flush(&self) {}
}

static LOGGER: CapturingLogger = CapturingLogger;

fn records_since(mark: usize) -> Vec<String> {
    RECORDS.lock().expect("records mutex")[mark..].to_vec()
}

#[test]
fn changing_the_level_at_runtime_enables_debug_records() {
    log::set_logger(&LOGGER).expect("the test binary owns the global logger");
    logging::set_runtime_level(LevelFilter::Info);
    let mark = RECORDS.lock().expect("records mutex").len();

    log::debug!("dropped-before-raise");
    assert!(
        records_since(mark).is_empty(),
        "debug must be filtered at info"
    );

    logging::set_runtime_level(LevelFilter::Debug);
    log::debug!("emitted-after-raise");
    let records = records_since(mark);
    assert!(
        records
            .iter()
            .any(|record| record.contains("DEBUG:emitted-after-raise")),
        "debug must be emitted after raising the level: {records:?}"
    );
    assert!(
        !records
            .iter()
            .any(|record| record.contains("dropped-before-raise")),
        "records filtered before the change must stay dropped"
    );

    logging::set_runtime_level(LevelFilter::Info);
    let mark = RECORDS.lock().expect("records mutex").len();
    log::debug!("dropped-again");
    assert!(records_since(mark).is_empty(), "debug must filter again");
    log::info!("visible-info");
    assert!(
        records_since(mark)
            .iter()
            .any(|record| record.contains("INFO:visible-info"))
    );
}
