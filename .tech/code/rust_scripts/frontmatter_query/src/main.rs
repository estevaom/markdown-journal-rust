use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::Parser;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to search for journal files
    #[arg(short, long, default_value = "journal")]
    path: PathBuf,

    /// Fields to extract from frontmatter
    #[arg(
        short,
        long,
        num_args = 1..,
        default_values_t = vec!["mood".to_string(), "anxiety".to_string(), "weight_kg".to_string()]
    )]
    fields: Vec<String>,

    /// Start date filter (YYYY-MM-DD)
    #[arg(short = 's', long)]
    start_date: Option<String>,

    /// End date filter (YYYY-MM-DD)
    #[arg(short = 'e', long)]
    end_date: Option<String>,

    /// Convenience filter: only include entries from the last N days (including today)
    /// (ignored if --start-date is provided)
    #[arg(long)]
    last_days: Option<i64>,

    /// Calculate statistics for numeric fields
    #[arg(long)]
    stats: bool,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value = "json")]
    format: OutputFormat,

    /// Include file paths in output
    #[arg(long)]
    include_files: bool,

    /// Only include entries that contain ALL of these tags (repeatable)
    #[arg(long, num_args = 1..)]
    has_tag: Vec<String>,

    /// Only include entries that contain ALL of these triggers (repeatable)
    #[arg(long, num_args = 1..)]
    has_trigger: Vec<String>,

    /// List all tags (with counts) instead of returning per-day results
    #[arg(long)]
    list_tags: bool,

    /// List all triggers (with counts) instead of returning per-day results
    #[arg(long)]
    list_triggers: bool,

    /// Lint tags and triggers for standards (format, length, word-count)
    #[arg(long)]
    lint: bool,

    /// Lint tags for standards (format, length, word-count)
    #[arg(long)]
    lint_tags: bool,

    /// Lint triggers for standards (format, length, word-count)
    #[arg(long)]
    lint_triggers: bool,

    /// Max words (underscore-separated parts) allowed in tags. Default: 2.
    #[arg(long, default_value_t = 2)]
    tag_max_words: usize,

    /// Max length allowed for a tag/trigger value. Default: 28.
    #[arg(long, default_value_t = 28)]
    term_max_length: usize,

    /// Find the last rest day (where rest_day=true)
    #[arg(long)]
    last_rest_day: bool,

    /// Show current training streak (consecutive days where rest_day=false or missing)
    #[arg(long)]
    streak: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
    Table,
}

#[derive(Debug)]
struct JournalEntry {
    file_path: PathBuf,
    date: NaiveDate,
    frontmatter: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Serialize)]
