# 重构与修复计划

> **状态**：P1 + P2-1 已完成；P2-2a~f（页码/边界、`usePageTurn`、滚轮/键盘、`ReaderTrack`、`ReaderChrome`、平台兜底）已完成；P2-2 手势与 P2-3~P3 待执行
> **基线**：2026-10-27，当前 master 工作区
> **适用对象**：后续接手本仓库的编码智能体
> **配套文档**：[AGENTS.md](AGENTS.md)（契约、命令、易错点）、[ROADMAP.md](ROADMAP.md)（功能方向）
> **行号说明**：任务卡里的 `file:line` 是基线行号，代码变动后会漂移；定位时优先按符号名（函数/结构体/常量）搜索，不要机械相信行号。

---

## 0. 一页摘要

这个仓库的架构意图是清楚的：UI 只经 `invoke`、错误码契约、i18n schema 派生类型、平台 trait、缓存键版本化、`spawn_blocking` 等该有的东西都有。当前问题不是“缺设计”，而是功能增长后**几处边界失守**：

1. **隐式状态机**：阅读器翻页、详情在线/离线、下载任务，都用散落的 `ref` + `watch` + 定时器表达。
2. **基础设施复制**：3 套 JSON 持久化、3 个列表页、4 份平台标签/时间格式化、多份日志包装。
3. **层间穿透**：下载直接调 image 内部、JM 解码写进通用管线、缩略图绕过 `api/images.ts`。
4. **契约手抄**：Rust `model.rs` 与 TS interface 双份维护，无运行时校验。
5. **单文件过大**：`ComicViewer.vue` 1411 行、`downloads.rs` 2231 行、`http.rs` 1030 行。

执行顺序：

- **P1（先做）**：低风险去重 + 高置信缺陷修复。
- **P2（再做）**：结构性拆分，改变代码组织但不改变外部行为。
- **P3（长期）**：契约生成、缓存清扫、worker 测试等。

**最小可交付**：只完成 P1-1 ~ P1-8，就能拿到大部分“去重 + 修高风险缺陷”的收益；P2 建议另开分支、按任务逐个合入。不要为了“一次重构完”把 P1 和 P2 混成一个巨型提交。

**一次只做一个任务卡，独立提交。** 不要在一个提交里顺手格式化无关文件，也不要把 P2 的拆分和 P1 的修复混在一起。

---

## 1. 目标与非目标

### 目标

- 降低 god file、重复样板、隐式状态机的维护成本。
- 修掉已确认的高风险竞态、缓存与错误契约问题。
- 让“新增一个列表页 / 新增一个平台 / 新增一种下载能力”只需要改一处。
- 让关键逻辑可单测，而不是只能靠 `pnpm tauri dev` 手点。

### 非目标（不要做）

- 不重写阅读器的三槽轨道渲染方案。
- 不为三个平台抽公共 HTML 解析框架（上游 DOM 差异大，保持独立更稳）。
- 不引入 TanStack Query、ORM、状态机库、UI 框架等新依赖。
- 不改 IPC wire 形状、错误码、i18n key、路由路径、持久化版本、资产协议 scope、缓存键语义（除非任务明确要求 version bump）。
- 不做与任务无关的重命名/格式化/文件移动。
- 不把 `history` 并入通用 JSON store（它是 append-only JSONL，语义不同）。
- 不给 `favorites`/`downloads` 做“通用 Repository trait”（三处 sanitize、迁移、corrupt 语义不同，过度抽象会压掉差异）。

---

## 2. 基线证据

### 2.1 前端热点

| 文件 | 行数 | 主要问题 |
| --- | ---: | --- |
| `src/components/reader/ComicViewer.vue` | 1411 | 轨道渲染、翻页动画、指针/双指手势、点击分区、键盘、滚轮、HUD、全屏、预加载全在一个 SFC |
| `src/views/ComicDetailView.vue` | 983 | 在线详情 + 下载索引 + 历史 + 收藏四路并发；离线回退靠 1.5s 定时器；下载 1.5s 轮询 |
| `src/views/FavoritesView.vue` | 1006 | 列表分页、分组 CRUD、弹窗、长按/移动全部混在一起 |
| `src/views/DownloadsView.vue` | 747 | 列表分页、轮询、暂停/继续/删除、格式化 |
| `src/views/HistoryView.vue` | 349 | 与上面两个列表页重复分页/时间格式化/平台标签 |

重复的具体证据：

- `PAGE_SIZE`、`requestId`、`load/changePage`、`pageCount/countText/errorText`、`Intl.DateTimeFormat` 在三个列表页各写一遍（`HistoryView.vue:14-110`、`FavoritesView.vue:31-225`、`DownloadsView.vue:23-214`）。
- `PLATFORM_LABEL_KEYS` 有四份：`HistoryView.vue:36`、`FavoritesView.vue:92`、`DownloadsView.vue:53`、`DetailShell.vue:19`。
- `requestId` 竞态守卫在 5 个文件里出现 26 次（`ComicDetailView.vue:438`、`DownloadsView.vue:226`、`ComicReaderView.vue:224`、`FavoritesView.vue:148`、`ComicThumb.vue:32` 等）。
- `ComicThumb.vue:42` 直接 `invoke("fetch_image")` 并重复定义 `CachedImage`，绕过 `api/images.ts:58` 的 in-flight 合并与已解析路径缓存。

### 2.2 Rust 热点

| 文件 | 行数 | 主要问题 |
| --- | ---: | --- |
| `src-tauri/src/downloads.rs` | 2231 | DTO、锁、索引事务、摘要投影、manifest 解析、worker、命令全在一个文件 |
| `src-tauri/src/image/platform/manhuagui.rs` | 1079 | 解析器 + LZString 解包 + 导航 + 测试 |
| `src-tauri/src/image/http.rs` | 1030 | 代理校验、client 构建、ECH 状态机/回退、镜像重试、图片校验、文档抓取 |
| `src-tauri/src/favorites.rs` | 1252 | 与 downloads 重复 load/save/backup/分页/校验 |
| `src-tauri/src/history.rs` | 673 | 与上面重复分页 clamp、`now_millis`、`KNOWN_PLATFORMS` |

### 2.3 验证基线

在改动前先跑一遍，记录当前是否全绿：

```powershell
# 仓库根目录
pnpm test
pnpm build

# src-tauri/
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Windows 下若 `cargo test` 以 `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` 退出，先按 [AGENTS.md](AGENTS.md) 的说明从 `PATH` 移除 Android JDK 的 `bin`，不要改代码。

---

## 3. 全局约束（必须遵守）

1. **外部契约不变**：错误码、`{ code, message, details }` 形状、IPC 命令名/参数、i18n key、路由、持久化文件版本、缓存键语义、资产协议 scope。
2. **持久化字段不重命名/删除**：本项目没有迁移层。`favorites` 仍为 version 2，`downloads` 仍为 version 1；`run_id` 是 `#[serde(skip)]` 的运行时字段，不要持久化。
3. **错误契约五处同步**：只有新增/修改错误码时才需要改 `error.rs`、`src/errors.ts`、`useErrorMessage.ts`、`en.json`、`zh-CN.json`。本计划原则上不新增错误码。
4. **Rust 命令返回 `Result<T, AppError>`**：不要返回展示文案；日志用 `logging::info/warn/error`，不要 `eprintln`（现有 `http.rs` 的 `eprintln` 可在 P2-5 清掉）。
5. **前端异步必须 `try/catch/finally` 复位 loading**；定时器/监听器在 `onBeforeUnmount`/`onScopeDispose` 清理。
6. **测试用 `@tauri-apps/api/mocks` 模拟 IPC**；新增纯函数优先补 Vitest，不要为了测试引入 Tauri mock runtime。
7. **每个任务独立提交**：Conventional Commits，祈使句、小写 subject；依赖变化同时提交 lockfile。
8. **改完一个任务就在本文件勾选**；如果偏离任务卡，在任务卡末尾追加“执行备注”，不要悄悄改设计。
9. **不要回滚工作区里其他人/其他任务的未提交改动**；开始前先 `git status`，避免把无关文件带进提交。
10. **契约变化才更新 AGENTS.md**；纯重构不需要，但如果新增了命令/模块边界/测试体系，必须同步。

