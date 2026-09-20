//! R005 判据的**验收探针**（契约 §5 grease + §12.6③）。
//!
//! # 为什么它在 `tests/` 里，而不是在 `reviewer.rs` 的 `#[cfg(test)]` 里
//!
//! 因为验收要求"**同一份探针**在修复前是红的、修复后是绿的"。
//! 写在 `reviewer.rs` 内部的测试做不到这一点：它们随修复一起被加进来，
//! "修复前"根本没有它们。写在这里，它只依赖**修复前就已存在**的公开 API
//! （`StaticReviewer::review` + `Severity` + `RuleId`），因此可以整体搬到
//! 修复前的版本上跑，直接观察红/绿（本次已在一个 `git worktree`（HEAD）上跑过，见报告）。
//!
//! # 它断言什么（对应四条验收）
//!
//! | 探针 | 断言 | 修复前 |
//! |---|---|---|
//! | ① | 注入新值（学习器实际发出的 `UNKNOWN`）⇒ 隔离，不静默丢弃，不与沉默混为一谈 | 🔴 `Error`（被拒） |
//! | ①b | 保留段 `X-`（grease）⇒ 原样携带 + 隔离，不得判为不合法 | 🔴 `Error` |
//! | ② | （不在本仓库：见 `helix-mind/tools/hooks/` 的比值判据） | — |
//! | ③ | 注册表加值 ⇒ R005 自动接受（派生性正控） | — |
//! | ④ | 空字段 ⇒ 处置不得轻于 `unknown` | 🔴 沉默 `Warning` < 声明 `Error` |
//!
//! # 它刻意不碰的东西
//!
//! 它**不**引用 `code` / `Disposition` / `classify_risk_level` —— 那些是修复引入的 API，
//! 一旦引用就无法在修复前编译，"修复前是红的"这句话就没有证据了。
//! 判据只看 `Severity`（`Info < Warning < Error`），因为严重度是修复前后都存在的量。

use helix_eco_glove_core::reviewer::{Severity, StaticReviewer, ToolManifest};

/// 与 `reviewer.rs` 的 `valid_manifest()` 同形：除 `risk_level` 外全部合法，
/// 这样任何 R005 之外的发现都不会污染探针。
fn manifest_with(risk_level: &str) -> ToolManifest {
    ToolManifest {
        name: "macos.fs.read_file".to_string(),
        version: "1.0.0".to_string(),
        description: "Read a file from the filesystem and return its content".to_string(),
        parameters_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path to read"}
            },
            "required": ["path"]
        }),
        risk_level: risk_level.to_string(),
        platform: "macos".to_string(),
        host_os: vec!["macos".to_string()],
        tags: vec!["filesystem".to_string(), "read".to_string()],
    }
}

/// R005 对这个输入给出的严重度。
///
/// `None` = **什么都没说** —— 在"未知必须显式表示"下，这本身就是一种失败
/// （静默丢弃），所以探针里一律用 `expect` 把 `None` 变成红。
fn r005_severity(risk_level: &str) -> Option<Severity> {
    let report = StaticReviewer::new().review(&manifest_with(risk_level));
    report
        .findings
        .iter()
        .find(|f| f.rule_id == "R005")
        .map(|f| f.severity)
}

/// R005 对这个输入给出的**发现内容**（用于断言"不被混为一谈"）。
fn r005_message(risk_level: &str) -> Option<String> {
    let report = StaticReviewer::new().review(&manifest_with(risk_level));
    report
        .findings
        .iter()
        .find(|f| f.rule_id == "R005")
        .map(|f| f.message.clone())
}

