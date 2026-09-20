//! 构建脚本：把 `CI-144_码注册表.md` §1.2 的 `RiskLevel` 词表**派生**成 Rust 常量。
//!
//! # 为什么是构建期派生，而不是运行期读文件
//!
//! 词表必须只有**一个来源**（`commonintents/.github/CI-144_码注册表.md` §1.2），
//! 而 `reviewer.rs` 里不能再出现第二处手维护的等级清单（K-104(a) 的形态）。
//! 构建期派生把"来源"钉在编译产物上：改注册表 ⇒ 重新构建 ⇒ R005 的行为随之改变，
//! **不需要改一行 R005 代码**。
//!
//! # 它拒绝做的事
//!
//! **找不到注册表时直接失败，不静默回退到任何副本。** 回退就是"用默认值承载未知"：
//! 构建会成功，而词表可能是旧的 —— 那是比构建失败更坏的结局（本生态已记过这个形态）。
//! 同理，解析出的块结构不完整时也直接失败（见 `registry_parse`）。
//!
//! 解析器与库共用 `src/registry_parse.rs`（`include!`），因此**只有一份解析逻辑**。

include!("src/registry_parse.rs");

use std::path::{Path, PathBuf};

/// 注册表在兄弟仓库里的相对位置（自本 crate 目录向上找）。
const REGISTRY_REL: &str = "commonintents/.github/CI-144_码注册表.md";

fn main() {
    let path = resolve_registry();
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rerun-if-env-changed=CI144_REGISTRY");
    // 解析器本身变了，生成物也要重来
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/registry_parse.rs");

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "CHECKER ERROR: 注册表存在但读不了：{}: {e}\n\
             词表是 R005 的唯一来源，读不到就不能构建（不静默回退）。",
            path.display()
        )
    });

    let rows = parse(&text).unwrap_or_else(|e| {
        panic!(
            "CHECKER ERROR: {} 的 §1.2 词表块解析失败：{e}\n\
             （来源：{}）\n\
             宁可拒绝构建，也不生成一张可能是错的词表。",
            REGISTRY_REL,
            path.display()
        )
    });

    let out = std::env::var("OUT_DIR").expect("OUT_DIR 未设置（不在 cargo 构建里？）");
    let dest = Path::new(&out).join("risk_vocabulary.rs");
    std::fs::write(&dest, generate(&path, &rows))
        .unwrap_or_else(|e| panic!("CHECKER ERROR: 写不了 {}: {e}", dest.display()));

    // 构建期把词表打到日志里（白盒可审计：构建记录里就能看到这次用的是哪版词表）
    println!(
        "cargo:warning=RiskLevel 词表从 {} 派生：{}",
        path.display(),
        rows.iter()
            .map(|r| format!("{}={}", r.value, r.class))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

/// 解析顺序：`$CI144_REGISTRY` → 自本 crate 目录逐级向上找 `commonintents/.github/…`。
fn resolve_registry() -> PathBuf {
    if let Ok(p) = std::env::var("CI144_REGISTRY") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return p;
        }
        panic!(
            "CHECKER ERROR: CI144_REGISTRY={} 不是一个文件。\n\
             它指向的词表是 R005 的唯一来源，指错了就不能构建。",
            p.display()
        );
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR 未设置（不在 cargo 构建里？）");
    let mut dir: Option<&Path> = Some(Path::new(&manifest_dir));
    let mut tried: Vec<String> = Vec::new();
    while let Some(d) = dir {
        let candidate = d.join(REGISTRY_REL);
        if candidate.is_file() {
            return candidate;
        }
        tried.push(candidate.display().to_string());
        dir = d.parent();
    }
    panic!(
        "CHECKER ERROR: 找不到 CI-144 码注册表（§1.2 是 RiskLevel 的唯一来源）。\n\
         找过：\n  {}\n\
         修法：把 commonintents/.github 放在本工作区里，\n\
               或设 CI144_REGISTRY=<注册表路径>。\n\
         **不提供静默回退**：回退到副本就是「用默认值承载未知」。",
        tried.join("\n  ")
    );
}

fn generate(source: &Path, rows: &[Row]) -> String {
    let mut s = String::new();
    s.push_str("// @generated —— 由 core/build.rs 从 CI-144 码注册表 §1.2 生成。\n");
    s.push_str("// 手改这里没有用：下一次构建会覆盖它；词表要改，改注册表。\n\n");
    s.push_str("/// 生成这份词表时读到的注册表文件（绝对路径，用于审计/漂移复核）。\n");
    s.push_str(&format!(
        "pub const REGISTRY_SOURCE: &str = {};\n\n",
        rust_str(&source.display().to_string())
    ));
    s.push_str("/// 词表一行：列序与注册表 §1.2 完全一致。\n");
    s.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    s.push_str("pub struct RiskVocabRow {\n");
    s.push_str("    /// 等级值（`class = reserved` 时是前缀；silence/illegal 行为 `-`）。\n");
    s.push_str("    pub value: &'static str,\n");
    s.push_str("    /// 类别：value / unknown / silence / illegal / reserved。\n");
    s.push_str("    pub class: &'static str,\n");
    s.push_str("    /// 注册表诊断码；`-` 表示这一类不产生发现。\n");
    s.push_str("    pub code: &'static str,\n");
    s.push_str("    /// 诊断级别：警告 / 错误 / `-`。\n");
    s.push_str("    pub level: &'static str,\n");
    s.push_str("    /// 含义（给人看）。\n");
    s.push_str("    pub meaning: &'static str,\n");
    s.push_str("    /// 确定性后果（给机器看）。\n");
    s.push_str("    pub consequence: &'static str,\n");
    s.push_str("    /// 该值是否要求 R008 的「高风险细节」。\n");
    s.push_str("    pub high_risk: bool,\n");
    s.push_str("    /// 归属（谁有定义权）。\n");
    s.push_str("    pub owner: &'static str,\n");
    s.push_str("}\n\n");
    s.push_str("/// `RiskLevel` 词表（顺序同注册表；R005 的唯一判据来源）。\n");
    s.push_str("pub const RISK_VOCABULARY: &[RiskVocabRow] = &[\n");
    for r in rows {
        s.push_str("    RiskVocabRow {\n");
        s.push_str(&format!("        value: {},\n", rust_str(&r.value)));
        s.push_str(&format!("        class: {},\n", rust_str(&r.class)));
        s.push_str(&format!("        code: {},\n", rust_str(&r.code)));
        s.push_str(&format!("        level: {},\n", rust_str(&r.level)));
        s.push_str(&format!("        meaning: {},\n", rust_str(&r.meaning)));
        s.push_str(&format!(
            "        consequence: {},\n",
            rust_str(&r.consequence)
        ));
        s.push_str(&format!("        high_risk: {},\n", r.high_risk));
        s.push_str(&format!("        owner: {},\n", rust_str(&r.owner)));
        s.push_str("    },\n");
    }
    s.push_str("];\n");
    s
}
