# 桌面剪贴板工具箱设计文档

## 1. 项目定位

本项目是一个基于 Tauri + React 的本地桌面剪贴板工具箱，面向日常开发、写作、资料整理等场景，自动保存用户复制过的内容，并按日期组织展示，支持查看、删除、复制回剪贴板等基础操作。

MVP 阶段优先支持文本剪贴板，先保证采集稳定、数据可靠、操作闭环清晰；图片、文件、富文本、标签、收藏等能力作为后续扩展。

## 2. 目标与非目标

### 2.1 目标

- 自动监听系统剪贴板中的文本变化。
- 将剪贴板记录持久化到本地数据库。
- 按日期聚合展示历史剪贴内容。
- 支持查看详情、复制、删除、搜索等常用操作。
- 支持暂停/恢复监听，避免用户在敏感场景被动记录。
- 支持保留策略、本地存储目录、单条文本上限与敏感内容过滤等本地设置。
- 保持所有数据本地存储，不依赖云端服务。

### 2.2 非目标

- MVP 不做账号体系、云同步、多设备同步。
- MVP 不支持图片、文件列表、HTML 富文本等非纯文本内容。
- MVP 不做复杂标签体系、OCR、AI 分类等增强功能。
- MVP 不直接处理跨设备剪贴板或浏览器插件能力。

## 3. 用户场景

### 3.1 高频场景

- 用户复制代码片段后，可在当天记录中快速找回。
- 用户复制多个文案内容后，可按时间线回看并二次复制。
- 用户误覆盖剪贴板后，可从历史记录中恢复上一条内容。
- 用户希望删除某条敏感记录，避免长期保留。

### 3.2 边界场景

- 连续复制同一内容时，不应生成大量重复记录。
- 从本应用中点击复制历史记录时，不应再次新增一条重复记录。
- 应用重启后，历史数据仍可正常按日期加载。
- 用户暂停监听期间，剪贴板变化不应被记录。

## 4. 核心业务流程

### 4.1 自动采集流程

1. 应用启动后初始化数据库与剪贴板监听服务。
2. 监听服务按固定间隔读取系统剪贴板文本。
3. 若剪贴板为空、不是文本、与上次内容相同，则跳过。
4. 对文本内容计算 `content_hash`。
5. 若数据库中已有相同 hash 的未删除记录，则更新 `last_copied_at` 与 `copy_count`。
6. 若不存在相同记录，则写入新记录。
7. 后端向前端发送 `clipboard:item-created` 或 `clipboard:item-updated` 事件。
8. 前端收到事件后刷新当前日期列表或局部插入记录。

### 4.2 查看与复制流程

1. 用户在列表中选择某条记录。
2. 前端展示完整内容与元信息。
3. 用户点击复制按钮。
4. 前端调用后端命令，将记录内容写回系统剪贴板。
5. 后端记录本次写入的 `content_hash`，用于短时间内忽略应用自身写入造成的监听回流。

### 4.3 删除流程

1. 用户点击删除单条记录。
2. 前端弹出确认或执行轻量删除反馈。
3. 后端将记录软删除或物理删除。
4. 前端从当前列表中移除记录。
5. 若当前日期无剩余记录，日期侧栏同步更新数量。

## 5. 技术架构

```text
┌─────────────────────────────────────────────┐
│ React UI                                     │
│ - 日期侧栏                                   │
│ - 记录列表                                   │
│ - 详情面板                                   │
│ - 搜索与设置浮层                             │
└─────────────────────┬───────────────────────┘
                      │ invoke / listen
┌─────────────────────▼───────────────────────┐
│ Tauri Native Menu                            │
│ - 系统：还原 / 移动 / 恢复默认大小 / 最小化 / 最大化 / 关闭 │
│ - 设置：打开设置浮层                         │
└─────────────────────┬───────────────────────┘
                      │ menu event
┌─────────────────────▼───────────────────────┐
│ Tauri Command Layer                          │
│ - query_items_by_date                        │
│ - search_items                               │
│ - copy_item                                  │
│ - delete_item                                │
│ - set_monitor_enabled                        │
└─────────────────────┬───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│ Rust Core Services                           │
│ - ClipboardMonitor                           │
│ - ClipboardRepository                        │
│ - ClipboardService                           │
│ - AppSettings                                │
└──────────────┬──────────────────┬────────────┘
               │                  │
┌──────────────▼───────┐  ┌───────▼────────────┐
│ System Clipboard      │  │ Local SQLite DB     │
└──────────────────────┘  └────────────────────┘
```

