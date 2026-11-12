use std::{
    fs,
    path::{Path, PathBuf},
};

use ditto_popup_app::{
    mock::MockProvider,
    provider::{ClipProvider, GroupFilter},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/rewrite/after/perf/search-10k-mock.csv"));
    let mut provider = MockProvider::default();
    let rows = provider.search_rows(
        &[
            "",
            "meeting",
            "客户项目",
            "日本語",
            "Synthetic clipboard",
            "CF_UNICODETEXT",
        ],
        30,
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut csv = String::from("query,iteration,elapsed_us,rows\n");
    for row in &rows {
        csv.push_str(&format!(
            "\"{}\",{},{},{}\n",
            row.query.replace('"', "\"\""),
            row.iteration,
            row.elapsed_us,
            row.rows
        ));
    }
    write_summary(&output, &rows)?;
    fs::write(&output, csv)?;
    let all = provider.load("", GroupFilter::All, usize::MAX, None)?;
    println!("wrote {} rows; fixture clips={}", rows.len(), all.total);
    Ok(())
}

fn write_summary(
    csv_path: &Path,
    rows: &[ditto_popup_app::mock::BenchmarkRow],
) -> Result<(), Box<dyn std::error::Error>> {
    let path = csv_path.with_file_name("search-10k-mock-summary.csv");
    let mut output = String::from("query,samples,p50_us,p95_us,max_us,rows\n");
    let mut queries = Vec::<&str>::new();
    for row in rows {
        if !queries.contains(&row.query.as_str()) {
            queries.push(&row.query);
        }
    }
    for query in queries {
        let mut values: Vec<_> = rows
            .iter()
            .filter(|row| row.query == query)
            .map(|row| row.elapsed_us)
            .collect();
        values.sort_unstable();
        let result_rows = rows
            .iter()
            .find(|row| row.query == query)
            .map_or(0, |row| row.rows);
        output.push_str(&format!(
            "\"{}\",{},{},{},{},{}\n",
            query.replace('"', "\"\""),
            values.len(),
            percentile(&values, 0.50),
            percentile(&values, 0.95),
            values.last().copied().unwrap_or_default(),
            result_rows
        ));
    }
    fs::write(path, output)?;
    Ok(())
}

fn percentile(values: &[u128], percentile: f64) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}
