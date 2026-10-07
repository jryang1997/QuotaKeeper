fn main() -> anyhow::Result<()> {
    let key = quotakeeper::credential::command_code_key()?.or_else(|| std::env::var("COMMAND_CODE_API_KEY").ok()).or_else(|| std::env::var("CMD_API_KEY").ok()).or_else(|| {
        let home = std::env::var("USERPROFILE").ok()?;
        let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{home}/.commandcode/auth.json")).ok()?).ok()?;
        value.get("apiKey")?.as_str().map(str::to_owned)
    }).ok_or_else(|| anyhow::anyhow!("No credential"))?;
    for path in ["/alpha/billing/subscriptions", "/alpha/billing/credits"] {
        match ureq::AgentBuilder::new().try_proxy_from_env(false).build().get(&format!("https://api.commandcode.ai{path}"))
            .set("Authorization", &format!("Bearer {key}"))
            .timeout(std::time::Duration::from_secs(15)).call() {
            Ok(response) => {
                let value: serde_json::Value = response.into_json()?;
                println!("{path}: {}", redact(&value));
            }
            Err(error) => println!("{path}: {error}"),
        }
    }
    let report = quotakeeper::goat::fetch_report()?;
    println!("plan={}, limits={:?}", report.plan_type, report.limits.iter().map(|l| (&l.label, l.used_pct)).collect::<Vec<_>>());
    assert!(report.limits.iter().any(|l| l.label.starts_with("Monthly")), "monthly progress missing");
    Ok(())
}

fn redact(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), match k.as_str() {
            "planId" | "status" | "currentPeriodEnd" | "currentPeriodStart" | "monthlyCredits" | "cap" | "used" | "resetAt" => v.clone(),
            _ if v.is_object() || v.is_array() => redact(v),
            _ => Value::String("<REDACTED>".into()),
        })).collect()),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        _ => Value::String("<REDACTED>".into()),
    }
}
