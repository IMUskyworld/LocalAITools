//! 长期记忆文档（memory.md）。
//!
//! 设计要点（与 ADR「记忆文档」一致）：
//! - 单一 markdown 文件是唯一事实源：用户可以打开、编辑、删除，模型记错了能改
//! - 注入时按字符上限截断（默认 3000 字符 ≈ 1500 token），避免每轮上下文被记忆挤爆
//! - 追加时去重（归一化比较 + 包含判断），避免同一事实反复堆积
//! - 写入原子化（临时文件 + rename），崩溃时不至于写坏半个文件
//! - 路径默认 `%APPDATA%/LocalMind/memory.md`，与 auth.json / localmind.db 同目录，
//!   卸载时由 installer 的清理逻辑一并删除
//!
//! 注意：本模块的测试一律使用临时路径，绝不触碰真实的用户记忆文件
//! （历史上 cargo test 覆盖过真实 auth.json，这里不再犯）。

use std::fs;
use std::path::{Path, PathBuf};

/// 注入上下文时的字符上限（中文按 1 字符 ≈ 0.5 token 估算，约 1500 token）
pub const INJECT_CHAR_LIMIT: usize = 3000;
/// 超过这个长度就建议触发一次「整理」（由前端调用模型压缩）
pub const COMPACT_CHAR_THRESHOLD: usize = 4000;
/// 记忆条目所在的小节标题
pub const SECTION_HEADING: &str = "## 记忆条目";
/// 单条事实的最大长度，防止模型写进来一大段
const MAX_FACT_CHARS: usize = 120;

const TEMPLATE: &str = "# LocalMind 长期记忆\n\n\
> 本文件由 LocalMind 在每个会话结束后自动维护：只沉淀跨会话仍然有用的稳定事实\n\
> （用户偏好、长期项目、环境信息等）。可以手动编辑或删除条目。\n\
> 卸载软件时会随配置一起删除。\n\n\
## 记忆条目\n";

/// 默认记忆文档路径（与 auth.json 同目录）。
pub fn default_memory_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("LocalMind").join("memory.md")
}

/// 确保文件存在（不存在才创建，绝不覆盖已有内容 —— 升级安装不会丢记忆）。
pub fn ensure_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建记忆目录失败: {e}"))?;
    }
    write_atomic(path, TEMPLATE)
}

/// 原子写入：先写临时文件，再 rename 覆盖。
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建记忆目录失败: {e}"))?;
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, content).map_err(|e| format!("写入记忆临时文件失败: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换记忆文档失败: {e}")
    })
}

/// 读取完整内容（不存在则先创建模板）。
pub fn read_raw(path: &Path) -> Result<String, String> {
    ensure_file(path)?;
    fs::read_to_string(path).map_err(|e| format!("读取记忆文档失败: {e}"))
}

/// 读取用于注入上下文的内容：超长时截断并加提示。
pub fn read_for_injection(path: &Path) -> Result<String, String> {
    let content = read_raw(path)?;
    Ok(truncate_for_injection(&content, INJECT_CHAR_LIMIT))
}

fn truncate_for_injection(content: &str, limit: usize) -> String {
    if content.chars().count() <= limit {
        return content.to_string();
    }
    let head: String = content.chars().take(limit).collect();
    format!("{head}\n\n（记忆文档较长，此处已截断；完整内容见设置页「记忆管理 → 记忆文档」）")
}

/// 保存用户手改的内容（覆盖前留一份 .bak，便于回退）。
pub fn save_user_edit(path: &Path, content: &str) -> Result<usize, String> {
    if path.exists() {
        if let Ok(previous) = fs::read_to_string(path) {
            if !previous.trim().is_empty() {
                let _ = fs::write(path.with_extension("md.bak"), previous);
            }
        }
    }
    write_atomic(path, content)?;
    Ok(content.chars().count())
}