---

## 4. 缺陷登记

以下缺陷按“修复优先级”排序，任务卡会引用这些 ID。**不要在重构里顺手修多个缺陷**；一个缺陷一个提交，便于回滚。

| ID | 症状 | 根因/证据 | 修复任务 |
| --- | --- | --- | --- |
| BUG-1 | JM 进程内第一条搜索/详情/阅读可能用旧版本签名失败 | `prepare` 在构建请求之后调用：`image/mod.rs:234-236`、`280-282`、`337-339`；JM 在 `search_request` 里读 `app_version` 签名：`jm.rs:83`、`jm_api.rs:127-129` | P1-5 |
| BUG-2 | 暂停 → 删除 → 重新下载后，旧 worker 可能写新记录/新目录 | `run_id` 是运行时字段（`downloads.rs:139-142`），删除后新记录从 0→1 复用旧值；`is_current` 只比对 `run_id + Downloading`（`downloads.rs:559-565`） | P1-6 |
| BUG-3 | 断电/异常后零字节缓存被永久命中；JM 解码算法变更不失效 | `find_cached` 只判 `is_file`（`cache.rs:31-43`）；`write_atomically` 无 `sync_all`（`cache.rs:61-74`）；解码键固定 `?decoded`（`image/mod.rs:110-112`） | P1-7 |
| BUG-4 | 200 挑战页会把坏镜像钉为 JM 首选 | `note_request_outcome` 在 HTTP 2xx 非空时就算成功（`http.rs:642-646`），解析失败在其后 | P2-4 |
| BUG-5 | 空章节错误文案分叉 | manhuagui 返回 `Unexpected`（`manhuagui.rs:226-230`），JM/nhentai 返回 `NotFound`（`jm.rs:401-405`、`nhentai.rs:117-121`） | P1-8 |
| BUG-6 | 边界滑动切章前画面保持错位 | `trackOffset` 在边界路径未 `snapBack`：`ComicViewer.vue:610-611`、`661-666` | P2-2 |
| BUG-7 | 动画期间指针仍可写 `trackOffset` | `onStagePointerMove` 无 `trackAnimating` 检查：`ComicViewer.vue:552-613`；动画写同一变量：`198-221` | P2-2 |
| BUG-8 | 页码三个真相，`?page=last` 依赖 watcher 注册顺序 | `props.page`/`displayPage`/`route.query.page` 互写：`ComicViewer.vue:316-327`、`ComicReaderView.vue:228/318-345` | P2-2 |
| BUG-9 | 详情离线回退是定时器与在线请求的隐式赛跑 | `setTimeout(1500)` 直接改 UI：`ComicDetailView.vue:678-727`；下载轮询：`486-537` | P2-1 |
| BUG-10 | 假平台兜底污染缓存键 | `viewerPlatform` 回退 `"nhentai"`：`ComicReaderView.vue:64-66` | P2-2 |
| BUG-11 | Rust/TS 类型漂移（如 `ReaderChapter.thumb` 可空但前端写 `string`） | `reader.ts:23` vs `history.ts:9/23`；`model.rs:234-244` | P3-1 |
| BUG-12 | 返回来源依赖 `window.history.state.back` 的内部结构 | `useBackNavigation.ts:10-24` | P2-7 |

> 说明：BUG-1/BUG-2/BUG-3 建议在 P1 修掉；BUG-4/BUG-6/BUG-7/BUG-8 需要结构拆分，放 P2；BUG-11 是长期契约问题。

---

## 5. 任务卡

### P1：低风险、先去重和修缺陷

#### P1-1 列表页基础设施（refactor）

- **状态**：done（2026-10-27，自动化门禁通过；UI 冒烟待做）
- **依赖**：无
- **证据**：三个列表页各自实现分页/竞态/格式化；`PLATFORM_LABEL_KEYS` 四份（见 2.1）。
- **目标**：把“分页列表页”的通用部分收成 composable 和格式化工具，三个 view 只保留领域逻辑。
- **新增文件**：
  - `src/composables/usePagedResource.ts`
  - `src/i18n/format.ts`（或 `src/lib/format.ts`，二选一，不要两处都建）
  - `src/i18n/platform.ts`
- **建议接口**：

```ts
// usePagedResource.ts
export interface PagedResult<T> {
  total: number;
  entries: T[];
}

export function usePagedResource<T>(options: {
  pageSize?: number;
  fetch: (offset: number, limit: number) => Promise<PagedResult<T>>;
}): {
  entries: Ref<T[]>;
  total: Ref<number>;
  page: Ref<number>;
  pageCount: ComputedRef<number>;
  loading: Ref<boolean>;
  error: Ref<AppError | null>;
  load: (silent?: boolean) => Promise<void>;
  changePage: (next: number) => Promise<void>;
  reload: () => Promise<void>;
  reset: () => Promise<void>; // page = 1 + load，用于分组/筛选变化
};
```

- **迁移顺序**（先易后难，每一步都能单独提交/验证）：
  1. `HistoryView`：纯分页，无轮询、无分组，先迁。
  2. `DownloadsView`：保留 1s 轮询；`load(true)` 必须不清空列表、不显示 spinner、不覆盖已有 `loadError`。
  3. `FavoritesView`：保留分组筛选、删除后回填/夹页、`loadGroups` 独立；删除逻辑不要塞进 composable。
  4. `DetailShell`：改用共享的 `platformLabel()`。
- **不要做**：不要为了统一而把 Favorites 的分组 CRUD、Downloads 的 pause/resume/delete 也抽象进 composable。
- **验收**：
  - [ ] 三个 view 里不再各自定义 `PAGE_SIZE`、`requestId`、`timestampFormatter`。
  - [ ] 分页、空态、错误重试、静默轮询行为与改动前一致。
  - [ ] `usePagedResource.test.ts` 覆盖：过期响应丢弃、翻页、错误、silent 刷新、total 变小后夹页。
  - [ ] `pnpm test`、`pnpm build` 通过。
- **手动冒烟**：历史/收藏/下载各翻两页；断网或让接口失败看错误态；下载进行中看轮询不闪。
- **建议提交**：`refactor(web): extract paged resource composable`

#### P1-2 异步竞态守卫（refactor）

- **状态**：done（2026-10-27，自动化门禁通过；UI 冒烟待做）
- **依赖**：无（可与 P1-1 并行）
- **证据**：`++requestId` / `current !== requestId` 在 5 个文件出现 26 次。
- **目标**：用一个 `useLatestRequest` 替代手写 token，减少漏判和样板。
- **新增文件**：`src/composables/useLatestRequest.ts`
- **建议接口**：

```ts
export function useLatestRequest(): {
  begin: () => number;
  isCurrent: (token: number) => boolean;
  invalidate: () => void;
};
```

- **迁移范围**：
  - `ComicDetailView` 的 `load`、`loadFavoriteState`、`loadHistoryState`、`loadDownloadState`、`toggleFavorite`、`onDownloadClick`。
  - `ComicReaderView.load` 与 `onBeforeUnmount`。
  - `FavoritesView` / `DownloadsView`：若 P1-1 已把 token 收进 composable，则只保留 composable 内部的实现。
  - `ComicThumb`、`useCachedImage`：**可以保留**各自的局部整数，因为它们是单实例、无共享状态的简单守卫；在任务备注里记录这个例外，不要为了统一而强行改。
- **验收**：
  - [ ] 共享 helper 之外不再出现 `++requestId`（`ComicThumb`/`useCachedImage` 例外需在代码注释或任务备注说明）。
  - [ ] 过期响应不会写 `detail/error/loading`；卸载后不再写状态。
  - [ ] `useLatestRequest.test.ts` 覆盖 `begin/isCurrent/invalidate`。
  - [ ] `pnpm test`、`pnpm build` 通过。