struct QueryResult {
    date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(flatten)]
    fields: HashMap<String, Option<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct TermCount {
    value: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct LintIssue {
    date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    field: String,
    kind: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
struct FieldStats {
    count: usize,
    min: f64,
    max: f64,
    avg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_count: Option<usize>,
}

fn extract_frontmatter(content: &str) -> Result<HashMap<String, serde_yaml::Value>> {
    let re = Regex::new(r"(?s)^---\n(.*?)\n---")?;

    if let Some(captures) = re.captures(content) {
        let yaml_content = captures.get(1).unwrap().as_str();
        let frontmatter: HashMap<String, serde_yaml::Value> =
            serde_yaml::from_str(yaml_content).context("Failed to parse YAML frontmatter")?;
        Ok(frontmatter)
    } else {
        Err(anyhow::anyhow!("No frontmatter found"))
    }
}

fn parse_date_from_frontmatter(frontmatter: &HashMap<String, serde_yaml::Value>) -> Result<NaiveDate> {
    let date_value = frontmatter
        .get("date")
        .ok_or_else(|| anyhow::anyhow!("No date field in frontmatter"))?;

    match date_value {
        serde_yaml::Value::String(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .context("Failed to parse date string"),
        _ => Err(anyhow::anyhow!("Date field is not a string")),
    }
}

fn find_journal_files(
    base_dir: &Path,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<JournalEntry>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(base_dir) {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let frontmatter = match extract_frontmatter(&content) {
                Ok(fm) => fm,
                Err(_) => continue,
            };

            let date = match parse_date_from_frontmatter(&frontmatter) {
                Ok(date) => date,
                Err(_) => continue,
            };

            if let Some(start) = start_date {
                if date < start {
                    continue;
                }
            }
            if let Some(end) = end_date {
                if date > end {
                    continue;
                }
            }

            entries.push(JournalEntry {
                file_path: path.to_path_buf(),
                date,
                frontmatter,
            });
        }
    }

    entries.sort_by_key(|e| e.date);

    Ok(entries)
}

fn extract_string_list(frontmatter: &HashMap<String, serde_yaml::Value>, key: &str) -> Vec<String> {
    match frontmatter.get(key) {
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| match v {
                serde_yaml::Value::String(s) => Some(s.trim().to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Some(serde_yaml::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed.to_string()]
            }
        }
        _ => Vec::new(),
    }
}

fn entry_contains_all(entry: &JournalEntry, key: &str, required: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }
    let values = extract_string_list(&entry.frontmatter, key);
    if values.is_empty() {
        return false;
    }
    required.iter().all(|req| values.iter().any(|v| v == req))
}

fn filter_entries(entries: Vec<JournalEntry>, has_tags: &[String], has_triggers: &[String]) -> Vec<JournalEntry> {
    entries
        .into_iter()
        .filter(|e| entry_contains_all(e, "tags", has_tags) && entry_contains_all(e, "triggers", has_triggers))
        .collect()
}

fn yaml_to_json_value(yaml_val: &serde_yaml::Value) -> serde_json::Value {
    match yaml_val {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or(0.into()))
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => {
            let cleaned = if let Some(pos) = s.find('#') {
                s[..pos].trim().to_string()
            } else {
                s.clone()
            };
            serde_json::Value::String(cleaned)
        }
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_to_json_value).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .filter_map(|(k, v)| k.as_str().map(|key| (key.to_string(), yaml_to_json_value(v))))
                .collect();
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

fn query_fields(entries: &[JournalEntry], fields: &[String], include_files: bool) -> Vec<QueryResult> {
    entries
        .iter()
        .map(|entry| {
            let mut field_values = HashMap::new();

            for field in fields {
                let value = entry
                    .frontmatter
                    .get(field)
                    .map(|v| yaml_to_json_value(v))
                    .filter(|v| !matches!(v, serde_json::Value::Null));

                field_values.insert(field.clone(), value);
            }

            QueryResult {
                date: entry.date.format("%Y-%m-%d").to_string(),
                file: if include_files {
                    Some(entry.file_path.display().to_string())
                } else {
                    None
                },
                fields: field_values,
            }
        })
        .collect()
}

fn parse_numeric_value(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            if let Some(dash_pos) = s.find('-') {
                let (start, end) = s.split_at(dash_pos);
                let end = &end[1..];

                if let (Ok(start_val), Ok(end_val)) = (start.trim().parse::<f64>(), end.trim().parse::<f64>()) {
                    return Some((start_val + end_val) / 2.0);
                }
            }

            s.parse::<f64>().ok()
        }
        _ => None,
    }
}

fn calculate_stats(results: &[QueryResult], field: &str) -> Option<FieldStats> {
    let mut values = Vec::new();
    let mut skipped = 0;

    for result in results {
        if let Some(Some(value)) = result.fields.get(field) {
            if let Some(num) = parse_numeric_value(value) {
                values.push(num);
            } else {
                skipped += 1;
            }
        }
    }

    if values.is_empty() {
        return None;
    }

    let count = values.len();
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = values.iter().sum();
    let avg = sum / count as f64;

    Some(FieldStats {
        count,
        min,
        max,
        avg,
        skipped_count: if skipped > 0 { Some(skipped) } else { None },
    })
}

fn to_csv(results: &[QueryResult], fields: &[String], include_files: bool) -> Result<String> {
    let mut rows = Vec::new();

    let mut header = vec!["date".to_string()];
    if include_files {
        header.push("file".to_string());
    }
    header.extend(fields.iter().cloned());
    rows.push(header.join(","));

    for result in results {
        let mut row = vec![result.date.clone()];
        if include_files {
            row.push(result.file.clone().unwrap_or_default());
        }
        for field in fields {
            let val = result
                .fields
                .get(field)
                .and_then(|v| v.as_ref())
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                })
                .unwrap_or_default();
            row.push(val);
        }
        rows.push(row.join(","));
    }

    Ok(rows.join("\n"))
}