/// 归一化：去掉空白与句末标点，便于比较是否重复。
fn normalize_key(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .filter(|c| !matches!(c, '。' | '，' | ',' | '.' | '！' | '!' | '；' | ';' | '：'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn normalize_fact(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches(['-', '*', ' ']).trim();
    let collapsed: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(MAX_FACT_CHARS).collect()
}

/// 最长公共后缀长度（按字符计）
fn common_suffix_len(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let mut n = 0;
    while n < av.len() && n < bv.len() && av[av.len() - 1 - n] == bv[bv.len() - 1 - n] {
        n += 1;
    }
    n
}

fn is_same(a: &str, b: &str) -> bool {
    let (ka, kb) = (normalize_key(a), normalize_key(b));
    if ka.is_empty() || kb.is_empty() {
        return false;
    }
    if ka == kb {
        return true;
    }
    // 新事实被已有条目完全包含（且不是极短的碎片）时视为重复，避免"用户的项目在 X"与
    // "用户的项目位于 X"这类近义条目反复堆积；反过来（新事实更具体）则保留。
    if ka.chars().count() >= 8 && kb.contains(&ka) {
        return true;
    }
    // 近义改写：共享很长的结尾（通常是路径、专有名词）且占较短一条的多数时视为同一条，
    // 例如「用户的项目在 C:\code\X」与「用户的项目位于 C:\code\X」。
    let shorter = ka.chars().count().min(kb.chars().count());
    let suffix = common_suffix_len(&ka, &kb);
    suffix >= 12 && suffix * 10 >= shorter * 6
}

/// 已存在的条目（所有 markdown 列表行）
fn collect_entries(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            t.strip_prefix("- ").map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn insert_facts(content: &str, facts: &[String]) -> String {
    let bullets: Vec<String> = facts.iter().map(|f| format!("- {f}")).collect();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    let section_idx = lines.iter().position(|l| l.trim() == SECTION_HEADING);
    let Some(start) = section_idx else {
        let mut out = content.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(SECTION_HEADING);
        out.push('\n');
        out.push_str(&bullets.join("\n"));
        out.push('\n');
        return out;
    };

    // 小节内已有一行内容？没有就先补一个空行，保持 markdown 可读
    let mut end = lines.len();
    for i in (start + 1)..lines.len() {
        if lines[i].trim_start().starts_with("## ") {
            end = i;
            break;
        }
    }
    // 插入点：小节末尾（去掉尾部空行）
    let mut insert_at = end;
    while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() {
        insert_at -= 1;
    }
    let mut idx = insert_at;
    for bullet in &bullets {
        lines.insert(idx, bullet.clone());
        idx += 1;
    }
    // 结束前补一个空行，避免与下一个小节粘连
    if end < lines.len() && (idx >= lines.len() || !lines[idx].trim().is_empty()) {
        lines.insert(idx, String::new());
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 追加事实（自动去重）。返回真正新增的条数。
pub fn append_facts(path: &Path, facts: &[String]) -> Result<usize, String> {
    let content = read_raw(path)?;
    let existing = collect_entries(&content);
    let mut accepted: Vec<String> = Vec::new();
    for raw in facts {
        let fact = normalize_fact(raw);
        if fact.is_empty() {
            continue;
        }
        if existing.iter().any(|e| is_same(&fact, e)) {
            continue;
        }
        if accepted.iter().any(|a| is_same(&fact, a)) {
            continue;
        }
        accepted.push(fact);
    }
    if accepted.is_empty() {
        return Ok(0);
    }
    let updated = insert_facts(&content, &accepted);
    write_atomic(path, &updated)?;
    Ok(accepted.len())
}

pub fn char_count(content: &str) -> usize {
    content.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn creates_template_once_and_keeps_existing_content() {
        let dir = temp_dir();
        let path = dir.path().join("memory.md");
        let first = read_raw(&path).unwrap();
        assert!(first.contains(SECTION_HEADING));
        // 已有内容不会被 ensure_file 覆盖
        write_atomic(&path, "# 用户自己改过的\n- 保留我\n").unwrap();
        let second = read_raw(&path).unwrap();
        assert!(second.contains("保留我"));
    }

    #[test]
    fn append_adds_facts_and_dedupes() {
        let dir = temp_dir();
        let path = dir.path().join("memory.md");
        let added = append_facts(
            &path,
            &[
                "用户的项目位于 C:\\code\\LocalAITools-main".to_string(),
                "用户偏好中文交流".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(added, 2);

        // 完全相同 + 近义包含 → 都算重复
        let again = append_facts(
            &path,
            &[
                "用户的项目位于 C:\\code\\LocalAITools-main".to_string(),
                "用户的项目在 C:\\code\\LocalAITools-main".to_string(),
                "   ".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(again, 0);

        let content = read_raw(&path).unwrap();
        assert_eq!(collect_entries(&content).len(), 2);

        // 新事实会被追加进「记忆条目」小节
        let more = append_facts(&path, &["用户使用 DeepSeek 模型".to_string()]).unwrap();
        assert_eq!(more, 1);
        let content = read_raw(&path).unwrap();
        assert_eq!(collect_entries(&content).len(), 3);
    }

    #[test]
    fn injection_truncates_long_document() {
        let dir = temp_dir();
        let path = dir.path().join("memory.md");
        let long = format!("{}{}", TEMPLATE, "x".repeat(INJECT_CHAR_LIMIT * 2));
        write_atomic(&path, &long).unwrap();
        let injected = read_for_injection(&path).unwrap();
        assert!(injected.chars().count() <= INJECT_CHAR_LIMIT + 80);
        assert!(injected.contains("已截断"));
    }

    #[test]
    fn save_user_edit_keeps_backup() {
        let dir = temp_dir();
        let path = dir.path().join("memory.md");
        write_atomic(&path, &format!("{TEMPLATE}- 旧条目\n")).unwrap();
        save_user_edit(&path, "# 新内容\n- 新条目\n").unwrap();
        assert!(read_raw(&path).unwrap().contains("新条目"));
        let backup = fs::read_to_string(path.with_extension("md.bak")).unwrap();
        assert!(backup.contains("旧条目"));
    }
}