- **建议提交**：`refactor(web): add latest-request guard`

#### P1-3 Rust 持久化叶子函数（refactor）

- **状态**：done（2026-10-27，自动化门禁通过）
- **依赖**：无
- **证据**：`favorites.rs`、`downloads.rs`、`history.rs` 各自实现 temporary/backup/原子写/分页 clamp/`now_millis`；`KNOWN_PLATFORMS`、`DEFAULT_PAGE_SIZE` 各三份。
- **目标**：只抽“叶子函数”，不抽 Repository，不动 history 的 append-only 语义。
- **新增文件**：`src-tauri/src/storage.rs`，在 `lib.rs` 加 `mod storage;`。
- **建议接口**：

```rust
pub(crate) const DEFAULT_PAGE_SIZE: u32 = 50;
pub(crate) const MAX_PAGE_SIZE: u32 = 200;

pub(crate) fn page_window(offset: Option<u32>, limit: Option<u32>) -> (usize, usize);
pub(crate) fn now_millis() -> u64;
pub(crate) fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> crate::error::Result<()>;
pub(crate) fn backup_corrupt(path: &Path) -> crate::error::Result<bool>;
```

- **迁移范围**：
  - `favorites.rs:588-638` 的 `write_document`：保留排序/默认分组补全，只把“临时文件 + sync_all + rename”替换为 `storage::atomic_write_json`。
  - `downloads.rs:1281-1305` 的 `write_document`：同上。
  - `favorites.rs`、`downloads.rs`、`history.rs` 的 `now_millis`。
  - `list_favorites`、`list_downloads`、`list_history` 的 offset/limit clamp。
  - `KNOWN_PLATFORMS` 可提到 `storage` 或 `image::platform`？**不要**：它属于领域校验，放 `src-tauri/src/platforms.rs` 也可以，但优先级低；如果抽，确认三处错误文案不变。
- **不要做**：不要把 `history` 的 `append_event` 改成原子重写；不要把 `FavoritesData::load` 的 v1→v2 迁移泛型化。
- **测试**：
  - `storage::tests`：原子写后读回、临时文件不残留、目标文件已存在时可覆盖、`backup_corrupt` 把原文件改名为 `.bak`。
  - 现有 favorites/downloads/history 测试全部通过。
- **验收**：
  - [ ] 两个 `write_document` 不再各自实现 temp/sync/rename。
  - [ ] 文件格式、版本、`.bak` 行为、日志 target 与改动前一致。
  - [ ] `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。
- **建议提交**：`refactor(rust): share atomic JSON storage helpers`

#### P1-4 下载命令的阻塞 IO 统一（fix/refactor）

- **状态**：done（2026-10-27，自动化门禁通过；下载 UI 冒烟待做）
- **依赖**：P1-3（可选，先做也行）
- **证据**：favorites/history 命令用 `spawn_blocking`（`favorites.rs:754`、`history.rs:404`），downloads 命令直接在 `async fn` 里同步调用 store 和 `std::fs`（`downloads.rs:1746-1855`）；`delete` 还在持锁时 `remove_dir_all`（`downloads.rs:439-456`）。
- **目标**：统一“同步 store + 阻塞 IO 在 blocking 池执行”的策略，避免 1s 轮询/worker/命令互相堵 tokio worker。
- **改动**：
  1. `start_download`、`pause_download`、`resume_download`、`delete_download`、`list_downloads`、`get_download` 包进 `tauri::async_runtime::spawn_blocking`，模式照抄 `favorites.rs:754-763`。
  2. `get_downloaded_chapter`：把 store 快照 + manifest 读取整体放进 `spawn_blocking`，用 `std::fs::read` 替代 `tokio::fs::read`；或者保留 async，但 store 快照也 `spawn_blocking`。二选一，保持模块内一致。
  3. `download_work`/`run_download` 保持 async（网络 + tokio fs）；store 的短锁操作可留在原处。
  4. `delete` 暂时保留“持 data 锁执行 remove_dir_all”的语义（它保证索引与目录一致）；因为现在整体在 blocking 池，不会阻塞 async runtime。若要在 P2-3 进一步拆，必须引入 per-work 删除守卫，**不要**在 P1 直接改成“先删索引再删目录”。
- **测试**：
  - 现有 downloads 单测通过。
  - 补一个 store 层测试：start → pause → delete → 再次 start 的索引状态正确（可与 P1-6 合并）。
- **验收**：
  - [ ] `downloads.rs` 的 `async fn` 命令体内不再直接做同步文件 IO（grep `std::fs`/`fs::` 确认）。
  - [ ] 暂停/继续/删除/列表/详情行为不变。
  - [ ] 下载进行中打开下载页轮询，UI 不卡顿（`pnpm tauri dev` 手测）。
  - [ ] Rust 三门禁通过。
- **风险/回滚**：`spawn_blocking` 需要 `AppHandle`/`State` 满足 `Send + 'static`；当前命令都满足。若编译报错，优先 clone `AppHandle`，不要把 `State` 引用跨线程。
- **建议提交**：`fix(downloads): run blocking index work off the async runtime`

#### P1-5 修正 JM `prepare` 顺序（fix，BUG-1）

- **状态**：done（2026-10-27，自动化门禁通过；JM 联网冒烟待做）
- **依赖**：无
- **证据**：`image/mod.rs:234-236`、`280-282`、`337-339` 先构建 `RequestSpec` 再 `prepare`；JM 的 `api_spec` 在构建时读取 `app_version` 生成 `tokenparam`（`jm.rs:83`、`jm_api.rs:127-129`），所以进程内第一条命令仍用内置版本。
- **目标**：保证“准备（同步版本/令牌）一定发生在构建请求之前”，并让这个顺序不容易被后续改回去。
- **改动**：
  1. 在 `image/mod.rs` 增加一个 helper，强制顺序：

```rust
async fn prepared_spec(
    client: &HttpClient,
    platform: &dyn Platform,
    build: impl FnOnce() -> Result<RequestSpec>,
) -> Result<RequestSpec> {
    platform.prepare(client).await;
    build()
}
```

  2. `search_comics_inner`、`get_comic_inner`、`get_reader_chapter_inner` 改为：

```rust
let spec = prepared_spec(client, platform, || platform.search_request(query, page)).await?;
let body = http::fetch_document(client, platform, &spec).await?;
```

  3. 注释写“为什么”：JM 的签名头依赖 `prepare` 刷新的版本，顺序错了首条命令会用旧版本。
- **测试**：
  - 在 `image/mod.rs` 的测试模块里实现一个假 `Platform`：`prepare` 设置 `AtomicBool`，`search_request` 断言该 bool 已置位；`prepared_spec` 返回 spec 后断言。这样不需要网络。
  - 现有 platform 测试通过。
- **验收**：
  - [ ] 三个 inner 函数都通过 `prepared_spec`（或等价 helper），不再手写“先 spec 后 prepare”。
  - [ ] 假平台测试能在顺序错误时失败。
  - [ ] 手动验证：重启应用后第一次 JM 搜索/详情成功；日志里能看到版本同步记录（若开启日志）。
- **建议提交**：`fix(jm): prepare the app version before building requests`

#### P1-6 修复下载 `run_id` ABA（fix，BUG-2）

