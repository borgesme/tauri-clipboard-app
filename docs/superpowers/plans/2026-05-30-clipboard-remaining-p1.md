# 剩余 P1 收尾（CSP / 设置面板混合保存 / search FTS5）— 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 收口审计 P1 剩余三项——为 WebView 配置 CSP（#10）、设置面板改混合保存（#11）、search 改 FTS5 trigram 检索（#12）。

**Architecture:** 后端 `migrate_schema` 新建 FTS5 外部内容表（`trigram` 分词）+ 三个同步触发器并 rebuild 灌存量；`search_items` 按查询字数走 FTS（≥3）或回退 LIKE（<3）。前端设置面板布尔开关保持即时提交，数字/正则字段合并到草稿区 + 单个「保存设置」按钮。`tauri.conf.json` 用 `csp`（生产收敛）+ `devCsp`（开发放行 Vite HMR）分环境配置。

**Tech Stack:** Rust + rusqlite 0.32（bundled SQLite 3.46，FTS5+trigram）；React 19 + TypeScript、Vitest（jsdom）+ @testing-library/react；Tauri 2.11。

> **执行约定**：本计划对应的 spec（`docs/superpowers/specs/2026-05-30-clipboard-remaining-p1-design.md`）与本 plan 已在执行前合并为单个规划 commit；以下每个 Task 的代码改动各自独立提交。

---

## 文件结构

**后端（`src-tauri/`）：**
- `src/clipboard/repository.rs` — `migrate_schema` 末尾加 `ensure_fts_index`；`search_items` 拆 FTS / LIKE 双路径
- `src/clipboard/repository_tests.rs` — 3 处现有 search 测试补 `migrate_schema`；新增 5 个用例

**前端（`src/`）：**
- `components/clipboard/DesktopSettingsPanel.tsx` — `SwitchRow` 加 `aria-label`；删除 `RetentionRow`/`LimitRow`/`PatternRow`，新增 `AdvancedSettingsSection`（草稿+保存）
- `components/clipboard/DesktopSettingsPanel.test.tsx` — 新建组件测试

**配置：**
- `tauri.conf.json` — `app.security.csp` + `app.security.devCsp`

**文档：**
- `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P1 #10/#11/#12 已修复

---

## Task 1: migrate_schema 建立 FTS5 索引与同步触发器

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs:35-75`（`migrate_schema`）
- Modify: `src-tauri/src/clipboard/repository_tests.rs`（新增表存在用例）

- [ ] **Step 1: 写 FTS 表/触发器存在的失败测试**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾追加：

```rust
#[test]
fn migrate_schema_creates_fts_table_and_triggers() {
    let path = temp_database_path("fts-schema");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let has_fts: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='clipboard_fts'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(has_fts, "clipboard_fts virtual table should exist");

    let trigger_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger'
             AND name IN ('clipboard_fts_ai','clipboard_fts_ad','clipboard_fts_au')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(3, trigger_count, "three FTS sync triggers should exist");
}

#[test]
fn migrate_schema_rebuilds_fts_for_existing_rows() {
    let path = temp_database_path("fts-rebuild");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    // 在建 FTS 之前插入存量数据（绕过触发器，模拟旧库）
    conn.execute(
        "INSERT INTO clipboard_items
            (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count, local_date)
         VALUES ('text', '历史归档内容', '历史归档内容', 'h-legacy',
            '2026-05-20T08:00:00Z', '2026-05-20T08:00:00Z', 1, '2026-05-20')",
        [],
    )
    .unwrap();

    migrate_schema(&conn).unwrap();

    // rebuild 后存量行应可被 FTS MATCH 命中
    let matched: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH '\"归档内\"'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(1, matched, "rebuild should index pre-existing rows");
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cd src-tauri; cargo test --lib clipboard::repository_tests::migrate_schema_creates_fts_table_and_triggers clipboard::repository_tests::migrate_schema_rebuilds_fts_for_existing_rows`
Expected: 两个用例失败 —— `clipboard_fts` 表与触发器不存在（`has_fts` 为 false / `no such table: clipboard_fts`）。

- [ ] **Step 3: 在 migrate_schema 末尾接入 FTS 建表**

在 `src-tauri/src/clipboard/repository.rs` 的 `migrate_schema`，把结尾的：

