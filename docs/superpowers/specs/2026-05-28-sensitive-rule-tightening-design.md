# 敏感内容识别规则收敛设计

> 日期：2026-05-28
> 范围：`src-tauri/src/clipboard/settings.rs::looks_like_secret_token`
> 目标：降低内置敏感内容识别的误报率，避免把 git commit hash、UUID 等常见标识误判为 secret 后被静默跳过

---

## 1. 背景

当前 `looks_like_secret_token` 的判定规则：

- 长度 ≥ 16
- 无空白字符
- 字符集 ⊆ `[A-Za-z0-9_\-=]`
- 字母 ≥ 8
- 数字 ≥ 4

**误报实例**：

- git commit hash（40 个十六进制字符）：长度 ≥16 ✓，无空白 ✓，字符集 ✓，字母 ≥8 ✓，数字 ≥4 ✓ → 命中
- UUID（如 `550e8400-e29b-41d4-a716-446655440000`）：同样全部命中
- SHA-256 hex（64 字符）：同样命中

一旦用户开启敏感过滤，复制以上内容会被静默跳过，且无法补救（监听跳过事件只提示 "疑似敏感"）。

## 2. 设计原则

- **宁可漏报，不要误报**：项目已提供 `custom_secret_patterns`，用户可自定义正则补足漏报；误报无法补救。
- **保留独立 JWT 检测**：`is_jwt_like` 判定精准（三段 `.` 分隔 + `eyJ` 前缀），独立逻辑保留不变。
- **改动最小**：只重写 `looks_like_secret_token`，不调整调用层与 `content_skip_reason` 接口。

## 3. 收敛后的算法

判定 `looks_like_secret_token(value: &str) -> bool` 为真，当且仅当全部满足：

1. 长度 ≥ 20
2. 无空白字符
3. 字符集 ⊆ `[A-Za-z0-9_\-=]`
4. 大写字母数量 ≥ 2 **且** 小写字母数量 ≥ 2 **且** 数字数量 ≥ 2
5. **不是**纯 hex（即至少存在一个字符不属于 `[0-9a-fA-F]`）
6. **不是** UUID 格式（不匹配 `^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`）

调用顺序：先做廉价检查（长度、空白、字符集），再做计数与排除，UUID 正则放最后。

**与上层 `is_password_like_text` 的关系**：当前 `is_password_like_text` 顶层有 `len < 16` 早期短路，新阈值 ≥20 放在 `looks_like_secret_token` 内部生效；顶层短路保持 `< 16` 不变（不影响正确性，作为廉价前置过滤）。`is_jwt_like` 路径独立，新规则不改变其行为。

## 4. 辅助函数

新增两个私有函数：

```rust
fn is_pure_hex(value: &str) -> bool;
fn is_uuid_format(value: &str) -> bool;
```

`is_uuid_format` 使用 `regex::Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")`。考虑到该 regex 在每次调用都编译开销大，使用 `std::sync::OnceLock<Regex>` 缓存编译结果。

## 5. 命中 / 漏报 / 误报矩阵

| 输入示例 | 旧规则 | 新规则 | 期望 | 说明 |
|---|---|---|---|---|
| git commit hash 40 hex | 命中 ❌ | 不命中 ✅ | 不命中 | 纯 hex 排除 |
| SHA-256 64 hex | 命中 ❌ | 不命中 ✅ | 不命中 | 纯 hex 排除 |
| UUID 36 字符 | 命中 ❌ | 不命中 ✅ | 不命中 | UUID 格式排除 |
| GitHub PAT `ghp_` + 36 混合 | 命中 ✅ | 命中 ✅ | 命中 | 长度 ≥20、大小写数字各 ≥2 |
| OpenAI `sk-` + 48 混合 | 命中 ✅ | 命中 ✅ | 命中 | 同上 |
| Stripe `sk_test_` + 24 混合 | 命中 ✅ | 命中 ✅ | 命中 | 同上 |
| AWS AKIA + 16 大写数字 | 命中 ✅ | **漏报 ⚠️** | 漏报可接受 | 由用户自定义正则补足 |
| JWT `eyJ...` | 命中（via `is_jwt_like`） | 命中（via `is_jwt_like`） | 命中 | 独立路径不变 |
| 长度 19 全混合 | 命中 ❌ | 不命中 ✅ | 不命中 | 长度阈值收紧 |
| 含空白 | 不命中 ✅ | 不命中 ✅ | 不命中 | 已存在 |

漏报项 `AKIA` 由 `custom_secret_patterns` 接管，文档同步给出推荐正则 `^AKIA[0-9A-Z]{16}$`。

## 6. 测试用例

补充到 `src-tauri/src/clipboard/settings.rs` 的 `#[cfg(test)]` 子模块（settings.rs 当前没有内置 mod tests，会同时新建该 mod）：

**阳性（应判为 secret）**：

1. `is_password_like_text("ghp_1234abCDEFghIJKL5678mnopQRstUVwxyz9012")` → true（GitHub PAT 形式）
2. `is_password_like_text("sk_test_4eC39HqLyjWDarjtT1zdp7dc")` → true（Stripe 形式）
3. `is_password_like_text("AbCd12EfGh34IjKl56MnOp")` → true（22 字符，大小写数字混合）
4. `is_password_like_text("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c")` → true（JWT，走 `is_jwt_like` 路径，不进 `looks_like_secret_token`）

**阴性（不应判为 secret）**：

5. `is_password_like_text("73778f4abc123def4567890abcdef1234567890ab")` → false（40 字符纯 hex / git hash）
6. `is_password_like_text("550e8400-e29b-41d4-a716-446655440000")` → false（UUID）
7. `is_password_like_text("AKIAIOSFODNN7EXAMPLE")` → false（全大写 + 数字，缺小写；已知漏报）
8. `is_password_like_text("abcDEF12")` → false（长度 8 < 20）
9. `is_password_like_text("aaaaaaaaaaaaaaaaaaaa")` → false（长度 20 但无大小写数字混合）
10. `is_password_like_text("hello world 12345 ABC")` → false（含空白）
11. `is_password_like_text("ABCDEF0123456789ABCDEF0123456789")` → false（纯大写 hex）

**边界**：

12. `is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6Gg")` → true（恰好 20 字符且大小写数字各 ≥2）
13. `is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6G")` → false（19 字符差 1）

## 7. 改动清单

- `src-tauri/src/clipboard/settings.rs`
  - 重写 `looks_like_secret_token`
  - 新增 `is_pure_hex`、`is_uuid_format` 私有函数
  - 新增 `OnceLock<Regex>` 缓存 UUID 正则
  - 新增 `#[cfg(test)] mod tests` 含上述 13 个测试用例
- `docs/clipboard-toolbox-design.md`
  - 第 11 节"隐私与安全"：更新内置敏感规则描述（提高长度阈值、要求大小写数字混合、显式排除 UUID/hex）
  - 第 14.3 节"长文本处理"或新增 14.4 节：给出 AKIA 漏报的自定义正则示例
- `docs/2026-05-28-clipboard-toolbox-audit.md`
  - 将 P0 第 4 项标记为已完成

## 8. 非目标

- 不引入熵阈值或机器学习
- 不修改 `is_jwt_like` 逻辑
- 不修改 `content_skip_reason` 调用链
- 不调整 `custom_secret_patterns` 行为

## 9. 验证

- `cd src-tauri; cargo test clipboard::settings`
- `cd src-tauri; cargo check`
- `pnpm build`
- `git diff --check`
