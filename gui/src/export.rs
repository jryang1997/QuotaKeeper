//! User-initiated, local-only usage export through the Windows save dialog.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use quotakeeper::ledger::{AnalysisRow, LedgerReport};

pub fn pin_report(
    latest: Option<&Arc<LedgerReport>>,
    report_id: Option<&str>,
) -> Option<Arc<LedgerReport>> {
    let report = latest?;
    (report_id == Some(report.report_id.as_str())).then(|| Arc::clone(report))
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelection {
    pub start: Option<String>,
    pub end: Option<String>,
    pub agent: Option<String>,
    pub project: Option<String>,
    pub model: Option<String>,
}

impl ExportSelection {
    fn matches(&self, row: &AnalysisRow) -> bool {
        self.start.as_ref().is_none_or(|start| &row.date >= start)
            && self.end.as_ref().is_none_or(|end| &row.date <= end)
            && self.agent.as_ref().is_none_or(|agent| row.client == agent)
            && self
                .project
                .as_ref()
                .is_none_or(|project| row.project.as_ref() == Some(project))
            && self.model.as_ref().is_none_or(|model| &row.model == model)
    }

    fn validate(&self) -> Result<()> {
        let exact_date = |value: &str| {
            chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .filter(|date| value.len() == 10 && date.format("%Y-%m-%d").to_string() == value)
        };
        let (Some(start), Some(end)) = (
            self.start.as_deref().and_then(exact_date),
            self.end.as_deref().and_then(exact_date),
        ) else {
            return Err(anyhow!("invalid date range"));
        };
        if !(1..=366).contains(&(end.signed_duration_since(start).num_days() + 1)) {
            return Err(anyhow!("invalid date range"));
        }
        Ok(())
    }
}

fn csv_field(value: &str) -> String {
    // CSV 是电子表格展示格式;JSON 保留原始文本。
    let trimmed = value.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let prefix = if trimmed.starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠'])
        || value.starts_with(['\t', '\r', '\n'])
    {
        "'"
    } else {
        ""
    };
    format!("\"{prefix}{}\"", value.replace('"', "\"\""))
}

/// Collector identity, rather than an individual file path: rows are aggregated by day.
fn source_for_client(client: &str) -> &'static str {
    match client {
        "codex" => "codex_rollout_jsonl",
        "claude" => "claude_project_jsonl",
        "dsh" => "dsh_session_jsonl_zstd",
        "omp" => "omp_session_jsonl",
        "opencode" => "opencode_sqlite",
        "zcode" => "zcode_sqlite",
        _ => "unknown_local_collector",
    }
}

const COLLECTION_NOTE: &str =
    "Local records only; no read failure does not prove complete historical capture.";
const COST_NOTE: &str =
    "Reported and estimated USD amounts are distinct. Estimates use this report's frozen price snapshot, not historical tariffs or subscription charges.";

#[derive(serde::Serialize)]
struct CollectionStatus {
    client: &'static str,
    read_failed: bool,
}

fn collection_status(report: &LedgerReport, selection: &ExportSelection) -> Vec<CollectionStatus> {
    report
        .sources
        .iter()
        .filter(|source| selection.agent.as_ref().is_none_or(|agent| source.client == agent))
        .map(|source| CollectionStatus {
            client: source.client,
            read_failed: source.error.is_some(),
        })
        .collect()
}