```rust
    connection.execute(
        "DROP INDEX IF EXISTS idx_clipboard_items_created_at_active",
        [],
    )?;
    Ok(())
}
```

改为（追加 `ensure_fts_index` 调用，并新增该私有函数）：

```rust
    connection.execute(
        "DROP INDEX IF EXISTS idx_clipboard_items_created_at_active",
        [],
    )?;
    ensure_fts_index(connection)?;
    Ok(())
}

fn ensure_fts_index(connection: &Connection) -> Result<(), ClipboardError> {
    let has_fts: bool = connection
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='clipboard_fts'")?
        .exists([])?;
    if has_fts {
        return Ok(());
    }
    connection.execute_batch(
        "CREATE VIRTUAL TABLE clipboard_fts USING fts5(
            content, preview,
            content='clipboard_items',
            content_rowid='id',
            tokenize='trigram'
        );
        CREATE TRIGGER clipboard_fts_ai AFTER INSERT ON clipboard_items BEGIN
            INSERT INTO clipboard_fts(rowid, content, preview)
            VALUES (new.id, new.content, new.preview);
        END;
        CREATE TRIGGER clipboard_fts_ad AFTER DELETE ON clipboard_items BEGIN
            INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
            VALUES('delete', old.id, old.content, old.preview);
        END;
        CREATE TRIGGER clipboard_fts_au AFTER UPDATE ON clipboard_items
        WHEN old.content IS NOT new.content OR old.preview IS NOT new.preview
        BEGIN
            INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
            VALUES('delete', old.id, old.content, old.preview);
            INSERT INTO clipboard_fts(rowid, content, preview)
            VALUES (new.id, new.content, new.preview);
        END;
        INSERT INTO clipboard_fts(clipboard_fts) VALUES('rebuild');",
    )?;
    Ok(())
}
```

> `au` 的 `WHEN` 限定仅正文变化才重写 FTS：本应用去重命中只改 `last_copied_at`/`copy_count`、软删除只改 `deleted_at`，故这两类高频 UPDATE 不触发无谓索引重写；软删除行因此留在 FTS，由 search 的 JOIN 过滤排除（见 Task 2）。`exists` 探测使 `migrate_schema` 幂等——已建过则跳过 rebuild。

- [ ] **Step 4: 运行测试，确认通过**

Run: `cd src-tauri; cargo test --lib clipboard::repository_tests::migrate_schema_creates_fts_table_and_triggers clipboard::repository_tests::migrate_schema_rebuilds_fts_for_existing_rows`
Expected: 两个用例通过。

- [ ] **Step 5: 全量后端测试 + 无警告**

Run: `cd src-tauri; cargo test --lib clipboard; cargo check`
Expected: 全部通过（既有用例不受影响——search 仍走 LIKE）；`cargo check` 无新警告。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): migrate_schema 建立 FTS5 trigram 索引与同步触发器"
```

---

## Task 2: search_items 改 FTS5 检索，短词回退 LIKE

> 本任务是行为保持的重构：把 `search_items` 从单一 LIKE 改为「≥3 字走 FTS / <3 字回退 LIKE」。先补齐回归测试建立绿色基线（覆盖中文、短词、软删除、大小写），再切换实现，改造前后均须全绿。依赖 Task 1 已建的 FTS 表。

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs:127-145`（`search_items`）
- Modify: `src-tauri/src/clipboard/repository_tests.rs`（补 `migrate_schema` + 新用例）

- [ ] **Step 1: 现有 search 用例补 migrate_schema，并新增回归用例**

改造后 `search_items` 对 ≥3 字查询走 FTS，需要 FTS 表存在。给以下 3 个**现有**用例在 `init_schema(&conn).unwrap();` 之后补一行 `migrate_schema(&conn).unwrap();`：

- `searches_active_content_across_dates`（search `"alpha"`）
- `search_ignores_deleted_items_and_blank_keywords`（search `"temporary"`）
- `cleanup_items_removes_old_dates`（search `"old"`/`"new"`）

例如 `searches_active_content_across_dates` 改为：

```rust
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();
```

> 这 3 个用例已 `use` 了 `migrate_schema`（文件顶部 import 已含）。`cleanup_items_respects_max_record_count` 用 `list_items_by_date` 断言、不调 `search_items`，**无需**改动。

在 `repository_tests.rs` 末尾追加 4 个新用例：

