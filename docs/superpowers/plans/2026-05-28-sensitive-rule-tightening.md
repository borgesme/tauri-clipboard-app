# 敏感规则收敛 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 收紧 `looks_like_secret_token` 的判定规则，避免把 git commit hash、UUID 等常见标识误判为 secret 后被静默跳过。

**Architecture:** 单文件改动。在 `src-tauri/src/clipboard/settings.rs` 内：(a) 重写私有函数 `looks_like_secret_token`，新增 `is_pure_hex`、`is_uuid_format` 辅助；(b) 新增 `#[cfg(test)] mod tests`，覆盖阳性、阴性、边界用例；(c) UUID 正则通过 `std::sync::OnceLock<Regex>` 缓存避免每次重编。`is_password_like_text` 顶层短路 `< 16` 保持不变（新阈值 ≥20 放在 `looks_like_secret_token` 内部）。`is_jwt_like` 路径不动。

**Tech Stack:** Rust 2021、`regex = "1"`、`std::sync::OnceLock`、`rusqlite`（不涉及）。

参考 spec：`docs/superpowers/specs/2026-05-28-sensitive-rule-tightening-design.md`

---

## File Structure

- Modify: `src-tauri/src/clipboard/settings.rs`
  - 重写 `looks_like_secret_token`
  - 新增 `is_pure_hex`、`is_uuid_format` 私有函数
  - 新增 `UUID_REGEX: OnceLock<Regex>`
  - 在文件末尾新增 `#[cfg(test)] mod tests`
- Modify: `docs/clipboard-toolbox-design.md` — 第 11 节同步规则描述
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P0 第 4 项已完成

---

## Task 1: 添加阴性测试模块，暴露旧规则误报

**Files:**
- Modify: `src-tauri/src/clipboard/settings.rs` — 文件末尾追加 `mod tests`

阴性测试先行：这些用例旧规则会判为 true（误报），新规则应判为 false。运行它们能直观看到旧规则的 bug。

- [ ] **Step 1: 在 `settings.rs` 末尾追加测试模块**

在 `settings.rs` 文件结尾追加（注意当前文件末尾在 `bool_to_setting` 之后，没有任何 `mod tests`）：

```rust
#[cfg(test)]
mod tests {
    use super::is_password_like_text;

    #[test]
    fn rejects_git_commit_hash() {
        assert!(!is_password_like_text("73778f4abc123def4567890abcdef1234567890ab"));
    }

    #[test]
    fn rejects_sha256_hex() {
        assert!(!is_password_like_text(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
    }

    #[test]
    fn rejects_uuid() {
        assert!(!is_password_like_text("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn rejects_pure_uppercase_hex() {
        assert!(!is_password_like_text("ABCDEF0123456789ABCDEF0123456789"));
    }

    #[test]
    fn rejects_short_string() {
        assert!(!is_password_like_text("abcDEF12"));
    }

    #[test]
    fn rejects_whitespace_content() {
        assert!(!is_password_like_text("hello world 12345 ABC"));
    }
}
```

- [ ] **Step 2: 运行测试观察失败**

Run: `cd src-tauri; cargo test clipboard::settings::tests`

Expected: 4 个失败 — `rejects_git_commit_hash`、`rejects_sha256_hex`、`rejects_uuid`、`rejects_pure_uppercase_hex`（旧规则把它们判为 secret，断言 `!true` 失败）。`rejects_short_string` 与 `rejects_whitespace_content` 通过（旧规则也拦截）。

- [ ] **Step 3: 不提交，进入下一 Task**

红阶段。算法重写后一并提交。

---

## Task 2: 重写 `looks_like_secret_token` 实现新算法

**Files:**
- Modify: `src-tauri/src/clipboard/settings.rs:1-7`（imports 与 OnceLock 声明）
- Modify: `src-tauri/src/clipboard/settings.rs:201-212`（`looks_like_secret_token` 与同级 helper）

- [ ] **Step 1: 引入 `OnceLock` 与 `Regex`，添加 UUID 正则常量**

在 `settings.rs` 顶部 `use` 区块（当前第 1-7 行）加入 `OnceLock`：

```rust
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::{Duration, Local};
use regex::Regex;

use super::error::ClipboardError;
use super::models::{ClipboardSkipReason, StoredSettings};
use super::repository;
```

