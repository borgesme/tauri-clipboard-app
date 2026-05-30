# 剩余 P1 收尾设计：CSP 加固 / 设置面板混合保存 / search FTS5

> 设计日期：2026-05-30
> 审查项：`docs/2026-05-28-clipboard-toolbox-audit.md` P1 #10、#11、#12
> 范围：`src-tauri/tauri.conf.json`、`src-tauri/src/clipboard/repository.rs`、`src/components/clipboard/DesktopSettingsPanel.tsx` 及对应测试

## 1. 背景与问题

审计 P1 中 #6/#7/#8/#9 已收口，剩余三项：

- **#10 CSP 为 null**（`tauri.conf.json:21`）：`security.csp: null`，WebView 无任何内容来源约束，缺少 XSS 基础防护。
- **#11 设置面板全量保存**（`DesktopSettingsPanel.tsx`）：除 `StorageDirRow` 外，所有字段 `onChange` 立即调 `onSettingsChange` → `updateSettings`（`useClipboardWorkspace.ts:236`）→ 后端 `update_desktop_settings`，其中 `run_retention()` 每次都扫表清理（`service.rs:231`）。逐字输入正则、连续调数字都会触发多次全量后端往返 + 保留策略执行。
- **#12 search 全表扫描**（`repository.rs:135`）：`content LIKE %x% OR preview LIKE %x%`，无索引，数据量上千后线性扫描卡顿。

**#13（Mutex→RwLock）本轮不做**，理由见 §2 非目标。

## 2. 设计目标与非目标

### 目标

- 为 WebView 配置收敛 CSP，开发/生产分别用 `devCsp`/`csp`，两种模式均不白屏。
- 设置面板改为**混合保存**：布尔开关即时生效，数字/文本字段草稿+显式保存。
- search 引入 FTS5（`trigram` 分词器）支持中文子串索引；短查询保留 `LIKE` 回退，功能不退化。

### 非目标

- **#13 Mutex→RwLock 不做**：`rusqlite::Connection` 非 `Sync`，`settings_conn`/`items_conn` 必须保持 `Mutex`；仅 `monitor_enabled`/`database_path` 两个纯数据字段可改 RwLock，而监听线程 800ms 才读一次 bool，锁竞争近乎为零，收益不抵改动风险。
- **2 字符及以下查询不优化**：trigram 以 3 字符为索引单位，`<3` 字符（含单字、2 字中文词如「天气」）回退 `LIKE` 全表扫描，仅保证功能正确，不提升性能。详见 §5.4。
- 不引入 FTS5 相关性排序（bm25），搜索结果仍按 `last_copied_at DESC, id DESC` 排序，与现状一致。
- 不动存储目录交互（`StorageDirRow` 已是草稿模式）；不触碰后端 `update_desktop_settings` 的 `run_retention` 逻辑（属后端优化，超出 #11 前端交互范围）。
- 不做 P2 项（回收站 UI、目录迁移、窗口尺寸约束等）。

## 3. #10 — CSP 加固

### 3.1 Tauri 2 CSP 机制（查证结论）

- Tauri 在编译时对打包后的脚本/样式自动注入 nonce 与 hash 收紧 CSP，应用只需声明自身独有的来源。
- `app.security.csp`：注入到生产构建所有 HTML；若未设 `devCsp`，开发模式也用它。
- `app.security.devCsp`：仅开发模式注入，覆盖 `csp`。用于放行 `tauri dev` 下 Vite devServer 的 localhost 与 WebSocket HMR。**要求 tauri ≥ 2.4**——本项目锁定 `tauri 2.11.2`（`Cargo.lock:3846`），满足。

### 3.2 配置草案

`tauri.conf.json` 的 `app.security`：

```json
"security": {
  "csp": "default-src 'self'; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost",
  "devCsp": "default-src 'self'; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost ws://localhost:1420 http://localhost:1420"
}
```

- `style-src 'unsafe-inline'`：radix-ui 定位、Tailwind v4 运行时会写内联样式，省略会导致样式失效。
- `devCsp` 的 `connect-src` 额外放行 `ws://localhost:1420`（Vite HMR）与 `http://localhost:1420`（模块加载）。
- `ipc:`/`http://ipc.localhost`、`asset:`/`http://asset.localhost`：Tauri IPC 与 asset 协议。

### 3.3 实现流程（白屏风险，必须实测）

CSP 设错会整页白屏，纸面值不可靠。实现按「先宽后严」：