pub fn csv(report: &LedgerReport, selection: &ExportSelection) -> String {
    let mut out = String::from(
        "\u{feff}date_local,client,project,model,raw_model,input,output,cache_read,cache_write,reasoning,total,messages,cost_reported_usd,cost_estimated_usd,cost_total_usd,unpriced_messages,cost_is_partial,source,timezone_offset,report_id,generated_at_ms,currency,price_basis_id,price_basis,price_snapshot,collection_status,collection_note,cost_note\r\n",
    );
    let snapshot = serde_json::to_string(&report.price_snapshot).expect("serializable price snapshot");
    let collection = serde_json::to_string(&collection_status(report, selection))
        .expect("serializable collection status");
    for row in report
        .analysis_rows
        .iter()
        .filter(|row| selection.matches(row))
    {
        let agg = &row.agg;
        let basis = serde_json::to_string(&report.price_bases[row.price_basis_id])
            .expect("serializable price basis");
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},",
            csv_field(&row.date),
            csv_field(row.client),
            csv_field(row.project.as_deref().unwrap_or("")),
            csv_field(&row.model),
            csv_field(&row.raw_model),
            agg.input,
            agg.output,
            agg.cache_read,
            agg.cache_write,
            agg.reasoning,
            agg.total(),
            agg.messages,
            agg.cost_reported,
            agg.cost_estimated,
            agg.cost(),
            row.unpriced_messages,
            row.unpriced_messages > 0,
            csv_field(source_for_client(row.client)),
            csv_field(&report.timezone_offset),
        ));
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\r\n",
            csv_field(&report.report_id),
            report.generated_at_ms,
            csv_field("USD"),
            row.price_basis_id,
            csv_field(&basis),
            csv_field(&snapshot),
            csv_field(&collection),
            csv_field(COLLECTION_NOTE),
            csv_field(COST_NOTE),
        ));
    }
    out
}

pub fn json(report: &LedgerReport, selection: &ExportSelection) -> Result<Vec<u8>> {
    let selected: Vec<_> = report
        .analysis_rows
        .iter()
        .filter(|row| selection.matches(row))
        .collect();
    let price_bases: Vec<_> = selected
        .iter()
        .map(|row| row.price_basis_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| crate::ipc::PriceBasisDto {
            id,
            basis: report.price_bases[id].clone(),
        })
        .collect();
    let rows: Vec<_> = selected
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "date_local": row.date,
                "client": row.client,
                "project": row.project,
                "model": row.model,
                "raw_model": row.raw_model,
                "price_basis_id": row.price_basis_id,
                "agg": crate::ipc::agg_dto(&row.agg),
                "cost_reported_usd": row.agg.cost_reported,
                "cost_estimated_usd": row.agg.cost_estimated,
                "unpriced_messages": row.unpriced_messages,
                "cost_is_partial": row.unpriced_messages > 0,
                "source": source_for_client(row.client),
            })
        })
        .collect();
    Ok(serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 4,
        "report_id": report.report_id,
        "generated_at_ms": report.generated_at_ms,
        "timezone_offset": report.timezone_offset,
        "price_snapshot": report.price_snapshot,
        "price_bases": price_bases,
        "currency": "USD",
        "cost_note": COST_NOTE,
        "collection_status": collection_status(report, selection),
        "collection_note": COLLECTION_NOTE,
        "selection": {
            "start": selection.start,
            "end": selection.end,
            "agent": selection.agent,
            "project": selection.project,
            "model": selection.model,
        },
        "rows": rows,
    }))?)
}