然后在 `DEFAULT_IGNORE_PASSWORD_LIKE_TEXT` 常量之后新增：

```rust
fn uuid_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .expect("UUID regex must compile")
    })
}
```

- [ ] **Step 2: 重写 `looks_like_secret_token`，新增两个辅助函数**

定位现有 `fn looks_like_secret_token(value: &str) -> bool { ... }`（当前 `settings.rs:201-212`），整体替换为：

```rust
fn looks_like_secret_token(value: &str) -> bool {
    if value.len() < 20 {
        return false;
    }
    let allowed = value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '='))
        .count();
    if allowed != value.len() {
        return false;
    }
    let uppercase = value
        .chars()
        .filter(|char| char.is_ascii_uppercase())
        .count();
    let lowercase = value
        .chars()
        .filter(|char| char.is_ascii_lowercase())
        .count();
    let digits = value.chars().filter(|char| char.is_ascii_digit()).count();
    if uppercase < 2 || lowercase < 2 || digits < 2 {
        return false;
    }
    if is_pure_hex(value) {
        return false;
    }
    if is_uuid_format(value) {
        return false;
    }
    true
}

fn is_pure_hex(value: &str) -> bool {
    value.chars().all(|char| char.is_ascii_hexdigit())
}

fn is_uuid_format(value: &str) -> bool {
    uuid_regex().is_match(value)
}
```

注意：旧函数中的"无空白字符"检查由调用方 `is_password_like_text` 顶层 `trimmed.contains(char::is_whitespace)` 已保证；新函数省略该检查避免重复。

- [ ] **Step 3: 运行 Task 1 的测试验证转绿**

Run: `cd src-tauri; cargo test clipboard::settings::tests`

Expected: 6 个测试全部通过。

- [ ] **Step 4: 运行全部 clipboard 测试确认无回归**

Run: `cd src-tauri; cargo test clipboard`

Expected: 全部通过。注意已有的 `service_tests` 中如有依赖 secretLike 跳过的测试，需要确认其输入仍命中（如果失败，调整测试输入而非新规则）。

- [ ] **Step 5: 不提交，继续添加阳性与边界测试**

---

## Task 3: 添加阳性测试与边界测试

**Files:**
- Modify: `src-tauri/src/clipboard/settings.rs` — 扩展 `mod tests`

- [ ] **Step 1: 追加阳性测试**

在 `mod tests` 已有用例之后追加：

```rust
    #[test]
    fn accepts_github_pat_form() {
        assert!(is_password_like_text(
            "ghp_1234abCDEFghIJKL5678mnopQRstUVwxyz9012"
        ));
    }

    #[test]
    fn accepts_openai_style_key() {
        assert!(is_password_like_text("sk_test_4eC39HqLyjWDarjtT1zdp7dc"));
    }

    #[test]
    fn accepts_jwt_via_jwt_path() {
        assert!(is_password_like_text(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        ));
    }

    #[test]
    fn accepts_boundary_twenty_chars_mixed() {
        assert!(is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6Gg"));
    }
```

- [ ] **Step 2: 追加边界与"已知漏报"测试**

继续追加：

```rust
    #[test]
    fn rejects_nineteen_chars_mixed() {
        assert!(!is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6G"));
    }

    #[test]
    fn rejects_twenty_chars_without_mix() {
        assert!(!is_password_like_text("aaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn rejects_aws_access_key_known_漏报() {
        // AWS Access Key ID 全大写 + 数字，缺小写字母。
        // 新规则要求大小写数字混合，此格式漏报，由 custom_secret_patterns 接管。
        assert!(!is_password_like_text("AKIAIOSFODNN7EXAMPLE"));
    }
```

注意：第三个测试函数名含中文 `漏报`。Rust 2021 允许非 ASCII 标识符，但为了避免环境差异，可改为 `rejects_aws_access_key_known_miss`。**用 ASCII 版本**：

```rust
    #[test]
    fn rejects_aws_access_key_known_miss() {
        assert!(!is_password_like_text("AKIAIOSFODNN7EXAMPLE"));
    }
```

- [ ] **Step 3: 运行测试验证全部通过**

Run: `cd src-tauri; cargo test clipboard::settings::tests`