- **状态**：done（2026-10-27，自动化门禁通过）
- **依赖**：P1-4（建议先做，便于测试）
- **证据**：`DownloadRecord::new` 的 `run_id` 为 0（`downloads.rs:166`），`start`/`resume` 用 `record.run_id.saturating_add(1)`（`349`、`389`），`run_id` 不持久化（`139-142`）。删除记录后重新 start，新记录又从 0→1，旧 worker 的 `run_id=1` 仍能通过 `is_current`（`559-565`）。
- **目标**：进程内 run id 单调递增，永不复用；旧 worker 在任何“暂停/删除/重启下载”后都不能写新记录。
- **改动**：
  1. `DownloadsStore` 增加 `run_seq: AtomicU64`（或 `Mutex<u64>`，但 Atomic 更简单）。
  2. 新增 `fn next_run_id(&self) -> u64`（`fetch_add(1) + 1`）。
  3. `start`/`resume` 在进入 `with_data` 前分配 `run_id`，写入 `record.run_id = run_id`；删除 `record.run_id.saturating_add(1)` 的两处。
  4. `DownloadRecord::new` 增加 `run_id` 参数，或在 `new` 后由调用方赋值；测试 helper 同步更新。
  5. `pause` 仍把 `run_id` 设为新的更大值（或只置 Paused，`is_current` 因 status 变化已失效；但为了语义清晰，保留 bump 也行）。
- **测试**（模块内，用临时索引文件）：
  1. start → `run_id=1`；
  2. pause；
  3. delete；
  4. start → `run_id=2`；
  5. `is_current(path, key, 1)` 为 false。
  6. 再补一个“旧 worker 在新 start 后调用 `note_page`/`complete_chapter` 返回 false”的测试。
- **验收**：
  - [ ] run id 分配处只有一个来源（`next_run_id`）。
  - [ ] 删除后重新下载，旧任务不能改新记录；暂停/继续仍能正确让旧 worker 失效。
  - [ ] 现有下载测试通过；Rust 三门禁通过。
- **注意**：`run_id` 是 `#[serde(skip)]`，**不要**为了修这个把它持久化；那会改变文件格式且没有必要。
- **建议提交**：`fix(downloads): allocate monotonic run ids`

#### P1-7 缓存健壮性（fix，BUG-3）

- **状态**：done（2026-10-27，自动化门禁通过）
- **依赖**：无
- **证据**：`cache.rs:31-43` 只判断 `is_file`；`cache.rs:61-74` 用 `tokio::fs::write` 后直接 rename，无 `sync_all`；解码键固定 `?decoded`（`image/mod.rs:110-112`）。
- **目标**：坏缓存可自愈；解码算法升级能失效旧缓存。
- **改动**：
  1. `find_cached`：`metadata.len() > 0` 才算命中；零字节文件视为 miss，可 best-effort `remove_file`（失败忽略）。
  2. `write_atomically`：改成 `tokio::fs::File::create` + `write_all` + `flush` + `sync_all`，再 rename；失败清理临时文件。若担心性能，至少保留读取端长度校验。
  3. 解码键：增加 `const IMAGE_DECODE_VERSION: u32 = 1;`，键改为 `?decoded=v{version}`；修改 JM 解码算法时 bump。旧文件不删除，等 P3-2 清扫。
  4. `image/mod.rs` 的 `?decoded` 拼接处同步改。
- **测试**：
  - `cache.rs` 新增：零字节文件 `find_cached` 返回 None；写入后读回长度正确；临时文件不残留。
  - `image/mod.rs` 测试：解码键包含版本号（可把拼接逻辑提成纯函数 `decode_key_suffix()` 再测）。
- **验收**：
  - [ ] 零字节缓存不会被当作命中。
  - [ ] 写入具有持久性（`sync_all`）或至少有读取端校验。
  - [ ] JM 解码缓存键带版本；旧缓存不会被误用。
  - [ ] Rust 三门禁通过。
- **注意**：这会改变 JM 解码缓存键，旧缓存自然失效但不删除；属于预期，不要同时做目录清扫（留给 P3-2）。
- **建议提交**：`fix(cache): ignore empty files and version decoded keys`

#### P1-8 统一空章节错误码（fix，BUG-5）

- **状态**：done（2026-10-27，自动化门禁通过）
- **依赖**：无
- **证据**：`manhuagui.rs:226-230` 用 `Unexpected`，`jm.rs:401-405`、`nhentai.rs:117-121` 用 `NotFound`。
- **目标**：三个平台对“章节没有页”给同一个错误码，前端文案一致。
- **改动**：
  1. `manhuagui::parse_reader_chapter` 在 `pages.is_empty()` 时返回 `AppError::NotFound { resource: format!("manhuagui chapter `{chapter}` pages") }`。
  2. 更新 manhuagui 的测试（若有断言 `Unexpected` 的用例）。
  3. 搜索 `ErrorCode::Unexpected`/`"unexpected"` 相关断言，确认没有其他测试依赖旧行为。
- **验收**：
  - [ ] 三个平台的空页测试都断言 `NotFound`。
  - [ ] `cargo test` 通过。
  - [ ] 前端无需改动（`useErrorMessage` 已有 `notFound` 文案）。
- **建议提交**：`fix(manhuagui): report empty chapters as not found`

---

### P2：结构性拆分（不改外部行为）

> P2 的每一步都应该能单独运行、单独回滚。先做“纯移动/纯提取”，再改行为。阅读器相关任务风险最高，建议放在 P1 全部合并后。

#### P2-1 详情页：显式状态机 + composable 拆分（refactor，BUG-9）

- **状态**：done（2026-10-27，自动化门禁 + E2E 通过；在线详情/收藏长按仍建议手动确认）
- **依赖**：P1-1、P1-2、P2-7（返回来源可后做，接口先留）
- **证据**：`ComicDetailView.vue:55-69` 有 10+ `ref`；在线/下载/历史/收藏四路并发；`678-727` 用 1.5s 定时器做离线回退；`486-537` 下载轮询。
- **目标**：把“在线详情 + 离线回退 + 收藏 + 下载”从视图里抽出来，用显式状态表达来源，不再让定时器直接改 UI。
- **新增文件**：
  - `src/composables/useComicDetail.ts`
  - `src/composables/useFavoriteAction.ts`
  - `src/composables/useDownloadAction.ts`
  - `src/composables/useDetailReturnTarget.ts`（依赖 P2-7）
  - `src/components/comic-detail/DetailTopbar.vue`
  - `src/components/comic-detail/DetailActions.vue`
- **状态模型建议**：

```ts
export type DetailState =
  | { kind: "loading"; fallback: DownloadDetail | null }
  | { kind: "online"; detail: ComicDetail }
  | { kind: "offline"; download: DownloadDetail }
  | { kind: "error"; error: AppError };

// 纯函数，便于单测
export function resolveDetailState(input: {
  online: ComicDetail | null;
  download: DownloadDetail | null;
  error: AppError | null;
  elapsedMs: number;
}): DetailState;
```

- **行为要求**：
  - 在线请求优先；1.5s 后若未返回且本地有已完成章节，进入 `offline` 并显示本地详情。
  - 在线随后返回时替换 `offline` 为 `online`。
  - 在线失败且本地有已完成章节 → `offline`；否则 `error`。
  - 收藏状态查询失败按未收藏处理（保持现状，add 幂等）。
  - 下载轮询只在 `queued/downloading` 时存在，卸载/切作品清理。
  - 长按（500ms）/右键/`Shift+Enter` 打开分组选择器，长按后抑制 click。
- **验收**：
  - [x] `ComicDetailView.vue` 降到 400 行以内（334 行）。
  - [x] 定时器只作为“状态转移的触发条件”，不直接改 `loading/offline` 以外的 UI 状态。
  - [x] `resolveDetailState` 有单测（online 快于/慢于 1.5s、无本地章节、在线失败、在线后到）。
  - [ ] 收藏短按/长按/右键、下载按钮各状态、离线开始/继续阅读手动回归（离线路径 E2E 已过；在线/收藏长按仍待手动）。
  - [x] `pnpm test`、`pnpm build` 通过。
- **手动矩阵**：在线详情；断网但已下载；断网且未下载；加载中下载完成；收藏/取消/移动分组；下载开始→暂停→继续→删除；返回收藏/历史/下载/搜索/首页。
- **建议提交**（拆成多个）：
  1. `refactor(detail): extract detail state resolution`
  2. `refactor(detail): move favorite actions into a composable`
  3. `refactor(detail): move download polling into a composable`
  4. `refactor(detail): extract topbar and actions`

