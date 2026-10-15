//! User-initiated, local-only usage export through the Windows save dialog.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use quotakeeper::ledger::{AnalysisRow, LedgerReport};

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
        let valid = |value: &str| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok();
        if self.start.as_deref().is_some_and(|value| !valid(value))
            || self.end.as_deref().is_some_and(|value| !valid(value))
            || self
                .start
                .as_ref()
                .zip(self.end.as_ref())
                .is_some_and(|(start, end)| start > end)
        {
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

pub fn csv(report: &LedgerReport, timezone_offset: &str, selection: &ExportSelection) -> String {
    let mut out = String::from(
        "\u{feff}date_local,client,project,model,raw_model,input,output,cache_read,cache_write,reasoning,total,messages,cost_reported_usd,cost_estimated_usd,cost_total_usd,unpriced_messages,cost_is_partial,source,timezone_offset\r\n",
    );
    for row in report
        .analysis_rows
        .iter()
        .filter(|row| selection.matches(row))
    {
        let agg = &row.agg;
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\r\n",
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
            csv_field(timezone_offset),
        ));
    }
    out
}

pub fn json(
    report: &LedgerReport,
    timezone_offset: &str,
    selection: &ExportSelection,
) -> Result<Vec<u8>> {
    let rows: Vec<_> = report
        .analysis_rows
        .iter()
        .filter(|row| selection.matches(row))
        .map(|row| {
            serde_json::json!({
                "date_local": row.date,
                "client": row.client,
                "project": row.project,
                "model": row.model,
                "raw_model": row.raw_model,
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
        "schema_version": 3,
        "generated_at_ms": report.generated_at_ms,
        "timezone_offset": timezone_offset,
        "currency": "USD",
        "cost_note": "Reported and estimated amounts are distinct; estimates may differ from the bill.",
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
    let timezone = chrono::Local::now().format("%:z").to_string();
    let (extension, label, bytes) = match format {
        "csv" => ("csv", "CSV", csv(report, &timezone, selection).into_bytes()),
        "json" => ("json", "JSON", json(report, &timezone, selection)?),
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

    #[test]
    fn csv_neutralizes_formulas_and_json_preserves_all_text_fields() {
        let mut report = quotakeeper::ledger::aggregate(Vec::new(), chrono::Local::now());
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
            }];
            let text = csv(&report, "+08:00", &ExportSelection::default());
            let safe_field = format!("\"'{}\"", project.replace('"', "\"\""));
            assert_eq!(text.matches(&safe_field).count(), 3, "{project:?}");
            let value: serde_json::Value = serde_json::from_slice(
                &json(&report, "+08:00", &ExportSelection::default()).unwrap(),
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
        let mut report = quotakeeper::ledger::aggregate(Vec::new(), chrono::Local::now());
        report.analysis_rows.clear();
        report.analysis_rows.push(quotakeeper::ledger::AnalysisRow {
            date: "2026-09-27".into(),
            client: "codex",
            project: Some("demo".into()),
            model: "gpt-5".into(),
            raw_model: "openai/gpt-5".into(),
            agg: quotakeeper::ledger::Agg {
                input: 8,
                output: 3,
                cache_read: 2,
                cache_write: 0,
                reasoning: 0,
                messages: 1,
                cost_reported: 1.5,
                cost_estimated: 0.25,
            },
            unpriced_messages: 1,
        });
        let result = csv(&report, "+08:00", &ExportSelection::default());
        assert!(result.contains(
            "\"2026-09-27\",\"codex\",\"demo\",\"gpt-5\",\"openai/gpt-5\",8,3,2,0,0,13,1,1.5,0.25,1.75,1,true,\"codex_rollout_jsonl\",\"'+08:00\""
        ));
    }

    #[test]
    fn exports_round_trip_tiny_and_full_precision_costs() {
        let mut report = quotakeeper::ledger::aggregate(Vec::new(), chrono::Local::now());
        for (reported, estimated) in [
            (1.25e-10, 3.75e-10),
            (1.2345678901234567, 2.345678901234567),
        ] {
            report.analysis_rows = vec![AnalysisRow {
                date: "2026-09-27".into(),
                client: "codex",
                project: None,
                model: "gpt-5".into(),
                raw_model: "OpenAI/GPT-5".into(),
                agg: quotakeeper::ledger::Agg {
                    input: 1,
                    messages: 1,
                    cost_reported: reported,
                    cost_estimated: estimated,
                    ..Default::default()
                },
                unpriced_messages: 1,
            }];
            let text = csv(&report, "+08:00", &ExportSelection::default());
            let mut lines = text.lines();
            let columns: Vec<_> = lines.next().unwrap().split(',').collect();
            let fields: Vec<_> = lines.next().unwrap().split(',').collect();
            assert_eq!(fields.len(), columns.len());
            for (column, expected) in [
                ("cost_reported_usd", reported),
                ("cost_estimated_usd", estimated),
                ("cost_total_usd", reported + estimated),
            ] {
                let index = columns.iter().position(|name| *name == column).unwrap();
                assert_eq!(fields[index].parse::<f64>().unwrap(), expected, "{column}");
            }
            let value: serde_json::Value = serde_json::from_slice(
                &json(&report, "+08:00", &ExportSelection::default()).unwrap(),
            )
            .unwrap();
            let row = &value["rows"][0];
            assert_eq!(row["cost_reported_usd"].as_f64().unwrap(), reported);
            assert_eq!(row["cost_estimated_usd"].as_f64().unwrap(), estimated);
            assert_eq!(row["agg"]["cost"].as_f64().unwrap(), reported + estimated);
            assert_eq!(row["unpriced_messages"], 1);
            assert_eq!(row["cost_is_partial"], true);
        }
    }

    #[test]
    fn export_selection_filters_exact_canonical_model_date_agent_and_project() {
        let mut report = quotakeeper::ledger::aggregate(Vec::new(), chrono::Local::now());
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
        let result = csv(&report, "+08:00", &selection);
        assert_eq!(result.lines().count(), 3);
        assert!(result.starts_with("\u{feff}date_local,client,project,model,raw_model,input,"));
        assert!(result.contains("\"2026-09-27\",\"codex\",\"alpha\""));
        assert!(result.contains("\"gpt-5\",\"OpenAI/GPT-5\""));
        assert!(result.contains("\"gpt-5\",\"openrouter/gpt-5\""));
        assert!(!result.contains("\"claude\""));
        let value: serde_json::Value =
            serde_json::from_slice(&json(&report, "+08:00", &selection).unwrap()).unwrap();
        assert_eq!(value["schema_version"], 3);
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
            serde_json::from_slice(&json(&report, "+08:00", &ExportSelection::default()).unwrap())
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
            serde_json::from_slice(&json(&report, "+08:00", &no_match).unwrap()).unwrap();
        assert!(value["rows"].as_array().unwrap().is_empty());
        assert_eq!(csv(&report, "+08:00", &no_match).lines().count(), 1);
    }

    #[test]
    fn json_identifies_each_collectors_source() {
        let mut report = quotakeeper::ledger::aggregate(Vec::new(), chrono::Local::now());
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
            })
            .collect();
        let value: serde_json::Value =
            serde_json::from_slice(&json(&report, "+08:00", &ExportSelection::default()).unwrap())
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
        let text = csv(&report, "+08:00", &ExportSelection::default());
        for source in sources {
            assert!(text.contains(&format!("\"{source}\"")));
        }
    }
}