fn to_table(results: &[QueryResult], fields: &[String], include_files: bool) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();

    let mut header = vec!["date".to_string()];
    if include_files {
        header.push("file".to_string());
    }
    header.extend(fields.iter().cloned());
    rows.push(header);

    for result in results {
        let mut row = vec![result.date.clone()];
        if include_files {
            row.push(result.file.clone().unwrap_or_default());
        }
        for field in fields {
            let val = result
                .fields
                .get(field)
                .and_then(|v| v.as_ref())
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                })
                .unwrap_or_else(|| "-".to_string());
            row.push(val);
        }
        rows.push(row);
    }

    // Compute column widths
    let mut widths: Vec<usize> = vec![0; rows[0].len()];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        if row_idx == 1 {
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    out.push_str(" | ");
                }
                out.push_str(&"-".repeat(*w));
            }
            out.push('\n');
        }

        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            out.push_str(&format!("{:width$}", cell, width = widths[i]));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

fn count_terms(entries: &[JournalEntry], key: &str) -> Vec<TermCount> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let values = extract_string_list(&entry.frontmatter, key);
        for value in values {
            *counts.entry(value).or_insert(0) += 1;
        }
    }

    let mut term_counts: Vec<TermCount> = counts
        .into_iter()
        .map(|(value, count)| TermCount { value, count })
        .collect();

    term_counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    term_counts
}

fn is_snake_case(value: &str) -> bool {
    let re = Regex::new(r"^[a-z0-9]+(_[a-z0-9]+)*$").expect("snake_case regex");
    re.is_match(value)
}

fn count_words_by_underscore(value: &str) -> usize {
    value.split('_').filter(|s| !s.is_empty()).count()
}

fn lint_terms(
    entry: &JournalEntry,
    key: &str,
    term_max_length: usize,
    tag_max_words: usize,
) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let values = extract_string_list(&entry.frontmatter, key);

    for value in values {
        if value.len() > term_max_length {
            issues.push(LintIssue {
                date: entry.date.format("%Y-%m-%d").to_string(),
                file: Some(entry.file_path.display().to_string()),
                field: key.to_string(),
                kind: "too_long".to_string(),
                value: value.clone(),
                suggestion: None,
            });
        }

        if !is_snake_case(&value) {
            issues.push(LintIssue {
                date: entry.date.format("%Y-%m-%d").to_string(),
                file: Some(entry.file_path.display().to_string()),
                field: key.to_string(),
                kind: "not_snake_case".to_string(),
                value: value.clone(),
                suggestion: Some(value.to_lowercase().replace(' ', "_").replace('-', "_")),
            });
        }

        if key == "tags" {
            let words = count_words_by_underscore(&value);
            if words > tag_max_words {
                issues.push(LintIssue {
                    date: entry.date.format("%Y-%m-%d").to_string(),
                    file: Some(entry.file_path.display().to_string()),
                    field: key.to_string(),
                    kind: "too_many_words".to_string(),
                    value: value.clone(),
                    suggestion: None,
                });
            }
        }

        if key == "triggers" {
            let words = count_words_by_underscore(&value);
            if words != 2 {
                issues.push(LintIssue {
                    date: entry.date.format("%Y-%m-%d").to_string(),
                    file: Some(entry.file_path.display().to_string()),
                    field: key.to_string(),
                    kind: "trigger_word_count".to_string(),
                    value: value.clone(),
                    suggestion: None,
                });
            }
        }
    }

    issues
}

fn find_last_rest_day(entries: &[JournalEntry]) -> Option<&JournalEntry> {
    entries.iter().rev().find(|e| {
        e.frontmatter
            .get("rest_day")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    })
}