#### P2-2 阅读器：单一 owner + 纯函数边界（refactor，BUG-6/7/8/10）

- **状态**：in progress（2026-10-27：P2-2a 纯函数与边界修复、P2-2b `usePageTurn`、P2-2c 滚轮/键盘、P2-2d `ReaderTrack`、P2-2e `ReaderChrome`、P2-2f 平台兜底已完成；手势 composable 待做）
- **依赖**：P1 全部；建议独立分支
- **证据**：`ComicViewer.vue` 1411 行；`trackOffset` 同时被手势和动画写；边界滑动漏 `snapBack`；页码三源；`?page=last` 依赖 watcher 顺序。
- **目标**：翻页状态只有一个 owner；所有入口（按钮、点击区、滑动、滚轮、方向键）走同一个边界决策；页码/URL/历史同步可测。
- **新增文件**：
  - `src/lib/reader-navigation.ts`（纯函数）
  - `src/composables/usePageTurn.ts`
  - `src/composables/useReaderGestures.ts`
  - `src/composables/useReaderWheel.ts`
  - `src/composables/useReaderKeyboard.ts`
  - `src/components/reader/ReaderTrack.vue`
  - `src/components/reader/ReaderChrome.vue`
- **纯函数建议**：

```ts
export function resolvePage(raw: unknown, total: number): { page: number; atEnd: boolean };
export type BoundaryAction =
  | { kind: "turn"; page: number }
  | { kind: "chapter"; chapterId: string; position: "start" | "end" }
  | { kind: "detail" };
export function boundaryAction(input: {
  direction: "previous" | "next";
  page: number;
  total: number;
  previousChapterId: string | null;
  nextChapterId: string | null;
}): BoundaryAction;
```

- **拆分边界**：
  - `usePageTurn`：唯一拥有 `displayPage`、`trackOffset`、`turnTarget`、`trackAnimating`、`turnToken`、`turnTimer`；对外 `turnTo/cancel/snapBack/syncFromProp`。
  - `useReaderGestures`：pointer/pinch/tap；只产出 `tapZone`、`swipe`、`pan`、`pinch` 意图，不直接改页码。
  - `useReaderWheel` / `useReaderKeyboard`：把滚轮/按键映射成 `turn/boundary/zoom/close` 意图。
  - `ReaderChrome.vue`：上/下栏、页码、电量、缩放/全屏按钮、章节按钮。
  - `ReaderTrack.vue`：三槽位渲染与尺寸回填。
- **必须修的行为**：
  1. 边界滑动/点击/滚轮/键盘在切章前统一 `snapBack()` 或完成退出动画，不能留下 `trackOffset` 错位。
  2. `trackAnimating` 期间忽略新的指针移动/滚轮/键盘翻页，或排队一次。
  3. `?page=last` 在 `ComicReaderView` 加载完成时一次性解析成真实页码，再写 URL/历史；不要依赖两个 watcher 的注册顺序。
  4. `viewerPlatform` 不再回退 `"nhentai"`：`ComicViewer` 只在 `platform` 非空时渲染，或接受可空并在内部不发起图片请求。
- **验收**：
  - [ ] `resolvePage`、`boundaryAction` 单测覆盖首/尾页、单章、多章、无上一/下一章、非法 query。
  - [ ] 翻页只有一个“写 page”的入口；`ComicViewer` 不再同时被 props 和内部动画回写。
  - [ ] 所有入口的边界行为一致；首章第一页上一页回详情、末章最后一页下一页回详情。
  - [ ] `?page=last` 不出现在最终 URL；刷新/深链行为与现状一致。
  - [ ] 手动回归：ltr/rtl/ttb、点击三区、滑动、滚轮、方向键、双指缩放、缩放后平移、全屏、菜单显隐、电量 HUD、跨章预加载。
  - [ ] `pnpm test`、`pnpm build` 通过。
- **风险/回滚**：阅读器是高风险区，按“纯函数 → usePageTurn → 手势 → Chrome”分 4–5 个提交；每个提交都能单独回滚。不要一次搬完 1400 行。
- **建议提交**：
  1. `refactor(reader): add pure page-boundary helpers`
  2. `refactor(reader): make usePageTurn own the turn state`
  3. `refactor(reader): extract pointer and pinch gestures`
  4. `refactor(reader): extract chrome and track components`

#### P2-3 `downloads.rs` 模块化（refactor）

- **状态**：todo
- **依赖**：P1-4、P1-6（先把行为修稳，再做纯移动）
- **目标**：把 2231 行拆成职责清晰的模块，worker 与 store 单向依赖。
- **目标结构**：

```text
src-tauri/src/downloads/
  mod.rs        // 常量、re-export、命令注册（lib.rs 的 mod downloads 不变）
  model.rs      // DTO、DownloadStatus、校验/键规范化
  index.rs      // downloads.json 的 load/save/backup（复用 storage）
  store.rs      // 纯内存状态机、summary、分页；不碰 fs/网络
  local.rs      // manifest -> ReaderChapter
  worker.rs     // download_work/run_download/spawn/cover
  commands.rs   // #[tauri::command]
  tests.rs      // 或各模块内 #[cfg(test)]
```

- **执行方式**：
  1. 第一次提交只做“剪切 + `pub(crate)` 可见性 + `use` 修正”，不改逻辑；`cargo test` 必须与移动前一致。
  2. 第二次提交再拆 worker 里的 `ensure_cover`：改为接收 `&DownloadsStore`/`&HttpClient`/路径参数，不再从 `app.state::<DownloadsStore>()` 反查。
  3. 第三次提交把章节完成/失败用 `enum ChapterOutcome { Completed { pages }, Failed { failure } }` 表达，减少 `Option<DownloadFailure>` + 提前 `?` 的分支。
- **不要做**：不要顺手改状态机语义、不要改 `downloads.json` 字段、不要把 `local.rs` 的 manifest 读取改成缓存。
- **验收**：
  - [ ] `downloads.rs` 不再存在（或只剩 `mod.rs` 的 re-export）。
  - [ ] 命令名、参数、返回类型、错误码不变。
  - [ ] 现有测试全部通过；Rust 三门禁通过。
  - [ ] `lib.rs` 的 `generate_handler!` 不需要改（命令路径仍在 `downloads::`）。
- **建议提交**：`refactor(downloads): split store, worker and commands`

#### P2-4 Platform 能力拆分 + 镜像健康语义（refactor/fix，BUG-4）

- **状态**：todo
- **依赖**：P1-5
- **证据**：`platform/mod.rs:93-185` 有 15 个方法，`generic` 只实现 `id/configure`，其余能力默认返回 `invalidInput`；`note_request_outcome` 在 HTTP 2xx 时就算成功（`http.rs:642-646`）。
- **目标**：把“图片请求 profile”和“可搜索/详情/阅读的 catalog”分开；镜像健康以“解析成功”为准。
- **建议结构**：
  - `trait ImageProfile`：`id`、`cache_version`、`matches`、`configure`、`cache_key_url`、`image_fallbacks`、`image_decode`（或 `decode_image`）。
  - `trait Catalog`：`prepare`、`search`/`detail`/`reader` 的 request + parse。
  - 静态表：`PROFILES: [&dyn ImageProfile; 4]`、`CATALOGS: [&dyn Catalog; 3]`；`find_profile`/`find_catalog`。
  - 通用命令用 `find_catalog(platform)`，找不到时返回 `invalidInput`（与现在 `unsupported` 文案保持一致或统一）。
- **镜像健康修复**：
  - 方案 A（推荐）：`fetch_document` 只返回 `{ url, body }`，命令在 `parse_*` 成功后才 `note_request_outcome(url, true)`；解析失败调用 `false`。
  - 方案 B：给 `fetch_document` 传一个回调，由调用方决定成功/失败。
  - 不要再用“HTTP 2xx 非空”代表成功。