```rust
#[test]
fn search_matches_chinese_substring() {
    let path = temp_database_path("search-cn");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "今天天气很好", "hash-cn", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    let results = search_items(&conn, "天气很").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("今天天气很好", results[0].content);
}

#[test]
fn search_short_keyword_falls_back_to_like() {
    let path = temp_database_path("search-short");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "今天天气", "hash-short", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    // 2 字查询低于 trigram 3 字下限，经 LIKE 回退仍应命中
    let results = search_items(&conn, "天气").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("今天天气", results[0].content);
}

#[test]
fn search_excludes_soft_deleted_via_fts() {
    let path = temp_database_path("search-fts-deleted");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "错误日志记录", "hash-del", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();
    assert_eq!(1, search_items(&conn, "错误日").unwrap().len());

    soft_delete_item(&conn, item.id, "2026-05-28T11:00:00Z").unwrap();
    assert!(search_items(&conn, "错误日").unwrap().is_empty());
}

#[test]
fn search_is_case_insensitive_via_fts() {
    let path = temp_database_path("search-case");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "AlphaCode", "hash-case", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    let results = search_items(&conn, "alphac").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("AlphaCode", results[0].content);
}
```

- [ ] **Step 2: 运行测试，确认绿色基线**

Run: `cd src-tauri; cargo test --lib clipboard::repository_tests`
Expected: 全部通过。此时 `search_items` 仍是 LIKE 实现，新用例验证的中文子串/短词/软删除/大小写在 LIKE 下也成立——这是重构前的安全网。

- [ ] **Step 3: 切换 search_items 为 FTS / LIKE 双路径**

在 `src-tauri/src/clipboard/repository.rs`，把现有 `search_items`（127-145）整段替换为：

```rust
pub fn search_items(
    connection: &Connection,
    keyword: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.chars().count() >= 3 {
        search_via_fts(connection, trimmed)
    } else {
        search_via_like(connection, trimmed)
    }
}

fn search_via_fts(
    connection: &Connection,
    keyword: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let fts_query = format!("\"{}\"", keyword.replace('"', "\"\""));
    let mut statement = connection.prepare(
        "SELECT i.id, i.content_type, i.content, i.preview, i.content_hash, i.created_at, i.last_copied_at, i.copy_count
         FROM clipboard_fts f
         JOIN clipboard_items i ON i.id = f.rowid
         WHERE f.clipboard_fts MATCH ?1 AND i.deleted_at IS NULL
         ORDER BY i.last_copied_at DESC, i.id DESC",
    )?;
    let rows = statement.query_map([fts_query], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

fn search_via_like(
    connection: &Connection,
    keyword: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let pattern = format!("%{keyword}%");
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND (content LIKE ?1 OR preview LIKE ?1)
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([pattern], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}
```

> `map_item` 是本文件已有私有函数，FTS 查询列顺序与之一致（id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count），直接复用。FTS 把用户输入包成 `"..."` 字面量并双写内部引号，避免被解析为 FTS 操作符。`search_via_like` 即原 LIKE 逻辑抽出，供 <3 字查询复用。

- [ ] **Step 4: 运行测试，确认通过**

Run: `cd src-tauri; cargo test --lib clipboard::repository_tests`
Expected: 全部通过（重构后行为不变，含 4 个新用例与补了 `migrate_schema` 的 3 个旧用例）。

- [ ] **Step 5: 全量后端测试 + 无警告**

Run: `cd src-tauri; cargo test --lib clipboard; cargo check`
Expected: 全通过；`cargo check` 无新警告（`search_via_like` 被 <3 字分支引用，非死代码）。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "refactor(clipboard): search_items 改 FTS5 检索，短词回退 LIKE"
```

---

## Task 3: 设置面板数字/正则改草稿+保存，布尔即时生效

**Files:**
- Create: `src/components/clipboard/DesktopSettingsPanel.test.tsx`
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx`

- [ ] **Step 1: 写组件测试**

创建 `src/components/clipboard/DesktopSettingsPanel.test.tsx`：

