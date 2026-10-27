# 重构与修复计划（待办）

> **状态**：P1、P2-1~P2-8、P3-2、P3-4、P4-1~P4-8 已完成并从本文件移除（历史见 git log）。剩余：P4-9（条件项）、P3-1（可选）、P3-3（条件项）。
> **基线**：2026-10-27，master（P4-1~P4-8 后）；`pnpm test` 35 files / 229 tests、`cargo test` 170 passed / 10 ignored。
> **适用对象**：后续接手本仓库的编码智能体。
> **配套文档**：[AGENTS.md](AGENTS.md)（契约、命令、易错点，优先于本文件）、[ROADMAP.md](ROADMAP.md)（功能方向）。
> **定位说明**：任务卡里的行号是基线行号，代码变动后会漂移；定位时按符号名（函数/结构体/常量）搜索，不要机械相信行号。

---

## 0. 一页摘要

主体重构已经结束。当前维护成本集中在：

1. **条件/按需项**：P4-9 阅读器编排抽取（先评估）、P3-1 其余 DTO（只在 wire 变化时补）、P3-3 分页索引（只在有 benchmark 时做）。

执行顺序：

1. P4-9 先评估收益；不要为了覆盖率强行拆阅读器。
2. P3-1 / P3-3 按需推进。

一次只做一个任务卡，独立提交；先加失败用例再修缺陷；结构拆分、行为修复、测试补齐不要混在同一提交。

---

## 1. 目标与非目标

### 目标

- 让关键交互逻辑可单测，而不是只能靠 `pnpm tauri dev` 手点。
- 降低剩余 god file、重复样板与隐式状态机的维护成本。
- 新增列表页/平台/下载能力时只改一处。

### 非目标（不要做）

- 不重写阅读器三槽轨道；不回退阅读器子路由、`navigationStack`、显式 `state.from`。
- 不恢复在线详情对下载索引的离线回退；不把在线/已下载详情合并成一个 loading/fallback 状态机。
- 不重做 downloads/http 模块拆分；不拆 `favorites.rs`/`manhuagui.rs`/`FavoritesView.vue`（要拆先加任务卡）。
- 不改 IPC wire、错误码、i18n key、持久化版本、资产协议 scope、缓存键语义（除非任务明确要求 version bump）。
- 不为三平台抽公共 HTML 解析框架；不引入 TanStack Query/ORM/状态机库/UI 框架/生成器。
- 不把 `history` 并入通用 JSON store；不给 favorites/downloads 做通用 Repository trait。
- 不做与任务无关的重命名/格式化/文件移动。

---

## 2. 当前基线

### 2.1 剩余大文件（不拆，除非新任务卡）

`favorites.rs` 1205、`manhuagui.rs` 1092、`downloads/store.rs` 688、`FavoritesView.vue` 941、`downloads/worker.rs` 614、`DownloadsView.vue` 676、`ComicViewer.vue` 360。行数含测试/样式；这些文件都已按职责拆过一轮。

记录在案的例外（不是待办）：`LogsView`/`NhentaiDetail` 各自构造 `Intl.DateTimeFormat`；`search`/`ComicThumb`/`useCachedImage`/`usePagedResource` 的局部 `requestId`。

### 2.2 测试覆盖

P4-1~P4-7 已为阅读器手势/缩放、跨章预加载、下载/收藏动作、在线/已下载详情、IPC 入口、图片缓存与搜索守卫补上单测。新增测试沿用现有约定：与源码同级、jsdom、`vi.mock`/`@tauri-apps/api/mocks`、`NMessageProvider` harness、`effectScope`。

### 2.3 验证基线