- **测试**：
  - 假 `ImageProfile`/`Catalog`：能力缺失时返回 `invalidInput`；`prepare` 顺序由 P1-5 的 helper 保证。
  - 镜像：解析失败不应把当前 host 记为 preferred（可用 mock/单元测试策略，或至少把判断逻辑抽成纯函数测试）。
- **验收**：
  - [ ] `generic` 不需要实现搜索/详情/阅读的空方法。
  - [ ] 新增平台只实现 `ImageProfile` 也能抓图；实现 `Catalog` 才具备搜索/详情/阅读。
  - [ ] JM 首选镜像不会因 200 挑战页被钉死。
  - [ ] 现有 platform 测试、搜索/详情/阅读手动冒烟通过。
- **风险**：会触碰所有平台文件；建议放在 P2-3 之后，避免与 downloads 拆分冲突。

#### P2-5 `http.rs` 拆分 + ImageCache（refactor）

- **状态**：todo
- **依赖**：P2-4（接口稳定后）
- **目标**：把代理、client、ECH、抓取、镜像策略分开；缓存从自由函数变成可注入的对象。
- **目标结构**：
  - `image/http/mod.rs`：`HttpClient`、re-export。
  - `image/http/client.rs`：base builder、proxy 校验、header。
  - `image/http/ech.rs`：ECH runtime、refresh、fallback 状态机。
  - `image/http/fetch.rs`：`download_image`、`fetch_document`、`read_limited`、`is_mirror_error`。
  - `image/cache.rs`：`struct ImageCache { base: PathBuf }`，方法 `lookup/write/sweep`。
- **可顺手做的去重**：
  - `read_limited` 合并 `download_image` 与 `fetch_document_once` 的 status/length/empty 检查。
  - `MirrorPolicy` 独立，避免 `is_mirror_error` 与平台逻辑混在一起。
  - 清掉 `http.rs:247/258` 的 `eprintln`，改走 `logging`。
  - `ImageCache` 的 base dir 在 `setup` 或 `OnceLock` 解析一次，避免每次 `fetch_image_inner` 都 `app.path()`。
- **验收**：
  - [ ] `http.rs` 降到 400 行以内，职责单一。
  - [ ] `http` 与 `ech` 不再互相 `use` 成环（或明确单向依赖）。
  - [ ] 缓存行为、缓存键、扩展名、版本语义不变（P1-7 的改动除外）。
  - [ ] ECH/代理互斥、启动门禁、plain fallback 手动回归。
  - [ ] Rust 三门禁通过。

#### P2-6 IPC 单入口 + ComicThumb 复用（refactor，BUG-11 的前置）

- **状态**：todo
- **依赖**：无（可在 P1 后做）
- **证据**：`ComicThumb.vue:42` 直接 `invoke`，重复定义 `CachedImage`；`waitForEchStartup` 散落在 comics/images/downloads/reader/ComicThumb。
- **目标**：所有 IPC 走一个薄封装，ECH 等待策略显式；缩略图复用 `fetchCachedImage`。
- **新增文件**：`src/api/client.ts`

```ts
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
  options: { waitForEch?: boolean } = {},
): Promise<T> {
  if (options.waitForEch ?? true) {
    await waitForEchStartup();
  }
  return invoke<T>(command, args);
}
```

- **迁移范围**：
  - `comics.ts`、`images.ts`、`reader.ts`、`downloads.ts`、`favorites.ts`、`history.ts`、`ech.ts`、`proxy.ts`、`logs.ts`。
  - 明确 `waitForEch: false` 的命令：`get_downloaded_chapter`、`pause_download`、`delete_download`、本地设置类命令（`set_logging_enabled` 等）。不要改变现有语义。
  - `ComicThumb` 改用 `fetchCachedImage`，删除本地 `CachedImage` 接口。
- **验收**：
  - [ ] `grep "invoke(" src` 只出现在 `api/client.ts` 和测试里（`convertFileSrc` 不算）。
  - [ ] 缩略图走 in-flight 合并与已解析路径缓存；断网/防盗链行为不变。
  - [ ] 现有测试更新 mock 后通过。
- **注意**：`invokeCommand` 只负责“等待 + 调用”，不要在里面吞错或自动 `toAppError`；错误规范化仍在调用点或 `useErrorMessage`。

#### P2-7 返回来源显式化（refactor，BUG-12）

- **状态**：todo
- **依赖**：P2-1（详情返回目标）
- **证据**：`useBackNavigation.ts:10-24` 读 `window.history.state.back`，是 Vue Router 的实现细节。
- **目标**：从收藏/历史/下载/搜索进入详情时，把来源写进导航 state；`previousPath` 只做兜底。
- **改动**：
  1. 列表页 `RouterLink` 的 `to` 增加 `state: { from: "favorites" | "history" | "downloads" }`。
  2. 搜索结果进详情时 `state: { from: "search" }`；首页进详情时 `state: { from: "home" }`。
  3. `useDetailReturnTarget` 先读 `window.history.state.from`，再回退 `previousPath()`。
  4. 保持 `useBackNavigation` 的“上一页等于目标则 `router.back()`，否则 `replace`”语义。
- **测试**：
  - 现有 `useBackNavigation.test.ts` 扩展：有 `state.from`、无 state、深链、直接访问 `/comic/...`。
- **验收**：
  - [ ] 从收藏/历史/下载/搜索/首页进详情，返回都落到来源页。
  - [ ] 深链/刷新详情，返回仍回首页或搜索（按现状）。
  - [ ] 不出现“返回又回到刚离开的页面”的栈问题。
  - [ ] `pnpm test`、`pnpm build` 通过。

---

### P3：契约与长期项

#### P3-1 Rust ↔ TS 类型生成/契约测试（chore）

- **状态**：todo
- **依赖**：P2-6（IPC 收口后）
- **目标**：减少手抄类型漂移（如 `ReaderChapter.thumb`）。
- **做法**（先 spike，不要直接引入重依赖）：
  - 评估 `ts-rs`（serde 驱动）或 `specta`（Tauri 生态）与当前 Tauri 2/Vite 8 的兼容性。
  - 如果生成器侵入太大：至少加“wire 字段快照”测试（Rust 侧序列化 fixture → JSON；前端侧类型测试/快照对照），并在 AGENTS.md 记录如何更新。
- **验收**：
  - [ ] 一条命令可重新生成类型（或一份契约测试可在字段漂移时失败）。
  - [ ] 生成文件不手改；构建脚本/文档同步。
  - [ ] 不改变现有 `api/*.ts` 的公开导出（或提供兼容 re-export）。

#### P3-2 图片缓存清扫（feat）

- **状态**：todo
- **依赖**：P1-7、P2-5
- **目标**：给可丢弃缓存加容量/年龄上限，版本 bump 后清旧目录。
- **做法**：
  - `ImageCache::sweep(max_bytes, max_age)`；启动后后台执行，避免阻塞首屏。
  - 删除旧 `CACHE_VERSION`/旧平台 `cache_version` 目录；或按“最后访问时间”淘汰。
  - 下载目录不在清扫范围（`app_data_dir()/downloads/` 是用户数据）。
- **验收**：
  - [ ] 缓存超过上限会降下来；正在使用的文件不会被删（同一进程内至少按 mtime/最后访问）。
  - [ ] 清扫失败只记日志，不影响阅读。
  - [ ] 单测用临时目录。

#### P3-3 分页索引（conditional）

- **状态**：todo（仅在有性能证据时做）
- **目标**：`list_downloads`/`list_favorites`/`list_history` 目前每次 collect + sort 全量；数据量大时再优化。
- **做法**：在 store 内维护按 `addedAt/updatedAt` 排序的 `Vec<Key>`，写时增量更新；或接受现状，记录 benchmark。
- **验收**：有 benchmark/ profiling 数据；不改变分页结果顺序。