/// None means the user canceled the dialog; no file is written.
pub fn save_report(
    report: &LedgerReport,
    format: &str,
    selection: &ExportSelection,
) -> Result<Option<PathBuf>> {
    selection.validate()?;
    let date = chrono::Local::now().format("%Y%m%d").to_string();
    let (extension, label, bytes) = match format {
        "csv" => ("csv", "CSV", csv(report, selection).into_bytes()),
        "json" => ("json", "JSON", json(report, selection)?),
        _ => return Err(anyhow!("unsupported export format")),
    };
    let path = rfd::FileDialog::new()
        .set_title("导出 Token 用量")
        .set_file_name(format!("token-monitor-{date}.{extension}"))
        .add_filter(label, &[extension])
        .save_file();
    if let Some(path) = path {
        std::fs::write(&path, bytes)?;
        Ok(Some(path))
    } else {
        Ok(None)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use quotakeeper::ledger::{self, Agg, TokenBreakdown, UsageRecord, pricing::PriceBasis};

    fn empty_report() -> LedgerReport {
        let mut report = ledger::aggregate(
            Vec::new(),
            chrono::Local::now(),
            &ledger::pricing::Pricing::default(),
        );
        report.price_bases.push(PriceBasis::Reported);
        report
    }

    fn cost_report(reported: f64, estimated: f64) -> LedgerReport {
        let now = chrono::Local::now();
        let pricing = crate::ipc::fixture_pricing(serde_json::json!({
            "openai/gpt-5": { "input": estimated, "output": 0.0 }
        }));
        let records = [
            (
                TokenBreakdown {
                    input: 7,
                    output: 3,
                    cache_read: 1,
                    ..Default::default()
                },
                Some(reported),
                false,
            ),
            (
                TokenBreakdown {
                    input: 1,
                    ..Default::default()
                },
                Some(999.0),
                true,
            ),
            (
                TokenBreakdown {
                    cache_read: 1,
                    ..Default::default()
                },
                None,
                false,
            ),
        ]
        .map(|(tokens, cost, cost_is_estimate)| UsageRecord {
            client: "codex",
            model: "openai/gpt-5".into(),
            ts_ms: now.timestamp_millis(),
            tokens,
            cost,
            cost_is_estimate,
            session: None,
            project: Some("demo".into()),
            duration_ms: 0,
            ttft_ms: 0,
        });
        ledger::aggregate(records.into(), now, &pricing)
    }

    fn csv_rows(text: &str) -> Vec<Vec<String>> {
        let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
        let mut rows = Vec::new();
        let mut row = Vec::new();
        let mut field = String::new();
        let mut quoted = false;
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => row.push(std::mem::take(&mut field)),
                '\r' if !quoted => {
                    assert_eq!(chars.next(), Some('\n'));
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
        assert!(!quoted && field.is_empty() && row.is_empty());
        rows
    }

    #[test]
    fn export_pin_rejects_missing_or_old_ids_and_holds_the_matching_report() {
        let original = std::sync::Arc::new(cost_report(1.5, 0.25));
        let mut latest = Some(std::sync::Arc::clone(&original));
        for id in [None, Some(""), Some("old-report")] {
            assert!(pin_report(latest.as_ref(), id).is_none());
        }
        assert!(pin_report(None, Some(&original.report_id)).is_none());
        let pinned = pin_report(latest.as_ref(), Some(&original.report_id)).unwrap();
        assert!(std::sync::Arc::ptr_eq(&pinned, &original));

        latest = Some(std::sync::Arc::new(cost_report(9.0, 2.0)));
        assert_ne!(pinned.report_id, latest.as_ref().unwrap().report_id);
        assert_eq!(pinned.today.cost(), 1.75);
        assert_eq!(pinned.price_snapshot, original.price_snapshot);
        assert!(pin_report(latest.as_ref(), Some(&original.report_id)).is_none());
    }

    #[test]
    fn save_requires_real_exact_iso_boundaries_and_at_most_366_inclusive_days() {
        for (start, end) in [
            ("2024-02-29", "2024-02-29"),
            ("2024-01-01", "2024-12-31"),
            ("2023-01-01", "2024-01-01"),
            ("2026-01-01", "2026-12-31"),
        ] {
            ExportSelection {
                start: Some(start.into()),
                end: Some(end.into()),
                ..Default::default()
            }
            .validate()
            .unwrap();
        }
        let report = empty_report();
        for (start, end) in [
            (None, None),
            (Some("2026-10-01"), None),
            (None, Some("2026-10-01")),
            (Some("2026-1-01"), Some("2026-01-01")),
            (Some("2026-01-1"), Some("2026-01-01")),
            (Some("2026-02-29"), Some("2026-03-01")),
            (Some("2026-04-31"), Some("2026-05-01")),
            (Some("2026-13-01"), Some("2026-13-02")),
            (Some(" 2026-01-01"), Some("2026-01-01")),
            (Some("2026-01-01T00:00:00Z"), Some("2026-01-02")),
            (Some("2026-10-02"), Some("2026-10-01")),
            (Some("2024-01-01"), Some("2025-01-01")),
        ] {
            let selection = ExportSelection {
                start: start.map(str::to_owned),
                end: end.map(str::to_owned),
                ..Default::default()
            };
            assert!(selection.validate().is_err(), "{start:?}..{end:?}");
            assert!(save_report(&report, "json", &selection).is_err());
        }
    }

    #[test]
    fn json_subset_retains_original_nonconsecutive_bases_and_frozen_metadata() {
        let mut report = cost_report(1.5, 0.25);
        assert_eq!(report.price_bases.len(), 3);
        report.timezone_offset = "+05:45".into();
        for row in &mut report.analysis_rows {
            row.project = Some(
                if row.price_basis_id == 1 { "excluded" } else { "selected" }.into(),
            );
        }
        report.sources = vec![ledger::SourceStatus {
            client: "codex",
            records: 3,
            error: Some("private diagnostic C:\\Users\\secret\\records.jsonl".into()),
        }];
        let selection = ExportSelection {
            agent: Some("codex".into()),
            project: Some("selected".into()),
            ..Default::default()
        };
        let snapshot = serde_json::to_value(&report.price_snapshot).unwrap();
        let newer_report = cost_report(99.0, 5.0);
        assert_ne!(report.price_snapshot.version, newer_report.price_snapshot.version);
        let bytes = json(&report, &selection).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["schema_version"], 4);
        assert_eq!(value["report_id"], report.report_id);
        assert_eq!(value["timezone_offset"], "+05:45");
        assert_eq!(value["price_snapshot"], snapshot);
        let bases = value["price_bases"].as_array().unwrap();
        assert_eq!(
            bases.iter().map(|entry| entry["id"].as_u64().unwrap()).collect::<Vec<_>>(),
            [0, 2]
        );
        for entry in bases {
            let id = entry["id"].as_u64().unwrap() as usize;
            assert_eq!(
                entry["basis"],
                serde_json::to_value(&report.price_bases[id]).unwrap()
            );
        }
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert!(bases.iter().any(|entry| entry["id"] == row["price_basis_id"]));
            assert_eq!(row["project"], "selected");
        }
        assert_eq!(
            value["collection_status"],
            serde_json::json!([{ "client": "codex", "read_failed": true }])
        );
        assert!(value["collection_note"].as_str().unwrap().contains("historical"));
        assert!(!String::from_utf8(bytes).unwrap().contains("secret"));
    }

    #[test]
    fn csv_rows_are_self_contained_formula_safe_and_use_collected_metadata() {
        let mut report = cost_report(1.25e-10, 3.75e-10);
        report.report_id = "=SUM(1,2)".into();
        report.timezone_offset = "+05:45".into();
        report.price_snapshot.version = " @version,\"quoted\"\nnext".into();
        report.price_snapshot.dataset_url = Some("\t=unsafe".into());
        for basis in &mut report.price_bases {
            if let PriceBasis::Estimated(estimate) = basis {
                estimate.matched_entry = "＝formula,\"entry\"\nline".into();
                estimate.input.entry = "=input".into();
            }
        }
        report.sources = vec![
            ledger::SourceStatus {
                client: "codex",
                records: 3,
                error: Some("private /home/secret/log.jsonl".into()),
            },
            ledger::SourceStatus {
                client: "omp",
                records: 0,
                error: Some("another private path".into()),
            },
        ];
        let selection = ExportSelection {
            agent: Some("codex".into()),
            ..Default::default()
        };
        let text = csv(&report, &selection);
        let table = csv_rows(&text);
        assert_eq!(table.len(), 4);
        let columns = &table[0];
        for fields in &table[1..] {
            assert_eq!(fields.len(), columns.len());
            let cell = |name: &str| {
                fields[columns.iter().position(|column| column == name).unwrap()].as_str()
            };
            assert_eq!(cell("report_id"), "'=SUM(1,2)");
            assert_eq!(cell("timezone_offset"), "'+05:45");
            assert_eq!(
                cell("generated_at_ms").parse::<i64>().unwrap(),
                report.generated_at_ms
            );
            assert_eq!(cell("currency"), "USD");
            let id = cell("price_basis_id").parse::<usize>().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(cell("price_basis")).unwrap(),
                serde_json::to_value(&report.price_bases[id]).unwrap()
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(cell("price_snapshot")).unwrap(),
                serde_json::to_value(&report.price_snapshot).unwrap()
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(cell("collection_status")).unwrap(),
                serde_json::json!([{ "client": "codex", "read_failed": true }])
            );
            assert!(cell("collection_note").contains("historical"));
            assert!(cell("cost_note").contains("frozen"));
            for name in [
                "report_id", "timezone_offset", "currency", "price_basis", "price_snapshot",
                "collection_status", "collection_note", "cost_note",
            ] {
                assert!(
                    !cell(name).starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠', '\t', '\r', '\n']),
                    "{name}"
                );
            }
        }
        assert!(!text.contains("secret"));
        assert!(!text.contains("another private"));
    }

    #[test]
    fn csv_neutralizes_formulas_and_json_preserves_all_text_fields() {
        let mut report = empty_report();
        for project in [
            "=1+1",
            " +1",
            "-1",
            "@SUM(1)",
            "\t=1",
            "\r=1",
            "\n=1",
            "＝1+1",
            "＋1",
            "－1",
            "＠SUM(1)",
            "\u{feff}=1",
            " \u{feff}\t=1",
            "=SUM(1,2)",
            "=HYPERLINK(\"https://example.com\",\"label\")",
        ] {
            report.analysis_rows = vec![AnalysisRow {
                date: "2026-10-01".into(),
                client: "codex",
                project: Some(project.into()),
                model: project.into(),
                raw_model: project.into(),
                agg: Default::default(),
                unpriced_messages: 0,
                price_basis_id: 0,
            }];
            let text = csv(&report, &ExportSelection::default());
            let safe_field = format!("\"'{}\"", project.replace('"', "\"\""));
            assert_eq!(text.matches(&safe_field).count(), 3, "{project:?}");
            let value: serde_json::Value = serde_json::from_slice(
                &json(&report, &ExportSelection::default()).unwrap(),
            )
            .unwrap();
            assert_eq!(value["rows"][0]["project"], project);
            assert_eq!(value["rows"][0]["model"], project);
            assert_eq!(value["rows"][0]["raw_model"], project);
        }
        assert_eq!(csv_field("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn csv_keeps_reported_and_estimated_cost_apart() {
        let report = cost_report(1.5, 0.25);
        assert_eq!(report.analysis_rows.len(), 3);
        let total = report
            .analysis_rows
            .iter()
            .fold(Agg::default(), |mut sum, row| {
                sum.merge(&row.agg);
                sum
            });
        assert_eq!(
            (total.input, total.output, total.cache_read, total.total()),
            (8, 3, 2, 13)
        );
        assert_eq!(
            (total.cost_reported, total.cost_estimated, total.cost()),
            (1.5, 0.25, 1.75)
        );
        let result = csv(&report, &ExportSelection::default());
        assert_eq!(result.lines().count(), 4);
        for costs in [
            ",1.5,0,1.5,0,false,",
            ",0,0.25,0.25,0,false,",
            ",0,0,0,1,true,",
        ] {
            assert!(result.contains(costs), "{result}");
        }
        for row in &report.analysis_rows {
            assert_eq!(
                row.unpriced_messages,
                if matches!(
                    report.price_bases[row.price_basis_id],
                    PriceBasis::Unpriced { .. }
                ) {
                    row.agg.messages
                } else {
                    0
                }
            );
        }
    }

    #[test]
    fn exports_round_trip_tiny_and_full_precision_costs() {
        for (reported, estimated) in [
            (1.25e-10, 3.75e-10),
            (1.2345678901234567, 2.345678901234567),
        ] {
            let report = cost_report(reported, estimated);
            let text = csv(&report, &ExportSelection::default());
            let table = csv_rows(&text);
            let columns = &table[0];
            let rows = &table[1..];
            assert_eq!(rows.len(), 3);
            assert!(rows.iter().all(|fields| fields.len() == columns.len()));
            for (column, expected) in [
                ("cost_reported_usd", reported),
                ("cost_estimated_usd", estimated),
                ("cost_total_usd", reported + estimated),
            ] {
                let index = columns.iter().position(|name| *name == column).unwrap();
                let values: Vec<f64> = rows
                    .iter()
                    .map(|fields| fields[index].parse().unwrap())
                    .collect();
                assert_eq!(values.iter().sum::<f64>(), expected, "{column}");
                for row in &report.analysis_rows {
                    let expected_row = match column {
                        "cost_reported_usd" => row.agg.cost_reported,
                        "cost_estimated_usd" => row.agg.cost_estimated,
                        _ => row.agg.cost(),
                    };
                    assert!(values.contains(&expected_row));
                }
            }
            let value: serde_json::Value = serde_json::from_slice(
                &json(&report, &ExportSelection::default()).unwrap(),
            )
            .unwrap();
            let rows = value["rows"].as_array().unwrap();
            assert_eq!(rows.len(), 3);
            let source = rows
                .iter()
                .find(|row| row["cost_reported_usd"].as_f64() == Some(reported))
                .unwrap();
            assert_eq!(source["agg"]["cost"].as_f64().unwrap(), reported);
            assert_eq!(source["cost_estimated_usd"].as_f64().unwrap(), 0.0);
            let estimate = rows
                .iter()
                .find(|row| row["cost_estimated_usd"].as_f64() == Some(estimated))
                .unwrap();
            assert_eq!(estimate["agg"]["cost"].as_f64().unwrap(), estimated);
            assert_eq!(estimate["cost_reported_usd"].as_f64().unwrap(), 0.0);
            assert_eq!(
                rows.iter()
                    .filter(|row| row["cost_is_partial"] == true)
                    .count(),
                1
            );
            assert_eq!(
                rows.iter()
                    .map(|row| row["unpriced_messages"].as_u64().unwrap())
                    .sum::<u64>(),
                1
            );
            assert_eq!(
                rows.iter()
                    .map(|row| row["agg"]["cost"].as_f64().unwrap())
                    .sum::<f64>(),
                reported + estimated
            );
        }
    }

    #[test]
    fn export_selection_filters_exact_canonical_model_date_agent_and_project() {
        let mut report = empty_report();
        report.analysis_rows = [
            (
                "2026-09-26",
                "codex",
                Some("alpha"),
                "gpt-5",
                "openai/gpt-5",
            ),
            (
                "2026-09-27",
                "codex",
                Some("alpha"),
                "gpt-5",
                "OpenAI/GPT-5",
            ),
            (
                "2026-09-27",
                "codex",
                Some("alpha"),
                "gpt-5",
                "openrouter/gpt-5",
            ),
            (
                "2026-09-28",
                "codex",
                Some("alpha"),
                "gpt-5",
                "openai/gpt-5",
            ),
            (
                "2026-09-27",
                "claude",
                Some("alpha"),
                "gpt-5",
                "openai/gpt-5",
            ),
            ("2026-09-27", "codex", Some("beta"), "gpt-5", "openai/gpt-5"),
            ("2026-09-27", "codex", None, "gpt-5", "openai/gpt-5"),
            (
                "2026-09-27",
                "codex",
                Some("alpha"),
                "gpt-5-mini",
                "openai/gpt-5-mini",
            ),
            (
                "2026-09-27",
                "codex",
                Some("alpha"),
                "gpt-5.1",
                "openai/gpt-5.1",
            ),
            ("2026-09-27", "codex", Some("alpha"), "unknown", ""),
        ]
        .into_iter()
        .map(|(date, client, project, model, raw_model)| AnalysisRow {
            date: date.into(),
            client,
            project: project.map(str::to_owned),
            model: model.into(),
            raw_model: raw_model.into(),
            agg: Default::default(),
            unpriced_messages: 0,
            price_basis_id: 0,
        })
        .collect();
        let selection: ExportSelection = serde_json::from_str(
            r#"{
            "start": "2026-09-27", "end": "2026-09-27",
            "agent": "codex", "project": "alpha", "model": "gpt-5"
        }"#,
        )
        .unwrap();
        selection.validate().unwrap();
        let result = csv(&report, &selection);
        assert_eq!(result.lines().count(), 3);
        assert!(result.starts_with("\u{feff}date_local,client,project,model,raw_model,input,"));
        assert!(result.contains("\"2026-09-27\",\"codex\",\"alpha\""));
        assert!(result.contains("\"gpt-5\",\"OpenAI/GPT-5\""));
        assert!(result.contains("\"gpt-5\",\"openrouter/gpt-5\""));
        assert!(!result.contains("\"claude\""));
        let value: serde_json::Value =
            serde_json::from_slice(&json(&report, &selection).unwrap()).unwrap();
        assert_eq!(value["schema_version"], 4);
        assert_eq!(value["selection"]["start"], "2026-09-27");
        assert_eq!(value["selection"]["end"], "2026-09-27");
        assert_eq!(value["selection"]["agent"], "codex");
        assert_eq!(value["selection"]["project"], "alpha");
        assert_eq!(value["selection"]["model"], "gpt-5");
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row["model"] == "gpt-5"));
        assert_eq!(rows[0]["raw_model"], "OpenAI/GPT-5");
        assert_eq!(rows[1]["raw_model"], "openrouter/gpt-5");

        let all: serde_json::Value =
            serde_json::from_slice(&json(&report, &ExportSelection::default()).unwrap())
                .unwrap();
        assert_eq!(all["rows"].as_array().unwrap().len(), 10);
        assert_eq!(
            all["selection"].get("model"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(all["rows"][9]["model"], "unknown");
        assert_eq!(all["rows"][9]["raw_model"], "");

        let no_match: ExportSelection = serde_json::from_str(r#"{"model":"GPT-5"}"#).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&json(&report, &no_match).unwrap()).unwrap();
        assert!(value["rows"].as_array().unwrap().is_empty());
        assert_eq!(csv(&report, &no_match).lines().count(), 1);
    }

    #[test]
    fn json_identifies_each_collectors_source() {
        let mut report = empty_report();
        report.analysis_rows = ["codex", "claude", "dsh", "omp", "opencode", "zcode"]
            .into_iter()
            .map(|client| AnalysisRow {
                date: "2026-09-27".into(),
                client,
                project: None,
                model: "gpt-5".into(),
                raw_model: "openai/gpt-5".into(),
                agg: Default::default(),
                unpriced_messages: 0,
                price_basis_id: 0,
            })
            .collect();
        let value: serde_json::Value =
            serde_json::from_slice(&json(&report, &ExportSelection::default()).unwrap())
                .unwrap();
        let sources: Vec<_> = value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["source"].as_str().unwrap())
            .collect();
        assert_eq!(
            sources,
            [
                "codex_rollout_jsonl",
                "claude_project_jsonl",
                "dsh_session_jsonl_zstd",
                "omp_session_jsonl",
                "opencode_sqlite",
                "zcode_sqlite",
            ]
        );
        let text = csv(&report, &ExportSelection::default());
        for source in sources {
            assert!(text.contains(&format!("\"{source}\"")));
        }
    }
}