```powershell
# 仓库根目录
pnpm test        # 35 files / 229 tests
pnpm build       # vue-tsc --noEmit + Vite build

# src-tauri/
cargo test       # 170 passed, 10 ignored
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Windows 下 `cargo test` 若以 `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` 退出，先从 `PATH` 移除 Android JDK 的 `bin`，不要改代码。无 CI，本地门禁是唯一门禁。

---

## 3. 全局约束

1. 外部契约保持当前版本：错误码、`{ code, message, details }`、IPC 命令名/参数、i18n key、持久化文件版本、缓存键语义、资产协议 scope、`/comic` 与 `/downloads` 详情父子路由。
2. 持久化字段不重命名/删除；`run_id` 不持久化；`favorites` 仍 version 2、`downloads` 仍 version 1。
3. 错误契约五处同步（`error.rs`、`src/errors.ts`、`useErrorMessage.ts`、`en.json`、`zh-CN.json`）；原则上不新增错误码。
4. Rust 命令返回 `Result<T, AppError>`，不返回展示文案；日志用 `logging::info/warn/error`，不要 `eprintln`。
5. 前端异步必须 `try/catch/finally` 复位 loading；定时器/监听器在 `onBeforeUnmount`/`onScopeDispose` 清理。
6. 测试用 `@tauri-apps/api/mocks` 或 `vi.mock` 模拟 IPC；不联网、不启动 Tauri、不新增需要真实驱动/网络/真机的用例。
7. 每个任务独立提交（Conventional Commits，祈使句、小写 subject）；依赖变化同时提交 `pnpm-lock.yaml` 与 `src-tauri/Cargo.lock`。
8. 改完一个任务就更新本文件；偏离任务卡时在“执行备注”追加一行。开始前先 `git status`，不要回滚他人未提交的改动。
9. 契约变化才更新 `AGENTS.md`：新增/修改命令、模块边界、测试体系、返回来源机制等。
10. P4 只加测试与局部修复：不改公开 composable/API 签名、IPC、路由、持久化或错误码；为可测性优先抽纯函数。
11. 缺陷与测试分开提交：先加失败用例再修，或用两个提交；不要用“顺手修”掩盖行为变化。

---

## 4. 未关闭缺陷

| ID | 症状 | 状态 | 对应任务 |
| --- | --- | --- | --- |
| BUG-11 | Rust/TS 类型漂移 | partial：ReaderChapter/DownloadDetail/HistoryPage 有契约测试，其余 DTO 未生成 | P3-1 |

BUG-1~BUG-10、BUG-12、BUG-13 的已修复项不再保留；需要追溯时看 git log。

---

## 5. 任务卡

### P3：剩余契约与长期项

#### P3-1 其余 DTO 契约（chore，可选）

- **状态**：in progress（`ReaderChapter`/`DownloadDetail`/`HistoryPage` 三份 fixture 已落地，其余 DTO 未生成）
- **范围**：`Search`/`Detail`/`Favorite`/`Log` 等 DTO 仍靠 TS interface + Rust serde 人工对齐。
- **做法**：只在准备新增/修改对应 wire 结构时，按 `contracts/*.json` 的既有模式补 fixture + 双端断言；先加 fixture 比引入生成器稳。生成器（ts-rs/specta）需要先做 spike，不是当前必做项。
- **验收**：字段漂移能让 `cargo test` 或 `pnpm test` 失败；生成文件不手改。

#### P3-3 分页索引（conditional）

- **状态**：todo（仅在有性能证据时做）
- **证据**：`list_history`/`list_favorites`/`list_downloads` 仍是 collect + sort 全量。
- **做法**：store 内维护按 `addedAt/updatedAt` 排序的 `Vec<Key>`，写时增量更新；或记录 benchmark 后接受现状。
- **验收**：有 benchmark/profiling 数据；分页结果顺序不变。

### P4：剩余可选与条件项

> P4 主线（P4-1~P4-7）与 P4-8 已完成并从本文件移除；这里只保留条件项。
> 测试约定沿用现有测试：jsdom + `vi.mock`/`@tauri-apps/api/mocks`、`NMessageProvider` harness、`effectScope`；测试不联网、不启动 Tauri。

#### P4-9 阅读器路由编排可测化（conditional）

- **状态**：conditional（未开始；先评估收益再决定是否抽取）
- **证据**：`ComicReaderView.vue` 约 463 行，包含缓存/在途页源选择、manhuagui 无章节深链解析首章、`?page=last`、URL query 同步、历史记录去抖 gate、body overflow；这些逻辑没有单测。
- **做法**：抽出 `useReaderChapterSource`（或等价 composable）负责“路由参数 → 章节数据/页码/URL/历史”的编排，视图只做渲染；用 mock 的 router + api 测。若评估认为抽取会破坏既定阅读器行为，保持现状并在任务卡记录理由，不要强行拆。
- **验收**：抽取后 `pnpm test`、`pnpm build` 通过；若环境可用，E2E 行为不变。
- **建议提交**：`refactor(reader): extract chapter source orchestration`

---

## 6. 验证与提交

### 6.1 最低门禁

| 改动范围 | 命令 |
| --- | --- |
| 前端 | `pnpm test` + `pnpm build` |
| Rust | `cargo test` + `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` |
| 前端 + Rust | 上面两组都跑 |

P4 只按自动化门禁验收；真实窗口冒烟（`pnpm tauri dev`）建议在发布前做，但不阻塞 P4 任务。手动冒烟清单以 `AGENTS.md` 的“验证”章节为准。

### 6.2 E2E 与 CI

- 无 CI；`pnpm test:e2e` 需要 `tauri-driver` 和匹配的 `msedgedriver.exe`，不是 P4 门禁。
- 涉及下载/收藏/离线/导航的改动，建议在有环境时至少跑一次 E2E；`e2e/` 只使用预置离线数据，不访问上游。

### 6.3 提交

- 一个任务卡一个提交；结构拆分、行为修复、测试补齐分开。
- 示例：`refactor(reader): extract gesture decision helpers`、`test(reader): cover pointer gestures and tap zones`、`fix(detail): reset the download action when the work changes`。
- 不要提交：`node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/schemas/`、`src-tauri/gen/android/**/build/`、`e2e/.drivers/`、未跟踪的编辑器文件。
- 文档同步：新增/修改命令、IPC 契约、错误码、持久化格式、目录/模块边界、返回来源机制、测试方式 → 更新 `AGENTS.md`；本文件只保留未完成事项。

---

## 7. 完成定义（DoD）

### 整体完成

- [x] P4-1~P4-7（测试补强主线）完成；`pnpm test` 35 files / 229 tests、`pnpm build` 通过。
- [x] BUG-13（下载 pending 泄漏）已修复并有回归测试。
- [x] P4-8（`KNOWN_PLATFORMS` 共享）完成；`cargo test` 170 passed / 10 ignored。
- [ ] P4-9 条件项按需推进。
- [ ] P3-1 只在 wire 变化时按需推进；P3-3 有 benchmark 证据再做。
- [x] 下一次新增列表页/平台/下载能力可以复用现有 composable、trait 和契约测试。

---

## 8. 执行备注

> 只记录当前未完成任务的偏离与结论；完成的任务随任务卡一起删除。

- 2026-10-27 / P4 规划 / 现状复核：前端 130 个测试、Rust 169 个测试均通过；临时 spike 验证了 jsdom 下伪造 PointerEvent + stub pointer capture 可驱动手势，`NMessageProvider` 包裹可测详情动作（spike 已删除）。未改动业务代码。