#### P3-4 下载 worker 可测化（test）

- **状态**：todo
- **依赖**：P2-3
- **目标**：不联网也能测“章节失败/暂停/完成/旧 run id”路径。
- **做法**：把“取章节页源 + 取图片”抽成 trait/函数注入；worker 只做编排。补测：单页失败、章节失败后继续、暂停后停止、旧 run id 写入被拒。
- **验收**：新增测试不需要网络/真实 Tauri runtime。

---

## 6. 验证矩阵

### 6.1 每次提交的最低门禁

| 改动范围 | 命令 |
| --- | --- |
| 前端 | `pnpm test` + `pnpm build` |
| Rust | `cargo test` + `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` |
| 前端 + Rust | 上面两组都跑 |
| 路由/阅读器/下载/收藏/图片 | 额外 `pnpm tauri dev` 手动冒烟 |

### 6.2 手动冒烟清单（按任务选做）

**列表页（P1-1）**

- [ ] 历史/收藏/下载各翻 2 页，页码、总数、滚动回顶正确。
- [ ] 接口失败显示错误与重试；重试后恢复。
- [ ] 下载进行中，下载页 1s 轮询不闪烁、不重置滚动。
- [ ] 收藏删除最后一条后，页码夹取正确。
- [ ] 分组筛选后分页/计数正确。

**详情页（P2-1）**

- [ ] 在线详情正常；骨架屏布局不跳。
- [ ] 断网 + 已下载：1.5s 内显示本地详情，在线恢复后替换。
- [ ] 断网 + 未下载：错误态 + 返回。
- [ ] 收藏短按、长按、右键、`Shift+Enter`；移动分组。
- [ ] 下载开始/暂停/继续/完成/删除按钮状态。
- [ ] 返回收藏/历史/下载/搜索/首页。

**阅读器（P2-2）**

- [ ] ltr/rtl/ttb 三种方向；点击左/中/右；滑动；滚轮；方向键。
- [ ] 首章第一页上一页回详情；末章最后一页下一页回详情；中间章跨章。
- [ ] `?page=` 写入；刷新恢复；`?page=last` 不出现在最终 URL。
- [ ] 双指缩放 1x–4x、缩放后单指平移、缩放结束不误触翻页。
- [ ] 菜单显隐、页码 HUD、电量显示、全屏。
- [ ] 跨章预加载不阻塞、失败静默。
- [ ] JM 页图还原无错位；manhuagui 页图去 `.webp`；nhentai 页图正常。

**下载（P1-4/P1-6/P2-3）**

- [ ] 三平台各下载一本；暂停/继续/删除。
- [ ] 下载中打开下载页/详情页，轮询正常，UI 不卡。
- [ ] 重启后 active 状态变 paused，不自动联网。
- [ ] 暂停→删除→重新下载，旧 worker 不污染新记录（观察日志/进度）。
- [ ] 断网阅读已下载章节；删除后阅读器不再命中旧本地页。

**JM（P1-5）**

- [ ] 重启应用后第一次 JM 搜索成功；详情/阅读也成功。
- [ ] 开启日志时，能看到版本同步与后续请求。

**缓存（P1-7）**

- [ ] 手动放一个零字节缓存文件，重新请求会重新下载而不是显示坏图。
- [ ] 修改解码版本常量后，旧 JM 解码缓存不被复用（预期会重新下载）。

### 6.3 E2E

`pnpm test:e2e` 只覆盖 `e2e/` 预置的离线数据，不访问上游；需要 `tauri-driver` 和匹配的 `msedgedriver.exe`。没有 CI，是否跑由改动范围决定；涉及下载/收藏/离线的重构建议至少跑一次。

---

## 7. 提交与协作

- **分支**：建议每个阶段一个分支（如 `refactor/p1-foundations`、`refactor/p2-reader`）；小任务也可直接在当前分支逐个提交。
- **提交粒度**：一个任务卡一个提交；P2-1/P2-2/P2-3 内部按“纯提取 → 行为修复”拆多个提交。
- **提交信息示例**：

```text
refactor(web): extract paged resource composable
refactor(web): add latest-request guard
refactor(rust): share atomic JSON storage helpers
fix(downloads): run blocking index work off the async runtime
fix(jm): prepare the app version before building requests
fix(downloads): allocate monotonic run ids
fix(cache): ignore empty files and version decoded keys
fix(manhuagui): report empty chapters as not found
refactor(reader): add pure page-boundary helpers
```

- **不要提交**：`node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/schemas/`、`src-tauri/gen/android/**/build/`、`e2e/.drivers/`、未跟踪的编辑器文件。
- **依赖变化**：同时提交 `pnpm-lock.yaml` 与 `src-tauri/Cargo.lock`；本计划原则上不新增依赖。
- **文档同步**：
  - 新增/修改命令、IPC 契约、错误码、持久化格式 → 更新 [AGENTS.md](AGENTS.md)。
  - 新增测试体系/脚本 → 更新 [AGENTS.md](AGENTS.md) 的验证章节与 `package.json`。
  - 本文件的任务状态/执行备注在完成时更新。

---

### 7.1 文档同步清单

纯重构不需要改 AGENTS.md；但只要**目录结构、模块边界、命令/参数、返回来源机制、测试方式**发生变化，就必须同步，否则下一个智能体会按旧地图走错路。

| 任务 | 需要同步的文档 |
| --- | --- |
| P1-3 | 新增 `src-tauri/src/storage.rs` 后，在 AGENTS.md 代码地图补一行（若保留该模块）。 |
| P1-4 / P1-6 | 下载的暂停/删除/续传语义不变则不必；若改变，更新“下载”章节。 |
| P2-3 | AGENTS.md 代码地图的 `downloads.rs` → `downloads/`，以及“改下载”入口的表项。 |
| P2-4 | AGENTS.md 的平台 trait 描述、`image/platform/` 说明、新平台入口。 |
| P2-5 | AGENTS.md 中所有 `image/http.rs` 引用（ECH、网络、缓存）改为新模块路径。 |
| P2-6 | AGENTS.md 的 `src/api/` 说明（单一 IPC 入口）、测试 mock 说明。 |
| P2-7 | AGENTS.md 的“从收藏/历史/下载进详情，返回来源用 `previousPath()`”段落，改为 state 优先、`previousPath` 兜底。 |
| P3-1 | 类型生成的命令、生成文件位置、构建/测试章节。 |
| P3-2 | 缓存清理策略与“缓存没有自动清理”这条易错点；若实现了上限，必须改。 |

## 8. 完成定义（DoD）

### P1 完成

