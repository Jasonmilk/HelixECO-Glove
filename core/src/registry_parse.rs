// 解析 `CI-144_码注册表.md` 的 §1.2 机器可解析块（`RiskLevel` 词表）。
//
// 本文件被**两处**包含，因此只有一份解析器：
//   - `core/build.rs`：构建期解析注册表 → 生成 `$OUT_DIR/risk_vocabulary.rs`
//   - `core/src/reviewer.rs`：测试期用同一份解析器复核"生成的常量 == 注册表现状"
// 这就是 K-104(a) 的修法：一处手维护的清单变成"一处来源 + 一处解析"。
//
// 只用 std —— 它必须在 build script 里编译。
//
// **它拒绝在不确定的输入上给结论**（沿用 `commonintents/.github/tools/check_proto_sync.py`
// 的纪律）：缺标记、列数不对、类别不全、码不合规、词表为空 —— 一律 `Err`，
// 由调用方 fail loud。**绝不返回一张空表让"全部通过"** —— 那是最坏的一种绿。
//
// 注意：本文件刻意不用 `//!` 内部文档注释 —— 它会被 `include!` 到 build.rs 的
// crate 根，内部属性在那种位置上不合法。

/// 词表块在注册表里的起始标记（该行以它开头）。
pub const BEGIN_MARKER: &str = "<!-- BEGIN CI144-RISK-VOCABULARY";
/// 词表块在注册表里的结束标记（整行等于它）。
pub const END_MARKER: &str = "<!-- END CI144-RISK-VOCABULARY -->";

/// 合法的 `class` 取值。
pub const CLASSES: [&str; 5] = ["value", "unknown", "silence", "illegal", "reserved"];

/// 词表的一行，列序与注册表 §1.2 完全一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 等级值（`class = reserved` 时它是前缀；`silence` / `illegal` 行为 `-`）。
    pub value: String,
    /// 类别：value / unknown / silence / illegal / reserved。
    pub class: String,
    /// 注册表诊断码；`-` 表示"这一类不产生发现"（当前只有 `value` 行如此）。
    pub code: String,
    /// 诊断级别：警告 / 错误 / `-`。
    pub level: String,
    /// 含义（给人看）。
    pub meaning: String,
    /// 确定性后果（给机器看：接下来会发生什么）。
    pub consequence: String,
    /// 该值是否要求 R008 的"高风险细节"。
    pub high_risk: bool,
    /// 归属（谁有定义权）。
    pub owner: String,
}

fn strip_cell(raw: &str) -> String {
    let t = raw.trim();
    let t = t.strip_prefix('`').unwrap_or(t);
    let t = t.strip_suffix('`').unwrap_or(t);
    t.trim().to_string()
}

fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// 解析注册表全文，返回 §1.2 词表的行（顺序保持注册表顺序）。
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut inside = false;
    let mut seen_begin = false;
    let mut rows: Vec<Row> = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if !inside {
            if line.trim_start().starts_with(BEGIN_MARKER) {
                seen_begin = true;
                inside = true;
                if !line.contains("-->") {
                    return Err(format!(
                        "第 {lineno} 行：BEGIN 标记未在同一行闭合（缺 `-->`）—— 拒绝解析"
                    ));
                }
            }
            continue;
        }
        if line.trim() == END_MARKER {
            inside = false;
            break;
        }
        if !line.trim_start().starts_with('|') {
            continue;
        }
        // 拆列：`|a|b|` → ["a","b"]（丢掉首尾两个空段）
        let parts: Vec<String> = line.split('|').map(|s| s.to_string()).collect();
        if parts.len() < 3 {
            return Err(format!("第 {lineno} 行：不是合法的表格行：{line}"));
        }
        let cells: Vec<String> = parts[1..parts.len() - 1]
            .iter()
            .map(|s| strip_cell(s))
            .collect();
        if is_separator_row(&cells) {
            continue; // 表头下的 `|---|---|`
        }
        if cells.len() != 8 {
            // 表头行有 8 列；列数不符即格式变了 —— 宁可拒绝，也不给一个自信的错结论
            if cells.iter().any(|c| c == "value" || c == "class") {
                continue; // 表头行
            }
            return Err(format!(
                "第 {lineno} 行：列数为 {}，应为 8（value|class|code|level|含义|确定性后果|high_risk|归属）",
                cells.len()
            ));
        }
        if cells[0] == "value" && cells[1] == "class" {
            continue; // 表头（以防分隔行缺失）
        }
        let class = cells[1].clone();
        if !CLASSES.contains(&class.as_str()) {
            return Err(format!("第 {lineno} 行：class={class:?} 不认识"));
        }
        let high_risk = match cells[6].as_str() {
            "yes" => true,
            "no" => false,
            other => {
                return Err(format!(
                    "第 {lineno} 行：high_risk={other:?} 不认识（只认 yes/no）"
                ))
            }
        };
        rows.push(Row {
            value: cells[0].clone(),
            class,
            code: cells[2].clone(),
            level: cells[3].clone(),
            meaning: cells[4].clone(),
            consequence: cells[5].clone(),
            high_risk,
            owner: cells[7].clone(),
        });
    }

    if !seen_begin {
        return Err(format!(
            "正文里找不到起始标记 `{BEGIN_MARKER}` —— 注册表块被删/被改名；拒绝解析（不返回空表）"
        ));
    }
    if inside {
        return Err(format!("找到起始标记但找不到结束标记 `{END_MARKER}`"));
    }

    validate(&rows)?;
    Ok(rows)
}