```tsx
// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

import { DesktopSettingsPanel } from "@/components/clipboard/DesktopSettingsPanel";
import type { DesktopSettings } from "@/types/clipboard";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/api/clipboard", () => ({
  validateStorageDir: vi.fn().mockResolvedValue(undefined),
}));

const SETTINGS: DesktopSettings = {
  autostartEnabled: false,
  monitorEnabled: true,
  retentionDays: 30,
  maxRecordCount: 1000,
  maxTextLength: 5000,
  ignorePasswordLikeText: true,
  customSecretPatterns: "",
  storageDir: "",
};

function renderPanel() {
  const onSettingsChange = vi.fn();
  render(
    <DesktopSettingsPanel
      settings={SETTINGS}
      isBusy={false}
      onSettingsChange={onSettingsChange}
      onPurgeDeletedItems={vi.fn()}
      onHideWindow={vi.fn()}
    />,
  );
  return { onSettingsChange };
}

describe("DesktopSettingsPanel 混合保存", () => {
  it("修改数字字段不立即提交（草稿）", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByRole("spinbutton", { name: "天" }), {
      target: { value: "7" },
    });
    expect(onSettingsChange).not.toHaveBeenCalled();
  });

  it("点击保存设置后提交合并草稿值", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByRole("spinbutton", { name: "天" }), {
      target: { value: "7" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    expect(onSettingsChange).toHaveBeenCalledTimes(1);
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ retentionDays: 7 }),
    );
  });

  it("修改正则不立即提交，保存后提交", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByPlaceholderText(/corp_/), {
      target: { value: "^secret_" },
    });
    expect(onSettingsChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ customSecretPatterns: "^secret_" }),
    );
  });

  it("拨布尔开关立即提交", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.click(screen.getByRole("switch", { name: "剪贴板监听" }));
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ monitorEnabled: false }),
    );
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test -- DesktopSettingsPanel`
Expected: 前 3 个用例失败 —— 当前数字/正则字段即时提交（「不立即提交」断言失败），且没有名为「保存设置」的按钮（`getByRole` 找不到）；第 4 个「布尔即时」通过（布尔本就即时）。

- [ ] **Step 3: SwitchRow 加 aria-label**

在 `src/components/clipboard/DesktopSettingsPanel.tsx` 的 `SwitchRow`，把其 `return` 中的 `<Switch .../>` 改为加上 `aria-label={label}`：

```tsx
  return (
    <ActionRow label={label} description={description}>
      <Switch
        className="settings-switch"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    </ActionRow>
  );
```

- [ ] **Step 4: SettingsForm 用 AdvancedSettingsSection 替换三个即时 Row**

把 `SettingsForm` 的 `return`（含 `<RetentionRow/>`、`<LimitRow/>`、`<PatternRow/>` 三行）替换为：

```tsx
function SettingsForm(props: SettingsFormProps) {
  return (
    <>
      <SwitchRow
        checked={props.settings.monitorEnabled}
        description="开启后自动捕获系统剪贴板文本。"
        disabled={props.isBusy}
        label="剪贴板监听"
        onChange={(monitorEnabled) => props.onSettingsChange({ ...props.settings, monitorEnabled })}
      />
      <SwitchRow
        checked={props.settings.ignorePasswordLikeText}
        description="疑似 JWT、API Key、长 token 会按敏感内容跳过。"
        disabled={props.isBusy}
        label="敏感内容过滤"
        onChange={(ignorePasswordLikeText) => props.onSettingsChange({ ...props.settings, ignorePasswordLikeText })}
      />
      <SwitchRow
        checked={props.settings.autostartEnabled}
        description="随系统启动后在后台运行。"
        disabled={props.isBusy}
        label="开机启动"
        onChange={(autostartEnabled) => props.onSettingsChange({ ...props.settings, autostartEnabled })}
      />
      <AdvancedSettingsSection {...props} />
      <StorageDirRow {...props} />
      <ActionRow label="数据维护" description="物理删除已移入回收状态的记录并压缩数据库。">
        <Button className="settings-button" disabled={props.isBusy} size="sm" variant="outline" onClick={props.onPurgeDeletedItems}>
          清理
        </Button>
      </ActionRow>
      <ActionRow label="托盘运行" description="隐藏主窗口，继续在后台监听剪贴板。">
        <Button className="settings-button" size="sm" variant="outline" onClick={props.onHideWindow}>
          隐藏
        </Button>
      </ActionRow>
    </>
  );
}
```

- [ ] **Step 5: 删除 RetentionRow / LimitRow / PatternRow，新增 AdvancedSettingsSection**

