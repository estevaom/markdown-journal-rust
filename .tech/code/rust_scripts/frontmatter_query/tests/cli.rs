use chrono::{Duration, Local, NaiveDate};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_frontmatter-query"))
}

fn write_entry(
    dir: &Path,
    filename: &str,
    date: NaiveDate,
    extra_frontmatter_lines: &[&str],
) -> PathBuf {
    let mut frontmatter = format!("---\ndate: {}\n", date.format("%Y-%m-%d"));

    for line in extra_frontmatter_lines {
        frontmatter.push_str(line);
        frontmatter.push('\n');
    }

    frontmatter.push_str("---\n\nBody\n");

    let path = dir.join(filename);
    fs::write(&path, frontmatter).expect("write test entry");
    path
}

fn parse_json(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("parse json output")
}

#[test]
fn last_days_1_includes_today_only() {
    let tmp = TempDir::new().expect("tempdir");
    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);

    write_entry(
        tmp.path(),
        "today.md",
        today,
        &["weight_kg: 101.0", "tags: [clean_day]"],
    );
    write_entry(
        tmp.path(),
        "yesterday.md",
        yesterday,
        &["weight_kg: 102.0", "tags: [clean_day]"],
    );

    let output = bin()
        .args([
            "--path",
            tmp.path().to_str().unwrap(),
            "--fields",
            "weight_kg",
            "--last-days",
            "1",
            "--format",
            "json",
        ])
        .output()
        .expect("run frontmatter-query");

    assert!(output.status.success(), "expected success: {:?}", output);

    let json = parse_json(&output.stdout);
    let results = json.as_array().expect("json array results");
    assert_eq!(results.len(), 1, "expected exactly 1 day");
    assert_eq!(
        results[0]["date"].as_str().unwrap(),
        today.format("%Y-%m-%d").to_string()
    );
}

#[test]
fn last_days_2_includes_today_and_yesterday() {
    let tmp = TempDir::new().expect("tempdir");
    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);

    write_entry(tmp.path(), "today.md", today, &["weight_kg: 101.0"]);
    write_entry(tmp.path(), "yesterday.md", yesterday, &["weight_kg: 102.0"]);

    let output = bin()
        .args([
            "--path",
            tmp.path().to_str().unwrap(),
            "--fields",
            "weight_kg",
            "--last-days",
            "2",
            "--format",
            "json",
        ])
        .output()
        .expect("run frontmatter-query");

    assert!(output.status.success(), "expected success: {:?}", output);

    let json = parse_json(&output.stdout);
    let results = json.as_array().expect("json array results");
    assert_eq!(results.len(), 2, "expected exactly 2 days");
    assert_eq!(
        results[0]["date"].as_str().unwrap(),
        yesterday.format("%Y-%m-%d").to_string()
    );
    assert_eq!(
        results[1]["date"].as_str().unwrap(),
        today.format("%Y-%m-%d").to_string()
    );
}

#[test]
fn last_days_0_errors() {
    let tmp = TempDir::new().expect("tempdir");

    let output = bin()
        .args([
            "--path",
            tmp.path().to_str().unwrap(),
            "--fields",
            "weight_kg",
            "--last-days",
            "0",
            "--format",
            "json",
        ])
        .output()
        .expect("run frontmatter-query");

    assert!(!output.status.success(), "expected failure");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--last-days must be >= 1"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn has_tag_and_has_trigger_filters_entries() {
    let tmp = TempDir::new().expect("tempdir");
    let base = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();

    write_entry(
        tmp.path(),
        "match.md",
        base,
        &["weight_kg: 1", "tags: [foo, bar]", "triggers: [a_b]"],
    );
    write_entry(
        tmp.path(),
        "missing_tag.md",
        base + Duration::days(1),
        &["weight_kg: 2", "tags: [bar]", "triggers: [a_b]"],
    );
    write_entry(
        tmp.path(),
        "missing_trigger.md",
        base + Duration::days(2),
        &["weight_kg: 3", "tags: [foo]", "triggers: [x_y]"],
    );

    let output = bin()
        .args([
            "--path",
            tmp.path().to_str().unwrap(),
            "--fields",
            "weight_kg",
            "--has-tag",
            "foo",
            "bar",
            "--has-trigger",
            "a_b",
            "--format",
            "json",
        ])
        .output()
        .expect("run frontmatter-query");

    assert!(output.status.success(), "expected success: {:?}", output);

    let json = parse_json(&output.stdout);
    let results = json.as_array().expect("json array results");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0]["date"].as_str().unwrap(),
        base.format("%Y-%m-%d").to_string()
    );
    assert_eq!(results[0]["weight_kg"].as_f64().unwrap(), 1.0);
}

#[test]
fn list_tags_returns_counts_sorted() {
    let tmp = TempDir::new().expect("tempdir");
    let base = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();

    write_entry(tmp.path(), "a.md", base, &["tags: [foo, bar]"]);
    write_entry(
        tmp.path(),
        "b.md",
        base + Duration::days(1),
        &["tags: [foo]"],
    );
    write_entry(
        tmp.path(),
        "c.md",
        base + Duration::days(2),
        &["tags: [baz]"],
    );

    let output = bin()
        .args(["--path", tmp.path().to_str().unwrap(), "--list-tags", "--format", "json"])
        .output()
        .expect("run frontmatter-query");

    assert!(output.status.success(), "expected success: {:?}", output);

    let json = parse_json(&output.stdout);
    let tags = json["tags"].as_array().expect("tags array");
    assert_eq!(tags.len(), 3);

    assert_eq!(tags[0]["value"].as_str().unwrap(), "foo");
    assert_eq!(tags[0]["count"].as_u64().unwrap(), 2);

    assert_eq!(tags[1]["value"].as_str().unwrap(), "bar");
    assert_eq!(tags[1]["count"].as_u64().unwrap(), 1);

    assert_eq!(tags[2]["value"].as_str().unwrap(), "baz");
    assert_eq!(tags[2]["count"].as_u64().unwrap(), 1);
}