/// 严重度序：**越大越重**。单调性断言用它，而不是靠"看起来更重"。
fn rank(s: Severity) -> u8 {
    match s {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

/// 同上，但把"没有发现"显式地当作最宽松的一档（0），
/// **不用默认值冒充一个严重度** —— `None` 与 `Info` 不是一回事。
fn rank_opt(s: Option<Severity>) -> u8 {
    match s {
        None => 0,
        Some(x) => rank(x),
    }
}

// ============================================================================
// ① 注入一个新值 ⇒ 隔离，不静默丢弃，不与沉默混为一谈
// ============================================================================

/// `"UNKNOWN"` 是**修复动机本身**：`Helix-MCP-Learner` 的 `RiskLevel::Unknown`
/// 序列化后就是它，并且带着 `FieldProvenance`（来源/规则/信任度）。
///
/// 修复前：`Error` ⇒ `rejected/` ⇒ **惩罚诚实声明**。
/// 修复后：`Warning` ⇒ `staging/` ⇒ **隔离待人工复核**。
///
/// `rejected/` 与 `staging/` 都保留 manifest 原文件（都不是删除）；
/// 区别是"人工复核即可放行"与"必须修复"，而探针要看的是这个区别确实存在。
#[test]
fn criterion_1_new_value_is_quarantined_not_dropped_not_conflated_with_silence() {
    let unknown =
        r005_severity("UNKNOWN").expect("① 声明 unknown 必须留下发现 —— 静默丢弃（None）是失败");
    assert_eq!(
        unknown,
        Severity::Warning,
        "① 声明 unknown 必须被判为**隔离**（Warning ⇒ staging/），而不是被拒（Error ⇒ rejected/）"
    );

    let silent = r005_severity("").expect("① 沉默也必须留下发现 —— 否则它会在没人看见的地方被放行");
    assert_ne!(
        Some(unknown),
        Some(silent),
        "① unknown 与沉默**不得混为一谈**：一个是声明，一个是没声明"
    );

    let um = r005_message("UNKNOWN").unwrap();
    let sm = r005_message("").unwrap();
    assert_ne!(um, sm, "① 两种输入的发现内容必须不同（可被人读出来）");
    assert!(
        um.contains("UNKNOWN"),
        "① 原值必须被原样保留在发现里（§4.2），实际：{um}"
    );
    assert!(
        sm.contains("unknown"),
        "① 沉默的发现应指出去显式写 unknown 才是出路，实际：{sm}"
    );
}

/// ①b **grease 探针**（契约 §5）：保留段 `X-` 的值必须**被容下**。
///
/// 契约原文：注入保留的未知码，断言每一跳**保留或忽略**它，**绝不**把它变成"通过"。
/// 修复前它被判为"不合法" ⇒ `Error` ⇒ 拒绝 —— 那正是"容不下未知"的形态，
/// 也正好让"定期暴露于未知值"的 grease 机制无法建立。
#[test]
fn criterion_1b_reserved_segment_is_carried_not_rejected() {
    let x = r005_severity("X-PROBE").expect("①b 保留码必须可被原样携带 —— 丢弃它违反注册表 §4.2");
    assert_eq!(
        x,
        Severity::Warning,
        "①b 保留段 `X-` 不得被判为不合法（§4 规则 1 + §5 grease）"
    );
    let msg = r005_message("X-PROBE").unwrap();
    assert!(
        msg.contains("X-PROBE"),
        "①b 保留码必须原样出现在发现里：{msg}"
    );
}

// ============================================================================
// ④ 空字段 ⇒ 处置不得轻于 unknown
// ============================================================================

/// 这一条是 §12.4 的**关键一行**："空不得轻于 `unknown`"。
///
/// 修复前恰好相反（沉默 `Warning`、声明 `Error`），于是**最便宜的攻击不是伪造危险值，
/// 而是删掉这个字段**：删掉 ⇒ 更轻 ⇒ 且没有出处信息可丢（`FieldProvenance` 随之消失）。
#[test]
fn criterion_4_empty_field_is_not_lighter_than_unknown() {
    let unknown = r005_severity("UNKNOWN").expect("unknown 必须有发现");
    let empty = r005_severity("").expect("④ 空字段**必须**有发现 —— 静默放行是这条要堵的攻击");
    assert!(
        rank(empty) >= rank(unknown),
        "④ 空字段({empty:?}) 不得轻于 unknown({unknown:?})：\
         不确定性上升不得导致更宽松的处置"
    );
}

/// ④b 完整阶梯：可接受度必须随不确定性单调不升。
///
/// 修复前这条阶梯在 (unknown, silence) 处**倒挂**，所以它在修复前是红的。
#[test]
fn criterion_4b_uncertainty_ladder_is_monotonic() {
    let ladder: [(&str, &str); 4] = [
        ("low", "词表内的已知等级"),
        ("UNKNOWN", "明确声明判不出"),
        ("", "沉默：字段缺失/为空"),
        ("banana", "词表外：违规"),
    ];
    let ranks: Vec<(String, u8)> = ladder
        .iter()
        .map(|(input, why)| {
            let sev = r005_severity(input);
            (format!("{input:?}（{why}）⇒ {sev:?}"), rank_opt(sev))
        })
        .collect();
    for pair in ranks.windows(2) {
        assert!(
            pair[1].1 >= pair[0].1,
            "④b 单调性被破坏：{} 之后出现了更宽松的 {} —— 完整阶梯：{ranks:?}",
            pair[0].0,
            pair[1].0
        );
    }
}

// ============================================================================
// 对照：修复不得以"全部放宽"为代价
// ============================================================================

/// 词表外的值（`"extreme"`）**必须仍然被拒**。
///
/// 这条在修复前后都应为绿：它是"我们没有为了修反向激励而把闸门拆掉"的证据，
/// 也是 `reviewer.rs` 里既有测试 `test_invalid_risk_level_fails` 的对外版本。
#[test]
fn control_out_of_registry_value_is_still_rejected() {
    assert_eq!(
        r005_severity("extreme"),
        Some(Severity::Error),
        "词表外的值必须仍被判为 Error（§12.4：不认 ⇒ 拒绝）"
    );
}

/// 合法值**不得**产生 R005 发现（否则"通过"这一档就不存在了）。
#[test]
fn control_registry_values_produce_no_r005_finding() {
    for v in ["low", "medium", "critical", "catastrophic", "LOW", "Medium"] {
        assert_eq!(
            r005_severity(v),
            None,
            "合法值 {v:?} 不该有 R005 发现（大小写不敏感是既有行为，不得回退）"
        );
    }
}