删除文件中 `RetentionRow`、`LimitRow`、`PatternRow` 三个函数定义（它们已不再被引用），在原 `RetentionRow` 位置新增：

```tsx
function AdvancedSettingsSection({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  const [draft, setDraft] = useState({
    retentionDays: settings.retentionDays,
    maxRecordCount: settings.maxRecordCount,
    maxTextLength: settings.maxTextLength,
    customSecretPatterns: settings.customSecretPatterns,
  });

  useEffect(() => {
    setDraft({
      retentionDays: settings.retentionDays,
      maxRecordCount: settings.maxRecordCount,
      maxTextLength: settings.maxTextLength,
      customSecretPatterns: settings.customSecretPatterns,
    });
  }, [
    settings.retentionDays,
    settings.maxRecordCount,
    settings.maxTextLength,
    settings.customSecretPatterns,
  ]);

  const hasChanged =
    draft.retentionDays !== settings.retentionDays ||
    draft.maxRecordCount !== settings.maxRecordCount ||
    draft.maxTextLength !== settings.maxTextLength ||
    draft.customSecretPatterns !== settings.customSecretPatterns;

  return (
    <>
      <ActionRow label="默认保留时长" description="超过期限的非固定记录自动清理。">
        <NumberInput
          min={1}
          suffix="天"
          value={draft.retentionDays}
          onChange={(retentionDays) => setDraft((current) => ({ ...current, retentionDays }))}
        />
      </ActionRow>
      <ActionRow label="记录容量" description="控制最大记录数和单条文本长度。">
        <div className="settings-inline-inputs">
          <NumberInput
            min={1}
            suffix="条"
            value={draft.maxRecordCount}
            onChange={(maxRecordCount) => setDraft((current) => ({ ...current, maxRecordCount }))}
          />
          <NumberInput
            min={1}
            suffix="字"
            value={draft.maxTextLength}
            onChange={(maxTextLength) => setDraft((current) => ({ ...current, maxTextLength }))}
          />
        </div>
      </ActionRow>
      <div className="setting vertical">
        <SettingText label="自定义敏感正则" description="每行一条正则；匹配内容会按敏感内容跳过。" />
        <textarea
          className="settings-pattern-input"
          disabled={isBusy}
          placeholder="例如 ^corp_[A-Za-z0-9]{24}$"
          value={draft.customSecretPatterns}
          onChange={(event) => setDraft((current) => ({ ...current, customSecretPatterns: event.currentTarget.value }))}
        />
      </div>
      <ActionRow label="高级设置" description="保留时长、容量与正则改动后点击保存生效。">
        <Button
          className="settings-button"
          disabled={isBusy || !hasChanged}
          size="sm"
          onClick={() => onSettingsChange({ ...settings, ...draft })}
        >
          保存设置
        </Button>
      </ActionRow>
    </>
  );
}
```

> 草稿仅依赖四个数字/文本字段，布尔开关即时提交回流不会重置草稿（`useEffect` 依赖项不含布尔字段），未保存的草稿不丢失。`NumberInput`/`ActionRow`/`SettingText`/`SwitchRow`/`StorageDirRow` 等保持不动。

- [ ] **Step 6: 运行测试，确认通过**

Run: `pnpm.cmd test -- DesktopSettingsPanel`
Expected: 4 个用例全部通过。

- [ ] **Step 7: 全量前端测试**

Run: `pnpm.cmd test`
Expected: 全部用例通过（含既有 `useClipboardWorkspace` 用例与新增组件用例）。

- [ ] **Step 8: 提交**

```bash
git add src/components/clipboard/DesktopSettingsPanel.tsx src/components/clipboard/DesktopSettingsPanel.test.tsx
git commit -m "feat(clipboard): 设置面板数字/正则改草稿+保存，布尔即时生效"
```

---

## Task 4: 配置 WebView CSP（生产收敛 + devCsp 放行 Vite）

> CSP 设错会整页白屏，无单元测试，依赖 dev/prod 两种模式手动实测。本任务先落地草案 + dev 验证；prod 构建验证在 Task 5。

**Files:**
- Modify: `src-tauri/tauri.conf.json:20-22`（`app.security`）

- [ ] **Step 1: 写入 csp 与 devCsp**

在 `src-tauri/tauri.conf.json`，把：