Expected: 共 13 个测试全部通过。

- [ ] **Step 4: 不提交，进入最终验证**

---

## Task 4: 最终验证与单次提交

**Files:**
- 仅 `src-tauri/src/clipboard/settings.rs`

- [ ] **Step 1: 完整后端测试**

Run: `cd src-tauri; cargo test clipboard`

Expected: 全部通过，无回归。

- [ ] **Step 2: cargo check**

Run: `cd src-tauri; cargo check`

Expected: 编译通过，无警告（如有 unused warning，清理后再 commit）。

- [ ] **Step 3: 前端构建确认未受影响**

Run: `pnpm.cmd build`

Expected: tsc + vite 通过。

- [ ] **Step 4: `git diff --check`**

Run: `git diff --check`

Expected: 无空白错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/clipboard/settings.rs
git commit -m "$(cat <<'EOF'
fix(clipboard): 收敛敏感内容识别规则降低误报

looks_like_secret_token 改为长度 ≥20、要求大小写数字混合、显式排除 UUID 与纯 hex，避免误判 git commit hash、UUID、SHA-256 hex 等常见标识。AWS AKIA 等纯大写 access key 改为漏报，由 custom_secret_patterns 接管。新增 13 个测试覆盖阳性、阴性、边界与已知漏报。
EOF
)"
```

---

## Task 5: 同步文档

**Files:**
- Modify: `docs/clipboard-toolbox-design.md`
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md`

- [ ] **Step 1: 更新设计文档第 11 节**

打开 `docs/clipboard-toolbox-design.md`，定位到第 11 节"隐私与安全"中关于敏感过滤的描述。当前为：

> 提供可选敏感内容过滤，当前跳过疑似 JWT、API Key、长 token。

替换为：

> 提供可选敏感内容过滤，跳过疑似 JWT 以及长度 ≥20、同时包含大小写字母与数字的 token；显式排除 git commit hash、SHA-256 等纯 hex 以及 UUID 格式，避免误判。AWS AKIA 等纯大写 access key 默认漏报，可在"自定义敏感正则"中补充 `^AKIA[0-9A-Z]{16}$` 接管。

- [ ] **Step 2: 标记审查文档中的 P0 第 4 项**

打开 `docs/2026-05-28-clipboard-toolbox-audit.md`，在第 4 项"敏感内容误报率偏高"标题行末尾追加 `（✅ 2026-05-28 已修复）`。

同时更新文末"审查清单"表中第 4 行"主题"列保留原值，在末尾增加一列说明状态，或直接将该行整条用粗体标记（择一即可）。

推荐改法：在标题行末尾追加状态徽章式文字。例：

```markdown
### 4. 敏感内容误报率偏高 ✅ 2026-05-28 已修复
```

- [ ] **Step 3: 提交文档同步**

```bash
git add docs/clipboard-toolbox-design.md docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "$(cat <<'EOF'
docs(clipboard): 同步敏感规则收敛说明并标记审查项完成
EOF
)"
```

---

## Final Verification

- [ ] `cd src-tauri; cargo test clipboard::settings::tests` — 13 个新测试通过
- [ ] `cd src-tauri; cargo test clipboard` — 后端全量测试通过
- [ ] `cd src-tauri; cargo check` — 编译通过且无新警告
- [ ] `pnpm.cmd build` — 前端构建通过
- [ ] `git log --oneline -3` — 应看到 "fix(clipboard): 收敛敏感内容识别规则..." 与 "docs(clipboard): 同步..." 两个新 commit
- [ ] `git status` — 工作树清洁

## Self-Review

- 覆盖矩阵：spec 第 5 节 11 行命中/漏报/误报矩阵 → Task 1 + Task 3 测试 1-13 全部覆盖 ✓
- spec 第 6 节"测试用例"13 条 → Task 1 共 6 条 + Task 3 共 7 条 = 13 条 ✓
- spec 第 7 节"改动清单"3 个文件 → Task 2 改 settings.rs、Task 5 改两份文档 ✓
- 无 TBD / TODO / "实现细节略" 占位 ✓
- 类型一致：所有测试调用 `is_password_like_text`（pub fn），辅助函数 `is_pure_hex`、`is_uuid_format` 仅在 `looks_like_secret_token` 内部调用，签名一致 ✓