### 5.1 前端职责

- 维护页面交互状态，例如当前日期、搜索词、选中记录、监听开关。
- 通过 Tauri `invoke` 调用后端命令。
- 监听后端事件，刷新或增量更新列表。
- 监听原生菜单发出的 `app:open-settings` 事件并打开设置浮层。
- 提供清晰的空状态、错误提示和操作反馈。

### 5.2 后端职责

- 读取与写入系统剪贴板。
- 维护监听状态与应用自身写入防回流状态。
- 负责数据持久化、查询、删除与去重。
- 向前端广播剪贴板记录变化事件。
- 管理原生菜单、系统托盘、窗口隐藏/显示策略。
- 管理设置项，例如保留天数、最大记录数、自定义存储目录、文本上限和自定义敏感规则。

## 6. 推荐目录结构

```text
src/
  App.tsx
  main.tsx
  components/
    clipboard/
      DateSidebar.tsx
      ItemListPanel.tsx
      DetailPanel.tsx
      DesktopSettingsPanel.tsx
    ui/
      button.tsx
      card.tsx
      switch.tsx
  hooks/
    useClipboardWorkspace.ts
    useClipboardEvents.ts
  api/
    clipboard.ts
  types/
    clipboard.ts

src-tauri/src/
  lib.rs
  desktop.rs
  clipboard/
    commands.rs
    monitor.rs
    service.rs
    repository.rs
    settings.rs
    models.rs
    hash.rs
    error.rs
```

目录设计以“前端视图与后端业务服务分离”为原则，避免把剪贴板监听、数据库访问和 Tauri command 混在单一文件中。

## 7. 数据模型

### 7.1 剪贴板记录

```sql
CREATE TABLE clipboard_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  content_type TEXT NOT NULL,
  content TEXT NOT NULL,
  preview TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  last_copied_at TEXT NOT NULL,
  copy_count INTEGER NOT NULL DEFAULT 1,
  deleted_at TEXT
);

CREATE UNIQUE INDEX idx_clipboard_items_hash_active
  ON clipboard_items(content_hash)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_clipboard_items_created_at_active
  ON clipboard_items(created_at)
  WHERE deleted_at IS NULL;
```

字段说明：

- `content_type`：MVP 固定为 `text`，为后续图片、文件扩展预留。
- `content`：完整剪贴板内容。
- `preview`：列表摘要，当前截断到 120 个字符。
- `content_hash`：用于去重的内容 hash。
- `created_at`：首次捕获时间，按本地时间生成日期分组。
- `last_copied_at`：最近一次检测到该内容的时间。
- `copy_count`：相同内容被复制的累计次数。
- `deleted_at`：软删除时间；若选择物理删除，可移除此字段。

### 7.2 应用设置

```sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

当前设置项：

- `monitor_enabled`：是否启用监听，会随桌面设置持久化。
- `retention_days`：历史记录保留天数。
- `max_record_count`：最大保留记录数。
- `max_text_length`：单条文本最大保存长度。
- `ignore_password_like_text`：是否忽略疑似密码/token 的内容。
- `custom_secret_patterns`：自定义敏感内容正则，每行一条，仅在敏感内容过滤开启时生效。
- `storage_dir`：自定义本地 SQLite 存储目录，空字符串表示默认应用数据目录。
- `autostart_enabled`：开机启动，由系统 autostart 插件管理。

## 8. Tauri 命令设计

### 8.1 查询命令

```ts
type ClipboardItem = {
  id: number;
  contentType: 'text';
  content: string;
  preview: string;
  createdAt: string;
  lastCopiedAt: string;
  copyCount: number;
};

type ClipboardDateGroup = {
  date: string;
  count: number;
};