```json
    "security": {
      "csp": null
    }
```

替换为：

```json
    "security": {
      "csp": "default-src 'self'; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost",
      "devCsp": "default-src 'self'; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost ws://localhost:1420 http://localhost:1420"
    }
```

- [ ] **Step 2: dev 模式实测无白屏、无 CSP 违规**

Run: `pnpm.cmd tauri dev`
确认：
- 应用窗口正常渲染（不白屏），样式完整。
- 设置面板（开关/数字/正则）、搜索、复制等功能正常；HMR 改动能热更新。
- 打开 WebView 开发者控制台（右键或快捷键），**无 `Content-Security-Policy` 违规报错**。

> 若控制台报某来源被拦截：按报错精确向 `devCsp` 追加最小来源（如 React DevTools/某 ws 端口）。若 dev 下脚本被拦导致白屏，优先放宽 `devCsp` 的 `script-src`（如临时加 `'unsafe-inline'`），定位后再收紧。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat(clipboard): 配置 WebView CSP（生产收敛 + devCsp 放行 Vite）"
```

---

## Task 5: 标记审计项并全量验证

**Files:**
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md`（§10/§11/§12 标题 + 清单表格 3 行）

- [ ] **Step 1: 标记三项已修复**

把以下三个小节标题：

```markdown
### 10. CSP 配置为 null
### 11. 设置面板每个字段改动都触发全量保存
### 12. search 用 `LIKE %x%` 全表扫描
```

分别改为：

```markdown
### 10. CSP 配置为 null ✅ 2026-05-30 已修复
### 11. 设置面板每个字段改动都触发全量保存 ✅ 2026-05-30 已修复
### 12. search 用 `LIKE %x%` 全表扫描 ✅ 2026-05-30 已修复
```

把审查清单表格中三行：

```markdown
| P1 | 10 | CSP 为 null | 安全卫生 |
| P1 | 11 | 设置面板全量保存 | 体验 |
| P1 | 12 | search 无 FTS | 性能 |
```

改为：

```markdown
| P1 | 10 | CSP 为 null ✅ | 安全卫生 |
| P1 | 11 | 设置面板全量保存 ✅ | 体验 |
| P1 | 12 | search 无 FTS ✅ | 性能 |
```

- [ ] **Step 2: 后端全量测试**

Run: `cd src-tauri; cargo test`
Expected: 全部通过。

- [ ] **Step 3: 前端全量构建（含 tsc 类型检查）**

Run: `pnpm.cmd build`
Expected: `tsc` 无类型错误、`vitest run` 全通过、`vite build` 成功产出。

- [ ] **Step 4: 生产构建实测 CSP 不白屏**

Run: `pnpm.cmd tauri build`
完成后安装/运行产物（或用 `pnpm.cmd tauri build --debug` 加快），确认：
- 生产应用窗口正常渲染（不白屏），样式与功能正常。
- 若白屏：按 §3.3，对照 dev 已验证来源，向生产 `csp` 补回缺失但必要的来源（如 `style-src 'unsafe-inline'` 已含），重新构建验证。

- [ ] **Step 5: 手动冒烟核心场景**

在 `pnpm.cmd tauri dev` 下确认：
- 设置面板：拨开关立即生效；改保留天数/容量/正则后需点「保存设置」才提交；改了未保存时拨开关，草稿值不丢失。
- 搜索：中文 3 字（如「天气很」）能命中；中文 2 字（如「天气」）也能命中（LIKE 回退）；删除某条后搜索不再返回它。

- [ ] **Step 6: 提交**

```bash
git add docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "docs(clipboard): 标记审计 P1 #10/#11/#12 已修复"
```

---

## 验证清单（完成后逐项确认）

- [ ] `cd src-tauri; cargo test` 全通过（含 2 个 FTS schema 用例 + 4 个 search 用例）
- [ ] `cd src-tauri; cargo check` 无新警告
- [ ] `pnpm.cmd build` 通过（tsc + vitest + vite build）
- [ ] 设置面板 4 个组件用例通过
- [ ] `pnpm.cmd tauri dev`：不白屏、控制台无 CSP 违规、混合保存与中英文搜索正常
- [ ] `pnpm.cmd tauri build`：生产产物不白屏、功能正常
- [ ] 审计文档 P1 #10/#11/#12 已标记