1. 先用 §3.2 草案落地。
2. `pnpm tauri dev` 启动，确认页面渲染、HMR 生效、设置面板/搜索/复制功能正常；打开 WebView 控制台核对**无 CSP 违规报错**。
3. `pnpm tauri build` 产物安装运行，确认生产模式不白屏、功能正常。
4. 若控制台报某来源被拦，按报错精确追加最小来源；若某来源实测不需要，则删除收紧。
5. 最终 `csp` 不得包含 `localhost`/`ws:` 等仅 dev 需要的来源。

## 4. #11 — 设置面板混合保存

### 4.1 字段分类

| 字段 | 类型 | 保存方式 |
|----|----|----|
| `monitorEnabled` / `ignorePasswordLikeText` / `autostartEnabled` | 布尔 | **即时**：`onChange` 立即 `onSettingsChange` |
| `retentionDays` / `maxRecordCount` / `maxTextLength` | 数字 | **草稿+保存** |
| `customSecretPatterns` | 文本 | **草稿+保存** |
| `storageDir` | 文本 | 维持现状（`StorageDirRow` 独立草稿+校验+保存） |

### 4.2 实现

- `SwitchRow`（3 个布尔）不变，保持即时提交。
- 新增一个「高级设置」草稿区，承载 `retentionDays`/`maxRecordCount`/`maxTextLength`/`customSecretPatterns` 四个字段：
  - 用单个 `useState` 持有草稿对象，初值取自 `settings`。
  - `useEffect` 依赖这四个 `settings` 字段，外部变化时重置草稿（复用 `StorageDirRow:116` 的同步模式）。
  - 一个「保存」按钮，`hasChanged`（草稿与 `settings` 对应字段有差异）时启用；点击调 `onSettingsChange({ ...settings, ...draft })` 一次性提交。
  - 数字仍经 `normalizeNumber(min)` 规整。
- `RetentionRow`/`LimitRow`/`PatternRow` 由「即时 onChange」改为「写草稿」。

### 4.3 数据流正确性

布尔即时提交会更新 `settings` 并经 `useClipboardWorkspace` 回流，但草稿 `useEffect` 仅依赖四个数字/文本字段，布尔变化不会重置草稿，未保存的草稿不丢失。`isBusy` 期间禁用输入与按钮（沿用现有 `disabled={isBusy}`）。

### 4.4 测试（Vitest + testing-library）

新增 `src/components/clipboard/DesktopSettingsPanel.test.tsx`：

1. 修改数字输入后**未**触发 `onSettingsChange`（草稿暂存）。
2. 修改后点「保存」→ `onSettingsChange` 被调用且 payload 含草稿值。
3. 拨布尔开关 → `onSettingsChange` **立即**被调用。
4. 修改正则 textarea 后未提交，点保存才提交。

## 5. #12 — search 改 FTS5 + trigram

### 5.1 FTS5 外部内容表

索引建在 `migrate_schema`（schema 演进统一入口），用 `sqlite_master` 探测是否已存在以避免重复 rebuild：

```sql
CREATE VIRTUAL TABLE clipboard_fts USING fts5(
    content, preview,
    content='clipboard_items',
    content_rowid='id',
    tokenize='trigram'
);
```

外部内容表（`content=`/`content_rowid=`）不重复存正文，仅存索引。

### 5.2 同步触发器

外部内容表需手动维护索引：

```sql
CREATE TRIGGER clipboard_fts_ai AFTER INSERT ON clipboard_items BEGIN
  INSERT INTO clipboard_fts(rowid, content, preview) VALUES (new.id, new.content, new.preview);
END;
CREATE TRIGGER clipboard_fts_ad AFTER DELETE ON clipboard_items BEGIN
  INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview) VALUES('delete', old.id, old.content, old.preview);
END;
CREATE TRIGGER clipboard_fts_au AFTER UPDATE ON clipboard_items
WHEN old.content IS NOT new.content OR old.preview IS NOT new.preview
BEGIN
  INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview) VALUES('delete', old.id, old.content, old.preview);
  INSERT INTO clipboard_fts(rowid, content, preview) VALUES (new.id, new.content, new.preview);
END;
```

`au` 的 `WHEN` 仅在正文变化时重写 FTS。本应用正文不可变——去重命中只改 `last_copied_at`/`copy_count`（`repository.rs:241`），软删除只改 `deleted_at`——故复制计数更新与软删除都不触发 FTS 重写，避免高频去重路径的无谓索引操作。软删除行因此**仍留在 FTS 中**，查询时由 §5.3 的 JOIN 过滤 `deleted_at IS NULL` 排除。