/// 结构校验：类别齐全、值非空、码合规、码不重复。
fn validate(rows: &[Row]) -> Result<(), String> {
    if rows.is_empty() {
        return Err("词表为空 —— 空表会让所有断言'全部通过'，那是最坏的一种绿".to_string());
    }
    for class in CLASSES {
        let n = rows.iter().filter(|r| r.class == class).count();
        // `value` 是词表本体，可以有多行；其余四类是**判定出口**，各恰好一行。
        if class == "value" {
            if n == 0 {
                return Err("class=value 的行数为 0 —— 词表里一个合法等级都没有".to_string());
            }
            continue;
        }
        if n != 1 {
            return Err(format!(
                "class={class:?} 的行数为 {n}，应为 1（判定出口必须恰好齐全且唯一）"
            ));
        }
    }
    let mut codes: Vec<&str> = Vec::new();
    for r in rows {
        if r.class == "value" {
            if r.code != "-" {
                return Err(format!(
                    "value 行 `{}` 的 code 应为 `-`（通过者不产生发现），实为 {:?}",
                    r.value, r.code
                ));
            }
            if r.value.is_empty() || r.value == "-" {
                return Err(format!("value 行为空：{r:?}"));
            }
        } else {
            if r.code == "-" {
                return Err(format!(
                    "class={} 的行必须有注册码（规则 3：新增码 = 先登记后使用）",
                    r.class
                ));
            }
            if !(r.code.starts_with("E-") || r.code.starts_with("W-")) {
                return Err(format!(
                    "码 {:?} 必须落在 E-*/W-* 诊断码命名空间（§0）",
                    r.code
                ));
            }
            if !matches!(r.level.as_str(), "警告" | "错误") {
                return Err(format!(
                    "码 {:?} 的 level={:?} 应为 警告/错误",
                    r.code, r.level
                ));
            }
            if codes.contains(&r.code.as_str()) {
                return Err(format!("码 {:?} 重复登记", r.code));
            }
            codes.push(&r.code);
        }
        if r.owner.trim().is_empty() || r.owner == "待定" {
            return Err(format!(
                "行 {:?} 的归属为 {:?} —— 跨仓消费的码必须有归属（§3.1 第五列）",
                r.value, r.owner
            ));
        }
    }
    // ---- 单调性：**校验的是序，不是某个具体取值** --------------------------
    //
    // 这里刻意不写"unknown 必须是警告"这种把策略钉死的断言。策略（隔离还是拒绝）
    // 属于注册表那一列；代码只校验**声明过的序**（注册表 §1.2 的不变量 1–4）：
    //
    //   可接受度(value) > 可接受度(unknown) > 可接受度(silence) ≥ 可接受度(illegal)
    //                                    ↑ 必须**严格**：否则两者被混为一谈
    //   可接受度(reserved) ≥ 可接受度(unknown)   （grease：保留段必须被容下）
    //   严重度序与之相反：rank 越大越重 ⇒ rank(silence) ≥ rank(unknown)
    fn level_rank(rows: &[Row], class: &str) -> Result<u8, String> {
        let level = rows
            .iter()
            .find(|r| r.class == class)
            .map(|r| r.level.as_str())
            .ok_or_else(|| format!("class={class} 没有行"))?;
        match level {
            "警告" => Ok(1),
            "错误" => Ok(2),
            other => Err(format!(
                "class={class} 的 level={other:?} 不认识（只认 警告/错误）"
            )),
        }
    }
    let unknown = level_rank(rows, "unknown")?;
    let silence = level_rank(rows, "silence")?;
    let illegal = level_rank(rows, "illegal")?;
    let reserved = level_rank(rows, "reserved")?;

    if silence < unknown {
        return Err(format!(
            "单调性被破坏：silence 的 level({silence}) 比 unknown({unknown}) 更轻 —— \
             §12.4 的关键一行是「空不得轻于 unknown」"
        ));
    }
    if silence == unknown {
        return Err(format!(
            "unknown 与 silence 的 level 相同（都是 {unknown}）⇒ **处置相同** —— \
             两者被混为一谈了（§12.4 + 本表不变量 3：沉默与声明必须可区分）"
        ));
    }
    if illegal < silence {
        return Err(format!(
            "单调性被破坏：illegal({illegal}) 不得比 silence({silence}) 轻 —— \
             「不认」必须至少和「没证据」一样不可接受"
        ));
    }
    if reserved > unknown {
        return Err(format!(
            "保留段({reserved}) 不得比 unknown({unknown}) 更重 —— \
             grease 探针必须被容下，不得被拒（§4 规则 1 + §5）"
        ));
    }
    // 保留段必须带 `-` 前缀形态（§4 规则 1）
    let reserved_row = rows.iter().find(|r| r.class == "reserved").unwrap();
    if !reserved_row.value.ends_with('-') {
        return Err(format!(
            "class=reserved 的 value={:?} 应为前缀（以 `-` 结尾）",
            reserved_row.value
        ));
    }
    Ok(())
}

/// 把一段文本转成 Rust 字符串字面量（含引号），供 build.rs 生成代码。
// 只在 build.rs 那一侧用到（库里由测试引用解析函数，不生成代码）。
#[allow(dead_code)]
pub fn rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
