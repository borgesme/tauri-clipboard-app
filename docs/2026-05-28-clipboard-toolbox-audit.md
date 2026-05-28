# Clipboard Toolbox 优化与遗留问题审查

> 审查日期：2026-05-28
> 审查范围：`src/`、`src-tauri/src/`、`docs/`、根目录配置文件
> 审查方法：源代码阅读 + 设计文档对照 + git log 比对

---

## 项目状态概览

**已收口**：

- MVP 文本闭环（采集、去重、按日期查询、详情、复制、软删除）
- M2 体验完善（搜索、清空当日、空/加载/错误状态、重复计数）
- M3 桌面增强（系统托盘、隐藏/显示、开机启动、原生菜单）
- P1/P2 路线图（监听持久化、目录选择校验、跳过原因事件、菜单语义、物理清理、自定义敏感正则）
- 后端测试覆盖较完整（`service_tests` / `repository_tests` / `storage_path_tests`）
- TypeScript `strict` + `noUnusedLocals` / `noUnusedParameters` 全开

**仍有 22 个可优化点**，按下方优先级分级。

---

## 🔴 P0 — 影响正确性或对外品质

### 1. 窗口尺寸三处不一致

- `src-tauri/tauri.conf.json:14` 启动尺寸 `960×640`
- `src-tauri/src/desktop.rs:17-18` "恢复默认大小"菜单写死 `1100×720`
- `src/App.css:3` `.clip-app` `min-width: min(860px, 100%); min-height: 620px`

点击"恢复默认大小"会得到比启动更大的窗口。建议把常量集中到 Rust 端，前后端读同一来源。

### 2. README.md 仍是 Vite 模板默认内容

没有项目介绍、截图、安装/构建步骤。对外完全不可读。

### 3. 版本号停留在 0.1.0