type DesktopSettings = {
  autostartEnabled: boolean;
  retentionDays: number;
  maxRecordCount: number;
  maxTextLength: number;
  ignorePasswordLikeText: boolean;
  storageDir: string;
};
```

推荐命令：

- `list_clipboard_dates(): ClipboardDateGroup[]`
- `list_clipboard_items(date: string): ClipboardItem[]`
- `search_clipboard_items(keyword: string): ClipboardItem[]`
- `get_clipboard_item(id: number): ClipboardItem`

### 8.2 操作命令

- `copy_clipboard_item(id: number): void`
- `delete_clipboard_item(id: number): void`
- `clear_clipboard_items_by_date(date: string): void`
- `set_clipboard_monitor_enabled(enabled: boolean): void`
- `get_clipboard_monitor_status(): { enabled: boolean }`
- `get_desktop_settings(): DesktopSettings`
- `update_desktop_settings(settings: DesktopSettings): DesktopSettings`
- `hide_main_window(): void`
- `show_main_window(): void`

### 8.3 事件设计

- `clipboard:item-created`：新增记录后触发。
- `clipboard:item-updated`：重复记录计数更新后触发。
- `clipboard:item-deleted`：删除记录后触发。
- `clipboard:monitor-status-changed`：监听状态变化后触发。
- `app:open-settings`：原生“设置”菜单触发，前端打开设置浮层。

## 9. 前端页面设计

### 9.1 主页面布局

```text
┌──────────────────────────────────────────────────────────┐
│ Native Menu: 系统 | 设置（打开设置浮层）                    │
├───────────────┬───────────────────────┬──────────────────┤
│ 日期侧栏       │ 记录列表               │ 详情面板          │
│ 今天 12       │ 10:21 复制内容摘要...  │ 完整内容          │
│ 昨天 8        │ 10:03 复制内容摘要...  │ 复制 / 删除       │
│ 2026-05-24 3  │ ...                   │ 元信息            │
└───────────────┴───────────────────────┴──────────────────┘
```

### 9.2 交互状态

- 默认选中今天，列表按 `created_at` 倒序展示。
- 点击日期后加载该日期记录。
- 点击记录后右侧展示完整内容。
- 搜索时可以跨日期检索，搜索结果列表不受日期侧栏限制。
- 监听、开机启动、保留策略、本地存储目录、文本上限和敏感过滤集中在设置浮层。
- 删除当前选中记录后，自动选中列表下一条记录。
- 当前日期无数据时展示空状态，引导用户复制一段文本进行测试。

## 10. 去重与防回流策略

### 10.1 去重规则

- 对剪贴板文本做标准化处理后计算 hash。
- 标准化建议只处理行尾差异，不默认 trim 首尾空白，避免改变用户真实内容语义。
- 相同 hash 的内容默认合并为同一条记录，并更新出现次数。

### 10.2 防回流规则

- `copy_clipboard_item` 写入系统剪贴板时记录 `last_app_write_hash` 与写入时间。
- 监听线程读取到相同 hash 且距离应用写入时间小于 2 秒时跳过。
- 超过时间窗口后，如果用户再次主动复制同内容，可正常更新 `copy_count`。

## 11. 隐私与安全

- 所有数据默认存储在本机 Tauri 应用数据目录。
- 不上传、不同步、不调用外部网络接口。
- 提供暂停监听，用户可在输入密码、token、隐私内容前手动关闭。
- 提供可选敏感内容过滤，当前跳过疑似 JWT、API Key、长 token。
- 提供单条文本上限，避免误复制大文件内容导致数据库膨胀。
- 删除操作应立即从 UI 消失；如果使用软删除，后续清理任务再物理移除。

## 12. 配置与依赖建议

### 12.1 Rust 依赖

- `arboard`：读写系统剪贴板。
- `rusqlite`：嵌入式 SQLite 存储。
- `sha2`：计算内容 hash。
- `chrono`：处理本地时间与日期分组。
- `tokio`：执行后台监听任务，若当前 Tauri 配置已可使用异步运行时。

### 12.2 前端依赖

- React + TypeScript：实现桌面 UI。
- TailwindCSS v4：负责样式系统。
- class-variance-authority / clsx / tailwind-merge：复用 shadcn 风格组件变体。
- lucide-react：图标。
- 自维护 `Button`、`Card`、`Badge`、`Switch` 等轻量 UI 组件，避免引入完整组件库。

## 13. 里程碑规划

### 13.1 Milestone 1：基础闭环

- 初始化 SQLite 数据库。
- 实现文本剪贴板轮询监听。
- 实现新增、去重、按日期查询。
- 前端展示日期侧栏和记录列表。
- 支持查看详情、复制、删除。

验收标准：复制一段文本后，应用能自动出现记录；重启应用后记录仍存在；点击复制能恢复到系统剪贴板。

### 13.2 Milestone 2：体验完善

- 增加搜索。
- 增加监听开关。
- 增加清空当前日期。
- 完善空状态、错误提示、加载状态。
- 增加重复记录计数展示。

验收标准：可以稳定按日期浏览历史内容，并能快速搜索和批量清理。

### 13.3 Milestone 3：桌面增强

- 增加系统托盘。
- 增加窗口隐藏/显示策略。
- 增加开机启动配置。
- 增加保留天数与最大记录数配置。
- 增加原生系统/设置菜单。
- 增加自定义本地存储目录、单条文本上限和敏感内容过滤配置。

验收标准：应用可作为长期后台工具使用，不影响用户正常工作流。

## 14. 风险与决策

### 14.1 监听方式

- MVP 建议使用轮询，优点是实现简单、跨平台稳定。
- 轮询间隔建议 500ms 到 1000ms，兼顾及时性与资源占用。
- 后续如发现 CPU 或电量问题，再评估平台原生事件监听。

### 14.2 删除策略

- 默认删除为软删除，便于减少误删风险和保留调试空间。
- 设置面板提供“清理已删除记录”，会物理删除软删除行，并可执行 SQLite `VACUUM` 压缩数据库。

### 14.3 长文本处理

- 已提供 `max_text_length` 设置，超长内容直接跳过保存。
- 已提供跳过原因提示；超长内容、疑似敏感内容被跳过时会在前端显示反馈。

## 15. 测试与验证计划

### 15.1 后端验证

- 数据库初始化成功。
- 新文本可写入数据库。
- 重复文本只更新计数，不新增重复记录。
- 删除后查询结果不再返回。
- 应用自身复制回流不会新增记录。

### 15.2 前端验证

- 日期侧栏数量与列表记录一致。
- 切换日期能正确加载记录。
- 搜索结果能正确展示并可复制。
- 删除记录后 UI 状态正确更新。
- 空状态、加载状态、错误状态可见且可理解。

### 15.3 集成验证

- 启动应用后复制文本，记录自动出现在当天列表。
- 关闭并重启应用，历史记录仍可查询。
- 暂停监听后复制文本，不会新增记录。
- 恢复监听后复制新文本，会继续新增记录。

## 16. 后续扩展方向

- 图片剪贴板：保存图片文件到应用数据目录，数据库记录文件路径与尺寸信息。
- 文件剪贴板：记录文件路径列表，提供打开所在目录能力。
- 收藏与标签：支持重要内容置顶、分类与快速过滤。
- 全局快捷键：快速唤起历史窗口或复制最近几条记录。
- 导入导出：支持 JSON 或 SQLite 备份恢复。
- 敏感规则：支持用户自定义正则过滤规则。

## 17. 当前收口状态

### 17.1 已收口

- 基础闭环：文本采集、去重、按日期查询、详情、复制、软删除。
- 体验能力：搜索、清空当前日期、空/加载/错误状态、重复计数展示。
- 桌面能力：系统托盘、隐藏/显示窗口、开机启动、原生系统/设置菜单。
- 本地设置：监听状态、保留天数、最大记录数、自定义 SQLite 存储目录、单条文本上限、自定义敏感规则。
- 数据安全：默认本地存储、不上传，敏感过滤可跳过疑似 JWT/API Key/长 token 及自定义正则命中的内容。
- 数据维护：支持物理清理已删除记录，并显式触发 SQLite `VACUUM` 压缩。

### 17.2 仍作为后续

- 图片、文件、富文本剪贴板支持。
- 收藏、标签、全局快捷键、导入导出。

### 17.3 P1/P2 分步开发路线图

完整执行计划见 `docs/superpowers/plans/2026-05-27-clipboard-p1-p2-optimization-roadmap.md`。

#### P1：可靠性与可解释性

1. ✅ 持久化监听状态，避免关闭监听后重启又自动开启。
2. ✅ 增加目录选择器与存储目录可写校验，避免手动输入错误路径。
3. ✅ 增加跳过原因反馈，让超长/敏感内容被跳过时可见。

#### P2：桌面语义与维护能力

1. ✅ 明确“大小”菜单语义，已改为“恢复默认大小”。
2. ✅ 增加软删除记录的物理清理与 SQLite vacuum。
3. ✅ 增加自定义敏感正则规则，覆盖团队/个人特定 token 格式。