fn calculate_streak(entries: &[JournalEntry]) -> (usize, Option<NaiveDate>) {
    let mut streak = 0;
    let mut streak_start: Option<NaiveDate> = None;

    for entry in entries.iter().rev() {
        let rest_day = entry
            .frontmatter
            .get("rest_day")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if rest_day {
            break;
        }

        streak += 1;
        streak_start = Some(entry.date);
    }

    (streak, streak_start)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let today = Local::now().date_naive();

    let start_date = args
        .start_date
        .as_deref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()?;
    let end_date = args
        .end_date
        .as_deref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()?;

    let last_days_active = args.start_date.is_none() && args.last_days.is_some();

    let computed_start = if start_date.is_some() {
        start_date
    } else if let Some(last_days) = args.last_days {
        if last_days <= 0 {
            return Err(anyhow::anyhow!("--last-days must be >= 1"));
        }
        Some(today - chrono::Duration::days(last_days - 1))
    } else {
        None
    };

    let computed_end = if end_date.is_some() {
        end_date
    } else if last_days_active {
        Some(today)
    } else {
        None
    };

    let entries = find_journal_files(&args.path, computed_start, computed_end)?;
    let entries = filter_entries(entries, &args.has_tag, &args.has_trigger);

    if args.last_rest_day {
        if let Some(entry) = find_last_rest_day(&entries) {
            println!("{}", entry.date.format("%Y-%m-%d"));
        } else {
            println!("No rest_day=true entries found");
        }
        return Ok(());
    }

    if args.streak {
        let (streak, start_date) = calculate_streak(&entries);
        if streak > 0 {
            println!("Current training streak: {} days", streak);
            if let Some(start) = start_date {
                println!("Streak started: {}", start.format("%Y-%m-%d"));
            }
        } else {
            println!("No current streak (last entry was a rest day)");
        }
        return Ok(());
    }

    if args.list_tags || args.list_triggers {
        let mut obj = serde_json::Map::new();
        if args.list_tags {
            obj.insert(
                "tags".to_string(),
                serde_json::to_value(count_terms(&entries, "tags"))?,
            );
        }
        if args.list_triggers {
            obj.insert(
                "triggers".to_string(),
                serde_json::to_value(count_terms(&entries, "triggers"))?,
            );
        }

        match args.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&obj)?);
            }
            OutputFormat::Csv | OutputFormat::Table => {
                // Non-JSON formats don't map well to dual lists; fall back to JSON output.
                println!("{}", serde_json::to_string_pretty(&obj)?);
            }
        }

        return Ok(());
    }

    let lint_tags = args.lint || args.lint_tags;
    let lint_triggers = args.lint || args.lint_triggers;
    if lint_tags || lint_triggers {
        let mut issues = Vec::new();

        for entry in &entries {
            if lint_tags {
                issues.extend(lint_terms(
                    entry,
                    "tags",
                    args.term_max_length,
                    args.tag_max_words,
                ));
            }
            if lint_triggers {
                issues.extend(lint_terms(
                    entry,
                    "triggers",
                    args.term_max_length,
                    args.tag_max_words,
                ));
            }
        }

        match args.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&issues)?);
            }
            OutputFormat::Csv => {
                let json = serde_json::to_value(&issues)?;
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Table => {
                // Table output for lint is intentionally JSON-ish to avoid formatting a complex schema.
                let json = serde_json::to_value(&issues)?;
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }

        return Ok(());
    }

    let results = query_fields(&entries, &args.fields, args.include_files);

    if args.stats {
        let mut stats = serde_json::Map::new();
        for field in &args.fields {
            if let Some(s) = calculate_stats(&results, field) {
                stats.insert(field.clone(), serde_json::to_value(s)?);
            }
        }

        match args.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&json!({ "results": results, "stats": stats }))?);
            }
            OutputFormat::Csv => {
                println!("{}", to_csv(&results, &args.fields, args.include_files)?);
            }
            OutputFormat::Table => {
                println!("{}", to_table(&results, &args.fields, args.include_files));
                if !stats.is_empty() {
                    println!();
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                }
            }
        }
    } else {
        match args.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&results)?);
            }
            OutputFormat::Csv => {
                println!("{}", to_csv(&results, &args.fields, args.include_files)?);
            }
            OutputFormat::Table => {
                println!("{}", to_table(&results, &args.fields, args.include_files));
            }
        }
    }

    Ok(())
}