- [x] P1-1 ~ P1-8 全部状态改为 done。
- [x] `pnpm test`、`pnpm build`、`cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 全绿（2026-10-27）。
- [x] `pnpm test:e2e` 4/4 通过（app shell、favorites 分组筛选、offline reader、settings 持久化）。
- [ ] 历史/下载列表分页、JM 首次联网请求、真实零字节缓存仍需一次 `pnpm tauri dev` 手动确认；自动化测试已覆盖缓存与 JM 顺序的核心逻辑。
- [x] 没有新增 lint warning；没有改变外部契约。

### P2 完成

- [ ] 详情页/阅读器/downloads/http 的拆分完成，单文件行数明显下降。
- [ ] 所有边界/离线/下载竞态手动回归通过。
- [ ] 新增纯函数/状态机测试；测试不依赖网络和真实 Tauri runtime。
- [ ] 若拆分导致接口变化，AGENTS.md 已同步。

### 整体完成

- [ ] 任务卡状态全部更新；偏离项有“执行备注”。
- [ ] 没有遗留 TODO/FIXME 被悄悄加入（若必须留，写明原因和后续任务 ID）。
- [ ] 下一次改动（新平台/新列表/新下载能力）可以只改一处，不需要再复制三份。

---

## 9. 证据索引（基线）

| 主题 | 证据 |
| --- | --- |
| 列表页重复 | `HistoryView.vue:14-110`、`FavoritesView.vue:31-225`、`DownloadsView.vue:23-214` |
| 平台标签四份 | `HistoryView.vue:36`、`FavoritesView.vue:92`、`DownloadsView.vue:53`、`DetailShell.vue:19` |
| requestId 26 处 | `ComicDetailView.vue:438`、`DownloadsView.vue:226`、`ComicReaderView.vue:224`、`FavoritesView.vue:148`、`ComicThumb.vue:32` 等 |
| 详情页状态 | `ComicDetailView.vue:55-69`、`678-727`、`486-537` |
| 阅读器状态 | `ComicViewer.vue:73-77`、`168-231`、`552-613`、`610-611`、`661-666`、`316-327` |
| 页码/URL | `ComicReaderView.vue:228`、`318-345`、`64-66` |
| 缩略图绕层 | `ComicThumb.vue:42-45`、`images.ts:58-88` |
| 返回来源 | `useBackNavigation.ts:10-24` |
| downloads 结构 | `downloads.rs:263-914`、`916-1010`、`1281-1305`、`1456-1735`、`1746-1855` |
| downloads 阻塞 IO | `downloads.rs:1746-1855`、`439-456`；对比 `favorites.rs:754`、`history.rs:404` |
| run_id | `downloads.rs:139-142`、`166`、`349`、`389`、`559-565` |
| 持久化重复 | `favorites.rs:588-638`、`downloads.rs:1281-1305`、`history.rs:339-354` |
| Platform trait | `image/platform/mod.rs:93-185`；`image/mod.rs:234-236`、`280-282`、`337-339` |
| JM 签名 | `jm.rs:83`、`147-158`；`jm_api.rs:127-129` |
| 镜像健康 | `http.rs:642-646`；`jm.rs:118-136` |
| 缓存 | `cache.rs:31-43`、`61-74`；`image/mod.rs:110-112` |
| 空章节错误码 | `manhuagui.rs:226-230`、`jm.rs:401-405`、`nhentai.rs:117-121` |
| 契约类型 | `reader.ts:6-30`、`model.rs:234-262`、`history.ts:9/23` |

---

## 10. 执行备注（由执行者填写）

> 每完成一个任务，或在执行中偏离计划，在这里追加一行：日期 / 任务 ID / 结论 / 未完成原因。不要删除历史记录。

- 2026-10-27 / 计划创建 / 基于当前工作区静态阅读与三路只读审阅；未改动业务代码。
- 2026-10-27 / P1-1..P1-8 / 代码完成。自动化验证：`pnpm test`（82 passed）、`pnpm build`、`cargo test`（158 passed，10 ignored）、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 全绿；`pnpm test:e2e` 4/4 passage（app shell、favorites 分组筛选、offline reader、settings 持久化）。未覆盖的手动项：历史/下载列表分页、JM 首次联网请求、真实零字节缓存场景（后两者已有单元测试，但仍建议真实窗口确认）。
- 2026-10-27 / P1-1 / 新增 `src/composables/usePagedResource.ts`、`src/i18n/format.ts`、`src/i18n/platform.ts`；三个列表页不再各自维护分页、时间格式化、字节格式化和平台标签。`DownloadsView` 仍保留轮询，`FavoritesView` 仍保留分组 CRUD。
- 2026-10-27 / P1-2 / `useLatestRequest` 支持 `begin`/`isCurrent`/`snapshot`/`invalidate`；`ComicDetailView` 的轮询用 `snapshot()` 读取当前 token，避免轮询反过来使详情请求失效。`ComicThumb` 与 `useCachedImage` 仍保留局部整数守卫（计划中的记录在案例外）。
- 2026-10-27 / P1-4 / `read_local_chapter` 从 async + `tokio::fs` 改为同步函数，下载命令统一走 `spawn_blocking`；删除仍持 data 锁执行 `remove_dir_all`，但不再占用 async runtime。
- 2026-10-27 / P1-6 / 顺带修复新记录被 `is_active()` 短路的问题：新下载首次 `run_id` 由 0 改为 1，之后由 `next_run_id` 单调分配；删除后重新下载不会复用旧 id。
- 2026-10-27 / P1-7 / JM 解码缓存键从 `?decoded` 变为 `?decoded=v1`，旧解码缓存自然失效但不删除（目录清扫留给 P3-2）。
- 2026-10-27 / P2-1 / `ComicDetailView.vue` 从 983 行降到 334 行：新增 `detailState.ts`（纯函数 + 5 个单测）、`useComicDetail`（在线/离线、下载索引、历史、1.5s 回退）、`useFavoriteAction`（短按/长按/分组选择器）、`useDownloadAction`（开始下载、1.5s 轮询、跳下载页）、`useDetailReturnTarget`，以及 `DetailTopbar`/`DetailActions`。验证：`pnpm test` 87 passed、`pnpm build`、`pnpm test:e2e` 4/4 spec（含离线详情与阅读器）。未覆盖：真实在线详情、收藏长按/右键、下载进行中的按钮状态，建议真实窗口手动确认。
- 2026-10-27 / P2-2a / 新增 `src/lib/reader-navigation.ts`（`resolveReaderPage`/`readerBoundaryAction`，8 个单测）；`ComicReaderView` 统一 `?page=last`、上一章/下一章和边界策略，query watcher 不再把 `last` 提前解析成 1；`ComicViewer.leaveAtBoundary` 先 `cancelTurn()` 清掉边界拖拽的 `trackOffset`，动画期间忽略 pointer move/up/cancel 对轨道的写入。验证：`pnpm test` 95 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 剩余：`usePageTurn`、手势/wheel/keyboard 拆分、`ReaderChrome`/`ReaderTrack`、`viewerPlatform` 假兜底。
- 2026-10-27 / P2-2b / 新增 `usePageTurn`（7 个单测）：`displayPage`/`trackOffset`/`turnTarget`/动画 token/边界标志只有一个 owner，`ComicViewer` 从 1434 行降到 1282 行；手势、滚轮、键盘、缩放和 3 槽轨道仍留在组件里，但翻页统一走 hook。验证：`pnpm test` 102 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 剩余：手势/wheel/keyboard composable、`ReaderChrome`/`ReaderTrack`、`viewerPlatform` 假兜底。
- 2026-10-27 / P2-2c / 新增 `useReaderWheel`/`useReaderKeyboard`（各 5 个单测）：滚轮阈值/节流、Ctrl+wheel 缩放、方向键映射、Home/End、缩放快捷键、全屏、Escape 和可编辑目标忽略保持原行为；组件只保留监听注册与移除。验证：`pnpm test` 112 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 剩余：手势 composable、`ReaderChrome`/`ReaderTrack`、`viewerPlatform` 假兜底。
- 2026-10-27 / P2-2d / 新增 `ReaderTrack.vue`（三槽位、方向类、slot 尺寸事件、轨道样式）；`ComicViewer` 从 1168 行降到 1086 行，模板只保留 stage 手势和空态。验证：`pnpm test` 112 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 剩余：手势 composable、`ReaderChrome`、`viewerPlatform` 假兜底。
- 2026-10-27 / P2-2e / 新增 `ReaderChrome.vue`（顶栏、底栏、HUD、sr-only 页码、电量），chrome 显隐改为组件内的 `viewer__bar--hidden` 类；`ComicViewer` 从 1086 行降到 680 行，累计从 1434 行降到 680 行。验证：`pnpm test` 112 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 剩余：手势 composable、`viewerPlatform` 假兜底。
- 2026-10-27 / P2-2f / 删除 `viewerPlatform` 的 `"nhentai"` 假兜底：`ComicReaderView` 只在 `chapter`、非空页和 `platform` 同时成立时渲染 `ComicViewer`，并传真实平台。验证：`pnpm test` 112 passed、`pnpm build`、`pnpm test:e2e` 4/4。P2-2 仅剩手势 composable。