`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 都没升。完成 M1+M2+M3+P1/P2 后建议至少 `0.4.0`。

### 4. 敏感内容误报率偏高

位置：`src-tauri/src/clipboard/settings.rs:201` `looks_like_secret_token`

规则：长度 ≥16、无空白、字符集 `[A-Za-z0-9_\-=]`、字母 ≥8、数字 ≥4。

误伤目标：git commit hash、UUID（含 `-`）、base64、JWT 之外的随机 ID、Git 短链 token。一旦用户开启敏感过滤，复制 commit hash 会被静默跳过。

建议：更收敛的规则（如要求"既有大写又有小写又有数字"或熵阈值）。

### 5. 日期分组依赖时区敏感字段

位置：`src-tauri/src/clipboard/service_runtime.rs:42`

`Local::now().to_rfc3339()` 写入 `created_at`，`repository.rs:64` 用 `substr(created_at, 1, 10)` 截前 10 位拿日期。用户切换时区、夏令时切换或备份到异时区机器，历史记录的日期组会错位。

建议：存 UTC 时间 + 独立的 `local_date` 列，或显式生成 `Local::now().format("%Y-%m-%dT%H:%M:%S")`（去时区后缀）。

---

## 🟡 P1 — 性能与可维护性

### 6. 数据库连接每次都新开

位置：`src-tauri/src/clipboard/repository.rs` 所有公共函数

每次 `Connection::open(path)?`。监听 800ms 一次循环 + 写入路径还会触发 `apply_retention_policy` → 再读 settings + 跑 2 条 cleanup UPDATE，单次写入打开 5+ 次连接。

建议：持有 `Mutex<Connection>` 或引入 `r2d2_sqlite`。

### 7. 保留策略每次写入都执行

位置：`src-tauri/src/clipboard/service.rs:65`

`capture_current_clipboard` 每写入一条就调用 `apply_retention_policy`，其中 `cleanup_by_count`（`ORDER BY ... LIMIT -1 OFFSET ?`）需要全表排序。记录超过 1000 条后写入路径越来越慢。

建议：改为按计数阈值或后台定时触发。

### 8. 前端零单元测试

`src/` 下没有任何 `*.test.ts(x)`，没装 Vitest。`useClipboardWorkspace`、`clipStudioHelpers`、`skipMessage` 等核心逻辑全裸奔。

### 9. 错误统一吞进 `eprintln!`

位置：`monitor.rs:25,33,58`、`commands.rs:173,179`、`desktop.rs:54,197`

打包后 stderr 不可见。建议接 `tracing` + 关键失败 emit 到前端。

### 10. CSP 配置为 null

位置：`src-tauri/tauri.conf.json:21`

`security.csp: null`。即使是本地工具也应当至少设 `default-src 'self'`。

### 11. 设置面板每个字段改动都触发全量保存

位置：`src/components/clipboard/DesktopSettingsPanel.tsx:43-58`

拖一下 Switch 就发完整 7 字段 update（含 `customSecretPatterns` 文本）+ 重新 `init_database`。

建议：改为"草稿 + 显式保存"或全量防抖。当前的 `StorageDirRow` 已是草稿模式，可统一这种交互。

### 12. search 用 `LIKE %x%` 全表扫描

位置：`src-tauri/src/clipboard/repository.rs:92`

content/preview 没 FTS 索引。数据量上千后会卡顿。

建议：引入 SQLite FTS5。

### 13. `Mutex` 改 `RwLock`

位置：`src-tauri/src/clipboard/service.rs:22-25`

监听线程主要读 `monitor_enabled` / `database_path`，读多写少，RwLock 减少阻塞。次要优化。

---

## 🟢 P2 — 完善向

### 14. 回收处于中间态

软删除后没有"回收站"UI，用户既看不到也不能恢复，最终只能"清理"。要么提供回收站，要么直接改物理删除。

### 15. 存储目录切换无数据迁移

UI 警告写了但没有"迁移旧数据"按钮，体验缺一块。

### 16. 没启用日志插件

未引 `tauri_plugin_log`，dev 阶段后端排查全靠 println。

### 17. 窗口缺 `minWidth` / `minHeight` / `resizable`

`src-tauri/tauri.conf.json` 没限制，可缩到 UI 崩溃。

### 18. `App.tsx` 中 interface 用在前定义在后

位置：`src/App.tsx:11` 使用，`src/App.tsx:30` 定义

TS 容忍但风格不佳。

### 19. VACUUM 阻塞且无 UI 反馈

`purgeDeletedClipboardItems(true)` 默认带 vacuum，表大时秒级阻塞 UI 没有进度提示。

### 20. `bundle.targets` 只有 nsis

未来 macOS / Linux 需扩展。

### 21. 布尔设置存 `"0"/"1"` 字符串

`bool_to_setting` 转换比较绕，可直接存 `"true"/"false"` 或拆 INTEGER/TEXT 两个 helper。

### 22. 设置文档需要补

`docs/clipboard-toolbox-design.md` 第 17 节是手工维护的"已收口"列表，跟实际代码同步靠人。可加 lint 或 CI 校验。

---

## 推荐执行顺序

按性价比从高到低：

1. **P0 第 1、3 项**（窗口尺寸统一 + 版本号升级）—— 30 分钟内可完成
2. **P0 第 4 项**（敏感规则收敛）—— 1 小时（写阈值 + 补测试）
3. **P1 第 6、7 项**（连接复用 + 保留策略改触发条件）—— 半天，性能收益最大
4. **P0 第 2 项**（README）—— 1 小时
5. **P1 第 8 项**（前端测试基础设施）—— 半天搭 Vitest + 关键 hook 测试

---

## 审查清单

| 优先级 | 编号 | 主题 | 影响面 |
|------|----|----|------|
| P0 | 1 | 窗口尺寸三处不一致 | 体验 |
| P0 | 2 | README 为模板默认 | 对外品质 |
| P0 | 3 | 版本号停留在 0.1.0 | 发布卫生 |
| P0 | 4 | 敏感规则误报 | 正确性 |
| P0 | 5 | 日期分组时区敏感 | 数据正确性 |
| P1 | 6 | DB 连接每次新开 | 性能 |
| P1 | 7 | 保留策略每次写入触发 | 性能 |
| P1 | 8 | 前端零测试 | 可维护性 |
| P1 | 9 | 错误吞进 `eprintln!` | 可观测性 |
| P1 | 10 | CSP 为 null | 安全卫生 |
| P1 | 11 | 设置面板全量保存 | 体验 |
| P1 | 12 | search 无 FTS | 性能 |
| P1 | 13 | Mutex → RwLock | 性能 |
| P2 | 14 | 回收无 UI | 体验 |
| P2 | 15 | 存储目录无迁移 | 体验 |
| P2 | 16 | 没启用日志插件 | 开发体验 |
| P2 | 17 | 窗口无尺寸约束 | 体验 |
| P2 | 18 | interface 用在定义前 | 代码风格 |
| P2 | 19 | VACUUM 阻塞无反馈 | 体验 |
| P2 | 20 | bundle 只支持 nsis | 平台覆盖 |
| P2 | 21 | 布尔设置存字符串 | 代码风格 |
| P2 | 22 | 设计文档手工同步 | 可维护性 |
