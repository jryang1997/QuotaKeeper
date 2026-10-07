//! 模型名归一化:不同来源(zcode/omp/opencode)对同一模型记录的名字
//! 大小写、供应商前缀、日期尾缀、p/点分变体各不相同,by_model 按
//! canonical 名分组才能把同一模型合并到一行。
//!
//! 规则(按序):
//! 1. 取最后一段(剥 `vendor/`、`databricks-`、`bedrock 点分` 等前缀,与
//!    pricing 后缀口径一致)
//! 2. 小写 + `_`→`-`
//! 3. 剥日期/快照尾缀(`-0731`、`-20250428`)与渠道变体(`:free/:batch/-latest`)
//! 4. `5p3`→`5.3` 点分还原(fireworks 用 p 当小数点)
//! 4b. 路由层 `auto-` 前缀剥除 + 单数字版本点还原(`v4-1`→`v4.1`,magpie
//!     类代理把点消毒成连字符)
//! 5. 显式别名表(官方改名/推广名),未命中走规则结果
//!
//! 注意:canonical 只管聚合分组;定价仍优先按原始名查真价(同一代不同档
//! 价格可能不同),lookup 失败时才落到 canonical 名。

/// 归一化为 canonical 名。空输入返回 "unknown"(与采集侧兜底一致)。
pub fn canonical(model: &str) -> String {
    let raw = model.trim();
    if raw.is_empty() {
        return "unknown".into();
    }
    // 1. 最后一段:剥供应商前缀(openrouter/z-ai/、bedrock 点分 zai. 等)。
    //    点分只用于 bedrock 式「org.model」两段名:仅当点后还有第二段且首段
    //    是已知 org 词(zai/moonshotai/minimax/deepseek/qwen)时才剥,避免把
    //    版本号 glm-5.3 当扩展名切断。
    let last = raw.rsplit('/').next().unwrap_or(raw);
    let last = match last.split_once('.') {
        Some((org, rest))
            if matches!(org.to_lowercase().as_str(), "zai" | "moonshotai" | "minimax" | "deepseek" | "qwen") =>
        {
            rest
        }
        _ => last,
    };
    // databricks 前缀(databricks-glm-5-3)
    let last = last.strip_prefix("databricks-").unwrap_or(last);
    // 2/3/4. 小写、下划线转连字符、剥尾缀、点分还原
    let mut name = last.to_lowercase().replace('_', "-");
    loop {
        let mut changed = false;
        // 渠道变体(:free/:batch/exacto)与 -latest
        if let Some(stripped) = name.strip_suffix("-latest") {
            name = stripped.into();
            changed = true;
        }
        if let Some((base, _variant)) = name.rsplit_once(':')
            && !base.is_empty()
        {
            name = base.into();
            changed = true;
        }
        // 推理力度尾缀(opencode/zcode 记录 effort 的方式):同一模型同价,并入基名
        if let Some((base, tail)) = name.rsplit_once('-')
            && matches!(tail, "high" | "xhigh" | "medium" | "low" | "minimal")
        {
            name = base.into();
            changed = true;
        }
        // 日期快照尾缀:全数字段且形如日期(YYYY / YYYY-MM / YYYY-MM-DD /
        // MMDD / YYMMDD / YYYYMMDD)。纯版本号数字(如 glm-5 的 5)不剥。
        if let Some((base, snap)) = name.rsplit_once('-') {
            let all_digits = snap.chars().all(|c| c.is_ascii_digit());
            let is_date = all_digits && is_date_like(snap);
            let parts: Vec<&str> = name.split('-').collect();
            // 复合日期:-YYYY-MM-DD(末三段 4/2/2 位)或 -YYYY-MM(末两段 4/2 位)
            let is_ymd = parts.len() >= 4
                && parts[parts.len() - 3].len() == 4
                && parts[parts.len() - 3].chars().all(|c| c.is_ascii_digit())
                && parts[parts.len() - 2].len() == 2
                && parts[parts.len() - 2].chars().all(|c| c.is_ascii_digit())
                && parts[parts.len() - 1].len() == 2
                && parts[parts.len() - 1].chars().all(|c| c.is_ascii_digit());
            let is_ym = parts.len() >= 3
                && parts[parts.len() - 2].len() == 4
                && parts[parts.len() - 2].starts_with("20")
                && parts[parts.len() - 2].chars().all(|c| c.is_ascii_digit())
                && parts[parts.len() - 1].len() == 2
                && parts[parts.len() - 1].chars().all(|c| c.is_ascii_digit());
            if !base.is_empty() && (is_date || is_ymd || is_ym) {
                name = base.into();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // 4b. 路由层命名(magpie 类代理):模型名被套 auto- 前缀、点分版本消毒成
    //     连字符(group/auto-deepseek-v4-1-flash)。剥路由前缀后把「两侧均为
    //     单独一位数字」的连字符还原成版本点;非 auto- 命名不动(databricks
    //     的 glm-5-3 等连字符拼写保持独立组)。
    if let Some(routed) = name.strip_prefix("auto-") {
        name = restore_version_dots(routed);
    }
    // 4. fireworks 的 p-当-点:5p3 → 5.3(仅 版本段,字母数字混合段不动)
    name = restore_dots(&name);
    // 5. 显式别名
    ALIAS.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| (*to).into())
}

/// 反向查别名表:canonical 名对应的原始短名(定价兜底用——目录里常有
/// 短名条目而没有全名条目,如 deepseek-flash ↔ deepseek-v4.1-flash).
pub fn source_names(canonical: &str) -> impl Iterator<Item = &'static str> {
    ALIAS
        .iter()
        .filter(move |(_, to)| *to == canonical)
        .map(|(from, _)| *from)
}

/// 全数字段是否形如日期快照:YYYY、MMDD、YYMMDD、YYYYMMDD。
/// 故意排除 1-3 位与 5-7 位:版本号(glm-5、gpt-4o)不能被剥。
fn is_date_like(snap: &str) -> bool {
    let n = snap.len();
    if !(n == 4 || n == 6 || n == 8) {
        return false;
    }
    let b = snap.as_bytes();
    let d = |i: usize| b[i].is_ascii_digit();
    match n {
        4 => b[0] == b'2' && b[1] == b'0' && d(2) && d(3)          // YYYY
            || (b[0] == b'0' || b[0] == b'1') && d(1) && d(2) && d(3) // MMDD
            || (b[0] == b'2' && d(1) && d(2) && d(3)),            // YYMM(25xx)
        6 => b[0] == b'2' && d(1) && d(2) && d(3) && d(4) && d(5), // YYMMDD/250731
        8 => b[0] == b'2' && b[1] == b'0' && d(2) && d(3) && d(4) && d(5) && d(6) && d(7), // YYYYMMDD
        _ => false,
    }
}

/// 还原路由层消毒掉的版本点:`单数字-单数字` → `单数字.单数字`
/// (deepseek-v4-1-flash → deepseek-v4.1-flash)。多位数字段不动,
/// qwen3-235b、3-70b、2-251201 这类尺寸/日期连字符不误伤。
fn restore_version_dots(name: &str) -> String {
    let b = name.as_bytes();
    let digit_run = |mut i: isize, step: isize| -> usize {
        let mut n = 0;
        while i >= 0 && (i as usize) < b.len() && b[i as usize].is_ascii_digit() {
            n += 1;
            i += step;
        }
        n
    };
    let mut out = String::with_capacity(name.len());
    for i in 0..b.len() {
        if b[i] == b'-'
            && i > 0
            && i + 1 < b.len()
            && digit_run(i as isize - 1, -1) == 1
            && digit_run(i as isize + 1, 1) == 1
        {
            out.push('.');
        } else {
            out.push(b[i] as char);
        }
    }
    out
}

/// fireworks 的 `p` 是小数点:glm-5p3 → glm-5.3。只在「字母-数字p-数字」
/// 边界替换,避免误伤普通词(parrot 里的 p)。
fn restore_dots(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut out = String::with_capacity(name.len());
    for i in 0..bytes.len() {
        let c = bytes[i] as char;
        let prev_digit = i > 0 && bytes[i - 1].is_ascii_digit();
        let next_digit = i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit();
        if c == 'p' && prev_digit && next_digit {
            out.push('.');
        } else {
            out.push(c);
        }
    }
    out
}

/// 官方改名/推广别名:from(规则化后的名) → to(canonical)。
/// 只收录确定的关系;拿不准的保持规则结果,不瞎合并。
const ALIAS: &[(&str, &str)] = &[
    // DeepSeek:deepseek-flash 是 v4.1-flash 的推广短名(omp 记录的就是它)
    ("deepseek-flash", "deepseek-v4.1-flash"),
    // GLM 官方短路名
    ("glm-flash-latest", "glm-5.3-flash"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_and_prefix_merge() {
        assert_eq!(canonical("GLM-5.3-Flash"), "glm-5.3-flash");
        assert_eq!(canonical("glm-5.3-flash"), "glm-5.3-flash");
        assert_eq!(canonical("zai/glm-5.3-flash"), "glm-5.3-flash");
        assert_eq!(canonical("openrouter/z-ai/glm-5.3-flash"), "glm-5.3-flash");
        assert_eq!(canonical("deepseek/deepseek-v4.1-flash"), "deepseek-v4.1-flash");
        assert_eq!(canonical("meta/muse-spark-1.3-contributor"), "muse-spark-1.3-contributor");
    }

    #[test]
    fn fireworks_p_dot_restored() {
        assert_eq!(canonical("fireworks_ai/accounts/fireworks/models/glm-5p3-flash"), "glm-5.3-flash");
        assert_eq!(canonical("fireworks_ai/glm-5p1"), "glm-5.1");
        assert_eq!(canonical("glm-5p3-flash:batch"), "glm-5.3-flash");
    }

    #[test]
    fn date_snapshot_stripped() {
        assert_eq!(canonical("dashscope/deepseek-v4-flash-0731"), "deepseek-v4-flash");
        assert_eq!(canonical("qwen-plus-2025-07-14"), "qwen-plus");
        assert_eq!(canonical("glm-4-7-251222"), "glm-4-7");
        // 非日期数字段不剥(尺寸/参数名)
        assert_eq!(canonical("qwen3-235b-a22b-2507"), "qwen3-235b-a22b");
    }

    #[test]
    fn effort_suffix_stripped() {
        // opencode/zcode 把推理力度记进模型名:同一模型并入基名
        assert_eq!(canonical("gemini-3.8-flash-high"), "gemini-3.8-flash");
        assert_eq!(canonical("glm-5.3-flash-xhigh"), "glm-5.3-flash");
        assert_eq!(canonical("deepseek-v4.1-flash-low"), "deepseek-v4.1-flash");
        // 非力度词不剥
        assert_eq!(canonical("deepseek-v4.1-flash"), "deepseek-v4.1-flash");
        assert_eq!(canonical("muse-spark-1.3-contributor"), "muse-spark-1.3-contributor");
    }

    /// 路由层(magpie 类代理)命名:模型名被套 auto- 前缀、点分版本消毒成
    /// 连字符(group/auto-deepseek-v4-1-flash)。剥路由前缀并还原版本点;
    /// 非 auto- 命名不受影响(databricks 连字符版保持独立组)。
    #[test]
    fn router_auto_prefix_and_mangled_version_dots_restored() {
        assert_eq!(canonical("group/auto-deepseek-v4-1-flash"), "deepseek-v4.1-flash");
        assert_eq!(canonical("group/auto-glm-5-3-flash"), "glm-5.3-flash");
        assert_eq!(canonical("group/auto-mimo-v2-6-flash"), "mimo-v2.6-flash");
        // 日期尾缀先剥,再还原版本点
        assert_eq!(canonical("group/auto-deepseek-v3-2-251201"), "deepseek-v3.2");
        // 两侧须均为单独一位数字:多位数字段(尺寸/日期)不误伤
        assert_eq!(canonical("group/auto-qwen3-235b-a22b"), "qwen3-235b-a22b");
        assert_eq!(canonical("group/auto-llama-3-3-70b"), "llama-3.3-70b");
        // 非 auto- 命名不动
        assert_eq!(canonical("databricks/databricks-glm-5-3-flash"), "glm-5-3-flash");
        assert_eq!(canonical("glm-5-3-251222"), "glm-5-3");
    }

    #[test]
    fn channel_variants_stripped() {
        assert_eq!(canonical("openrouter/z-ai/glm-5.2:free"), "glm-5.2");
        assert_eq!(canonical("openrouter/~deepseek/deepseek-v4-flash-latest"), "deepseek-v4-flash");
        assert_eq!(canonical("openrouter/z-ai/glm-5.3-flash:batch"), "glm-5.3-flash");
    }

    #[test]
    fn alias_table() {
        // omp 记录的推广短名 → v4.1-flash
        assert_eq!(canonical("deepseek-flash"), "deepseek-v4.1-flash");
        // deepseek-v4-flash 是独立档位,不得并入 v4.1
        assert_eq!(canonical("deepseek-v4-flash"), "deepseek-v4-flash");
        assert_eq!(canonical("bedrock/us-east-1/zai.glm-5"), "glm-5");
        // databricks 的连字符版 + p/点还原后同组
        assert_eq!(canonical("databricks/databricks-glm-5-3-flash"), "glm-5-3-flash");
        assert_eq!(canonical("glm-5-3-251222"), "glm-5-3");
    }

    #[test]
    fn edge_cases() {
        assert_eq!(canonical(""), "unknown");
        assert_eq!(canonical("  "), "unknown");
        assert_eq!(canonical("big-pickle"), "big-pickle");
        assert_eq!(canonical("deepseek-v4.1-flash"), "deepseek-v4.1-flash");
    }
}
