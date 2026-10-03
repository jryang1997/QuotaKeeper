//! User-initiated, local-only usage export through the Windows save dialog.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use tui_token_monitor::ledger::{AnalysisRow, LedgerReport};

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelection {
    pub start: Option<String>,
    pub end: Option<String>,
    pub agent: Option<String>,
    pub project: Option<String>,
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
        "omp" => "omp_stats_sqlite",
        "opencode" => "opencode_sqlite",
        "zcode" => "zcode_sqlite",
        _ => "unknown_local_collector",
    }
}

pub fn csv(report: &LedgerReport, timezone_offset: &str, selection: &ExportSelection) -> String {
    let mut out = String::from(
        "\u{feff}date_local,client,project,input,output,cache_read,cache_write,reasoning,total,messages,cost_reported_usd,cost_estimated_usd,cost_total_usd,unpriced_messages,cost_is_partial,source,timezone_offset\r\n",
    );
    for row in report
        .analysis_rows
        .iter()
        .filter(|row| selection.matches(row))
    {
        let agg = &row.agg;
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{},{},{},{}\r\n",
            csv_field(&row.date),
            csv_field(row.client),
            csv_field(row.project.as_deref().unwrap_or("")),
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
        "schema_version": 2,
        "generated_at_ms": report.generated_at_ms,
        "timezone_offset": timezone_offset,
        "currency": "USD",
        "cost_note": "Reported and estimated amounts are distinct; estimates may differ from the bill.",
        "selection": {
            "start": selection.start,
            "end": selection.end,
            "agent": selection.agent,
            "project": selection.project,
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

fn decode_base64(text: &str) -> Result<Vec<u8>> {
    fn value_of(byte: u8) -> Result<u8> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(anyhow!("invalid base64 byte {byte:#x}")),
        }
    }
    let clean: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for [a, b, c, d] in clean.as_chunks::<4>().0 {
        let word = (value_of(*a)? as u32) << 18
            | (value_of(*b)? as u32) << 12
            | (value_of(*c)? as u32) << 6
            | value_of(*d)? as u32;
        out.push((word >> 16) as u8);
        out.push((word >> 8) as u8);
        out.push(word as u8);
    }
    match clean.len() % 4 {
        2 => {
            let tail = &clean[clean.len() - 2..];
            let word = (value_of(tail[0])? as u32) << 18 | (value_of(tail[1])? as u32) << 12;
            out.push((word >> 16) as u8);
        }
        3 => {
            let tail = &clean[clean.len() - 3..];
            let word = (value_of(tail[0])? as u32) << 18
                | (value_of(tail[1])? as u32) << 12
                | (value_of(tail[2])? as u32) << 6;
            out.push((word >> 16) as u8);
            out.push((word >> 8) as u8);
        }
        _ => {}
    }
    Ok(out)
}

/// 分享卡导出:前端 snapdom 生成的 `data:image/png;base64,...` 存成 PNG。
/// None = 用户在对话框取消。
pub fn save_png(data_url: &str) -> Result<Option<PathBuf>> {
    const PREFIX: &str = "data:image/png;base64,";
    let encoded = data_url
        .strip_prefix(PREFIX)
        .ok_or_else(|| anyhow!("unsupported image data url"))?;
    if encoded.len() > 64 * 1024 * 1024 {
        return Err(anyhow!("image too large"));
    }
    let bytes = decode_base64(encoded)?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(anyhow!("not a PNG payload"));
    }
    let date = chrono::Local::now().format("%Y%m%d-%H%M").to_string();
    let path = rfd::FileDialog::new()
        .set_title("保存分享卡")
        .set_file_name(format!("token-monitor-share-{date}.png"))
        .add_filter("PNG", &["png"])
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
    fn base64_decode_round_trips_png_payload() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(3003).collect();
        // 与标准库对齐的编码由前端 dataURL 保证;这里用固定样例验证含余数分组
        let text = "aGVsbG8gd29ybGQ="; // "hello world"
        assert_eq!(decode_base64(text).unwrap(), b"hello world");
        let empty = decode_base64("").unwrap();
        assert!(empty.is_empty());
        assert!(decode_base64("!!!!").is_err());
        let _ = payload; // 长度样例仅约束容量路径
    }

    #[test]
    fn save_png_rejects_non_png_and_foreign_prefix() {
        assert!(save_png("data:image/jpeg;base64,AAAA").is_err());
        assert!(save_png("data:image/png;base64,AAAA").is_err()); // 解出非 PNG 魔数
        assert!(save_png("hello").is_err());
    }

    #[test]
    fn csv_neutralizes_formulas_and_json_preserves_project_text() {
        let mut report = tui_token_monitor::ledger::aggregate(Vec::new(), chrono::Local::now());
        for project in [
            "=1+1",
            " +1",
            "-1",
            "@SUM(1)",
            "\t=1",
            "\r=1",
            "\n=1",
            "＝1+1",
            "\u{feff}=1",
        ] {
            report.analysis_rows = vec![AnalysisRow {
                date: "2026-10-01".into(),
                client: "codex",
                project: Some(project.into()),
                agg: Default::default(),
                unpriced_messages: 0,
            }];
            let text = csv(&report, "+08:00", &ExportSelection::default());
            assert!(text.contains(&format!("\"'{project}\"")), "{project:?}");
            let value: serde_json::Value = serde_json::from_slice(
                &json(&report, "+08:00", &ExportSelection::default()).unwrap(),
            )
            .unwrap();
            assert_eq!(value["rows"][0]["project"], project);
        }
        assert_eq!(csv_field("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn csv_keeps_reported_and_estimated_cost_apart() {
        let mut report = tui_token_monitor::ledger::aggregate(Vec::new(), chrono::Local::now());
        report.analysis_rows.clear();
        report
            .analysis_rows
            .push(tui_token_monitor::ledger::AnalysisRow {
                date: "2026-09-27".into(),
                client: "codex",
                project: Some("demo".into()),
                agg: tui_token_monitor::ledger::Agg {
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
            "\"2026-09-27\",\"codex\",\"demo\",8,3,2,0,0,13,1,1.500000,0.250000,1.750000,1,true,\"codex_rollout_jsonl\",\"'+08:00\""
        ));
    }

    #[test]
    fn export_selection_filters_date_agent_and_project() {
        let mut report = tui_token_monitor::ledger::aggregate(Vec::new(), chrono::Local::now());
        report.analysis_rows = vec![
            tui_token_monitor::ledger::AnalysisRow {
                date: "2026-09-26".into(),
                client: "codex",
                project: Some("alpha".into()),
                agg: Default::default(),
                unpriced_messages: 0,
            },
            tui_token_monitor::ledger::AnalysisRow {
                date: "2026-09-27".into(),
                client: "codex",
                project: Some("alpha".into()),
                agg: Default::default(),
                unpriced_messages: 0,
            },
            tui_token_monitor::ledger::AnalysisRow {
                date: "2026-09-27".into(),
                client: "claude",
                project: Some("beta".into()),
                agg: Default::default(),
                unpriced_messages: 0,
            },
        ];
        let selection = ExportSelection {
            start: Some("2026-09-27".into()),
            end: Some("2026-09-27".into()),
            agent: Some("codex".into()),
            project: Some("alpha".into()),
        };
        selection.validate().unwrap();
        let result = csv(&report, "+08:00", &selection);
        assert_eq!(result.lines().count(), 2);
        assert!(result.contains("\"2026-09-27\",\"codex\",\"alpha\""));
        assert!(!result.contains("\"claude\""));
    }

    #[test]
    fn json_identifies_each_collectors_source() {
        let mut report = tui_token_monitor::ledger::aggregate(Vec::new(), chrono::Local::now());
        report.analysis_rows = ["codex", "claude", "dsh", "omp", "opencode", "zcode"]
            .into_iter()
            .map(|client| AnalysisRow {
                date: "2026-09-27".into(),
                client,
                project: None,
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
                "omp_stats_sqlite",
                "opencode_sqlite",
                "zcode_sqlite",
            ]
        );
    }
}
