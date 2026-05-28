# 连接复用与保留策略触发收敛 — Follow-up

> 来源：2026-05-29 最终评审（commit `c2e3095` 完成后）
> 状态：已收集，待后续处理
> 评审范围：commits `eeada44`..`c2e3095`

## 1. 设置更新失败路径计数未重置

**位置**：`src-tauri/src/clipboard/service.rs` `update_desktop_settings` 末段

**现状**：`self.run_retention()?` 在错误返回时，下方 `captures_since_cleanup` 重置块不会执行。

**风险**：retention 失败后再次进入 `update_desktop_settings`，计数器仍可能维持在阈值附近，导致一次额外清理。当前 retention 失败被视为硬错误，但后置条件不完整。

**Spec 对照**：spec §6.4 隐含「设置更新即触发清理 + 把计数清零」两个原子性后置条件。

**建议**：把计数重置移到 `run_retention` 之前，或显式用 `match` 处理 retention 错误并确保计数仍重置。

---

## 2. 旧 `items_conn` 在持锁中 drop

**位置**：`src-tauri/src/clipboard/service.rs` `update_desktop_settings` swap 块

**现状**：`*items_guard = new_conn;` 触发旧 conn drop，此时 `path_guard` 与 `items_guard` 均持锁。

**风险**：SQLite drop 可能触发 WAL flush，相关 I/O 落在临界区。当前 swap 不与其他 reader 并发，影响有限；但与 follow-up #1 类似属于不变式脆弱。

**建议**：用 `std::mem::replace` 把旧 conn 取出到临界区外变量，guards 释放后再 drop。

---

## 3. retention 阈值测试绕过 service 写路径

**位置**：`src-tauri/src/clipboard/service_tests.rs` 中 `retention_runs_only_after_threshold_captures` 与 `retention_counter_resets_after_settings_update`

**现状**：测试用独立 `Connection::open` 直接 `INSERT` 准备数据，未经 `service.capture_current_clipboard`。

**风险**：未验证生产写路径与计数器的耦合；若未来 capture 流程重构，计数器逻辑可能与 retention 解耦而测试仍通过。

**建议**：评估改造测试经 capture 入口的成本；可考虑提取 `read_clipboard_text` 为可注入依赖，或保留当前快路径并增加一个集成用例覆盖全链路。

---

## 4. 切换 storage_dir 后旧目录残留 WAL/SHM

**位置**：`src-tauri/src/clipboard/service.rs` `update_desktop_settings` swap 后

**现状**：切换 storage_dir 后，旧目录下保留 `*.db-wal`、`*.db-shm` 副本文件。

**风险**：非正确性问题；用户手动迁移目录时可能困惑，或误以为旧目录还在被使用。

**建议**：swap 完成后对旧 path 做 best-effort 清理（仅当旧文件确属本应用所建），或在 storage_dir 设置 UI 文案中说明会留下原 SQLite 文件。

---

## 风险评估补充（来自评审）

- 串行 `Mutex<Connection>` 在 UI 并发查询 + 突发 capture 场景下可能产生延迟尖刺；当前测试套件单线程，未暴露此类争用
- Mutex 中毒（critical section 内 panic）会让 service 永久不可用至重启；当前所有临界区均无显式 panic 源，理论风险