### 5.3 迁移逻辑（`migrate_schema`）

```rust
let has_fts: bool = connection
    .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='clipboard_fts'")?
    .exists([])?;
if !has_fts {
    // 1. CREATE VIRTUAL TABLE clipboard_fts ...
    // 2. CREATE TRIGGER clipboard_fts_ai / _ad / _au ...
    // 3. 灌存量：INSERT INTO clipboard_fts(clipboard_fts) VALUES('rebuild');
}
```

`rebuild` 从内容表 `clipboard_items` 的 `content`/`preview` 重建全部索引，仅首次迁移执行一次；此后新写入靠触发器增量维护。放 `migrate_schema` 而非 `init_schema`：生产启动顺序为 `init_schema` → `migrate_schema`（`service.rs:37-43`），与 `local_date` 列的演进方式保持一致。

### 5.4 `search_items` 改造

```rust
pub fn search_items(connection: &Connection, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.chars().count() >= 3 {
        search_via_fts(connection, trimmed)   // FTS5 trigram 路径
    } else {
        search_via_like(connection, trimmed)  // 原 LIKE 回退（保留现逻辑）
    }
}
```

FTS 路径（JOIN 主表过滤软删除，排序不变）：

```sql
SELECT i.id, i.content_type, i.content, i.preview, i.content_hash, i.created_at, i.last_copied_at, i.copy_count
FROM clipboard_fts f
JOIN clipboard_items i ON i.id = f.rowid
WHERE f.clipboard_fts MATCH ?1 AND i.deleted_at IS NULL
ORDER BY i.last_copied_at DESC, i.id DESC
```

MATCH 查询词转义：把用户输入包成 FTS5 字符串字面量，内部双引号双写，避免输入被解析为 FTS 语法/操作符：

```rust
let fts_query = format!("\"{}\"", trimmed.replace('"', "\"\""));
```

trigram 默认 `case_sensitive 0`，大小写不敏感，与现 `LIKE` 行为一致。

### 5.5 测试（`repository_tests.rs`）

现有 `searches_active_content_across_dates`、`search_ignores_deleted_items_and_blank_keywords`、`cleanup_items_*`（用 `search_items` 断言）仅调 `init_schema`，需补 `migrate_schema(&conn)`（建立 FTS 表，亦更贴近生产启动顺序）。

新增用例：

1. `search_matches_chinese_substring_via_fts`：插入「今天天气很好」，`migrate` 后 `search("天气很")`（3 字）命中。
2. `search_short_keyword_falls_back_to_like`：插入含「天气」的文本，`search("天气")`（2 字）经 LIKE 回退命中。
3. `search_excludes_soft_deleted_via_fts`：≥3 字查询命中后软删除，再查不返回该行。
4. `search_is_case_insensitive_via_fts`：插入「AlphaCode」，`search("alphac")` 命中。

## 6. 改动文件清单

- `src-tauri/tauri.conf.json`：`security.csp` + `security.devCsp`
- `src-tauri/src/clipboard/repository.rs`：`migrate_schema` 增 FTS 建表/触发器/rebuild；`search_items` 拆 FTS / LIKE 双路径 + 转义辅助
- `src-tauri/src/clipboard/repository_tests.rs`：补 `migrate_schema` 调用；新增 4 个 FTS 用例
- `src/components/clipboard/DesktopSettingsPanel.tsx`：数字/文本字段改草稿+保存按钮，布尔保持即时
- `src/components/clipboard/DesktopSettingsPanel.test.tsx`：新增组件测试
- `docs/2026-05-28-clipboard-toolbox-audit.md`：标记 #10、#11、#12 已修复

## 7. 验证

- `cd src-tauri; cargo test clipboard`：全通过（含新增 FTS 用例）
- `cd src-tauri; cargo check`：无新警告
- `pnpm.cmd test`：前端用例通过（含设置面板组件测试）
- `pnpm.cmd build`：`tsc` + `vitest run` + `vite build` 通过
- `pnpm tauri dev` 与 `pnpm tauri build`：两种模式均不白屏，WebView 控制台无 CSP 违规；手动验证设置面板（布尔即时、数字/正则草稿+保存）、搜索（中文 3 字走 FTS、2 字回退、软删除排除）
