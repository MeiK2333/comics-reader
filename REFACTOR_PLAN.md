# 重构与修复计划（待办）

> **状态**：P1、P2-1~P2-8、P3-2、P3-4、P4-4 已完成并从本文件移除（历史见 git log）。剩余：P4-1~P4-3、P4-5~P4-7（主线）、P4-8（低优先）、P4-9（条件项）、P3-1（可选）、P3-3（条件项）。
> **基线**：2026-10-27，master（P4-4 后）；`pnpm test` 25 files / 136 tests、`cargo test` 169 passed / 10 ignored。
> **适用对象**：后续接手本仓库的编码智能体。
> **配套文档**：[AGENTS.md](AGENTS.md)（契约、命令、易错点，优先于本文件）、[ROADMAP.md](ROADMAP.md)（功能方向）。
> **定位说明**：任务卡里的行号是基线行号，代码变动后会漂移；定位时按符号名（函数/结构体/常量）搜索，不要机械相信行号。

---

## 0. 一页摘要

主体重构已经结束。当前维护成本集中在：

1. **测试盲区**（P4 主线）：阅读器手势/缩放、跨章预加载、收藏动作与详情数据层、IPC/缓存/搜索守卫还没有直接单测。
2. **少量叶子重复**：`KNOWN_PLATFORMS` 三处（P4-8）；其余重复已记录为例外，不要强行统一。
3. **按需项**：P3-1 其余 DTO 契约只在 wire 结构变化时补；P3-3 分页索引只在有 benchmark 时做。

执行顺序：

1. **P4-1 → P4-2 → P4-3 → P4-5 → P4-6 → P4-7**；P4-8 低优先，P4-9 条件项。
2. P3-1 随 wire 变化推进；P3-3 有性能证据再做。

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

记录在案的例外（不是待办）：`KNOWN_PLATFORMS` 三处；`LogsView`/`NhentaiDetail` 各自构造 `Intl.DateTimeFormat`；`search`/`ComicThumb`/`useCachedImage`/`usePagedResource` 的局部 `requestId`。只有 `KNOWN_PLATFORMS` 由 P4-8 处理。

### 2.2 无前端测试的高风险模块（P4 依据）

| 模块 | 行数 | 为什么高风险 |
| --- | ---: | --- |
| `src/composables/useReaderGestures.ts` | 406 | 手势/双指/防误触全部内联；E2E 只覆盖基础滑动与点击区 |
| `src/composables/useReaderZoom.ts` | 122 | 缩放锚点与边界 clamp 是数学核心 |
| `src/composables/useNextChapterPreload.ts` | 93 | 跨章预热链路、去重与失败静默 |
| `src/composables/useFavoriteAction.ts` | 186 | 短按/长按/分组/切换作品 |
| `src/composables/useComicDetail.ts` | 286 | 在线详情、下载索引、历史、续读回退 |
| `src/composables/useDownloadedComicDetail.ts` | 204 | 已下载详情与本地续读 |
| `src/api/client.ts` | 35 | IPC 单入口与 ECH 等待约定 |
| `src/composables/useCachedImage.ts` | 102 | 局部 requestId 例外 |
| `src/stores/search.ts` | 90 | 局部 requestId 例外；URL-free 搜索会话 |

这些模块合计约 1,500 行。P4 不按覆盖率数字推进，按对应卡片的行为矩阵补测试。

### 2.3 验证基线

```powershell
# 仓库根目录
pnpm test        # 25 files / 136 tests
pnpm build       # vue-tsc --noEmit + Vite build

# src-tauri/
cargo test       # 169 passed, 10 ignored
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

### P4：测试补强与局部修复（test/fix）

> **范围**：前端为主。目标不是提高覆盖率数字，而是把“只能靠手点、真机或联网”的交互逻辑变成可重复的自动化。
> **原则**：先抽纯函数、再测组合式；不为可测性改公开 API、IPC 或路由；测试不联网、不启动 Tauri；测试暴露的行为缺陷单独用 `fix` 提交。
> **非目标**：不改路由/持久化/wire/错误码/i18n；不重做阅读器三槽轨道；不拆大文件；不引入生成器或新依赖；e2e 不在 P4 门禁内。

#### P4-0 测试约定（所有 P4 卡片共用）

- 组件/组合式测试跑在 jsdom：`vitest.config.ts` 已注入 `src/test/setup.ts`；IPC 用 `vi.mock` 或 `@tauri-apps/api/mocks`，禁止真实网络。
- `useMessage()` 依赖 `NMessageProvider`；测试用 `defineComponent` + `h(NMessageProvider, …)` 包一层 harness，不要为了测试把 message 改成参数。
- jsdom 有 `PointerEvent` 构造函数，但没有 `setPointerCapture`/`releasePointerCapture`/`hasPointerCapture`；手势测试在 stage 元素上 stub 这三个方法，并用普通对象伪造事件（hook 只读 `pointerId`/`pointerType`/`button`/`clientX`/`clientY`/`target`/`preventDefault`）。
- 需要布局的测试（手势、缩放）用 `Object.defineProperties` 设置 `clientWidth`/`clientHeight`，并覆盖 `getBoundingClientRect`（jsdom 默认全为 0）。
- 组合式优先包在 `effectScope()` 里，结束后 `scope.stop()`；否则 `onScopeDispose` 的清理会在测试之间泄漏。
- Pinia store 测试按 `src/stores/settings.test.ts` 的模式：先 `createApp(...).use(pinia)`，再创建 store，否则持久化插件不会执行。
- 测试文件与源码同级（`src/**/*.test.ts`），命名沿用现有风格，不新建 `__tests__/` 目录。

#### P4-1 阅读器手势决策可测化（test/refactor）

- **状态**：todo
- **依赖**：无
- **证据**：`src/composables/useReaderGestures.ts` 约 406 行、0 测试；tap 三区、ltr/rtl/ttb 方向映射、滑动阈值、边界限幅、缩放后平移与双指 pinch 全在 pointer 回调里内联判断；`e2e/specs/offline-reader.e2e.js` 只覆盖左右滑动、点击区和键盘。方向映射在 `onStagePointerMove` 与 `onStagePointerUp` 各写了一遍，已有漂移风险。
- **目标**：把“由方向/页码/阈值/缩放状态决定的行为”抽成 `src/lib/reader-gestures.ts` 的纯函数，hook 只保留事件编排与指针状态。
- **建议纯函数**（命名可调整，保持单一职责）：
  - `tapZone(ratio: number): "left" | "middle" | "right"`（含边界值）
  - `isForwardGesture(direction, horizontal, primaryDelta): boolean`
  - `swipeTarget(input: { displayPage, total, forward }): number | "boundary"`
  - `dragLimit(stageSpan, atBoundary): number`
  - `isTapSuppressed(input: { moved, pinched }): boolean`
- **测试文件**：`src/lib/reader-gestures.test.ts` + `src/composables/useReaderGestures.test.ts`
- **用例**：
  - ltr 左滑 → `turnTo(page+1)`；ltr 右滑 → `turnTo(page-1)`；rtl 相反；ttb 上滑/下滑映射，横向轴不参与。
  - 点击左/中/右三区：ltr 与 rtl 下左右区含义相反，中间区 `toggleChrome()`；`ratio` 恰好在 1/3、2/3 时归入哪一侧要有断言。
  - 滑动小于阈值 → `snapBack()`，不翻页；首/尾页继续滑动 → 调用 `nextPage()`/`previousPage()`（由视图层切章/回详情），不调用 `turnTo`。
  - `scale > 1` 时拖拽只 `panBy`，松开不触发 tap/翻页；双指 pinch 调用 `panBy` + `setScale`，松开一指后剩余指针按平移处理且不误触。
  - 动画中（`isAnimating()` 为 true）忽略 down/move/up；`pointercancel` 归位；按钮/链接等交互目标忽略指针事件。
- **不要做**：不要改 `usePageTurn` 的 owner 语义或三槽轨道；不要把 tap 判定改成基于 pointerdown 坐标（当前以释放点为准，先测现状）。
- **验收**：
  - [ ] 手势决策不再以裸 `if` 散落在 hook 中；纯函数有边界值测试。
  - [ ] 现有 E2E 覆盖的行为（滑动/点击区/键盘）不被改变；`pnpm test`、`pnpm build` 通过。
  - [ ] 新增测试不依赖真实 PointerEvent 实现，可在 jsdom 稳定运行。
- **建议提交**：
  1. `refactor(reader): extract gesture decision helpers`
  2. `test(reader): cover pointer gestures and tap zones`

#### P4-2 缩放/平移单测（test）

- **状态**：todo
- **依赖**：无（可与 P4-1 并行）
- **证据**：`src/composables/useReaderZoom.ts` 约 122 行、0 测试；锚点缩放、边界 clamp、`reset` 与 `reflow` 是双指缩放的数学核心。
- **目标**：给缩放/平移的 clamp 与锚点计算补单测；若发现边界错误，单独 `fix` 提交。
- **用例**：
  - `setScale` 夹在 1–4；目标为 1 时 offset 归零；`zoomStep` 按 0.25 步进；`percent` 四舍五入。
  - 以光标为中心缩放时，图像上的同一点保持在光标下（固定 `getBoundingClientRect` + `clientWidth/Height` 断言 offset）。
  - `panBy` 在图像小于视口时 offset 为 0；放大后平移被夹在 `(image * scale - viewport) / 2`。
  - `panBy`/`reflow` 在 stage 或 imageSize 缺失时把 offset 归零。
- **验收**：
  - [ ] `pnpm test`、`pnpm build` 通过；不改变 `ComicViewer` 的调用方式。
- **建议提交**：`test(reader): cover zoom clamping and anchor math`

#### P4-3 跨章预加载单测（test）

- **状态**：todo
- **依赖**：无
- **证据**：`src/composables/useNextChapterPreload.ts` 约 93 行、0 测试；`prefetchReaderChapter` → 过滤本地页 → `prefetchCachedImages` 的链路、`warmed`/`inFlight` 去重和失败静默都只能靠观察网络与日志。
- **目标**：用 mock 的 `prefetchReaderChapter`/`prefetchCachedImages` 固定“何时预热、预热哪些 URL、失败后能否重试”。
- **用例**：
  - 不在末页、`enabled=false`、无下一章、缺 platform/id → 不请求。
  - 末页且有下一章 → 先取页源，再只预热前 N 张；N 由 `clampReaderPreloadCount` 夹到 3–10。
  - 跳过 `localPath` 页和空白 URL；下一章 0 页时图片预热收到空数组（当前实现仍会调用 `prefetchCachedImages`）。
  - 同一章、同一 N 不重复预热；并发触发只发一次；失败不写 `warmed`，后续触发可重试。
  - `scope.stop()` 后 resolve 的请求不再写状态，且不抛出。
- **不要做**：不要把预热改成阻塞翻页、写入历史或改变末页自动切章。
- **验收**：
  - [ ] `pnpm test`、`pnpm build` 通过；测试期间没有真实网络请求。
- **建议提交**：`test(reader): cover next-chapter preloading`

#### P4-5 收藏动作单测（test）

- **状态**：todo
- **依赖**：无（复用 P4-0 的 `NMessageProvider` harness）
- **证据**：`src/composables/useFavoriteAction.ts` 约 186 行、0 测试；短按 toggle、长按抑制 click、分组选择器、详情切换时的 reset 都只有人工冒烟。
- **用例**：
  - 详情从 null 变为非空才触发 `getFavorite`（watch 没有 `immediate`）；查询失败按未收藏处理且按钮可用。
  - 短按：未收藏 → `addFavorite`（默认分组），已收藏 → `removeFavorite`；成功/失败提示与本地化文案。
  - 长按：`consumeClick()` 为 true 时 `toggle` 不执行；右键/`Shift+Enter` 打开选择器；`disabled`（loading/pending/无详情）时不打开。
  - 切换作品：旧请求的结果被丢弃；`reset` 清空 entry/pending/picker。
  - `onSaved(entry)` 更新 entry 并关闭选择器。
- **验收**：
  - [ ] 不改变 `getFavorite`/`addFavorite` 的幂等语义与错误处理。
  - [ ] `pnpm test`、`pnpm build` 通过。
- **建议提交**：`test(detail): cover favorite actions`

#### P4-6 详情数据层单测（test）

- **状态**：todo
- **依赖**：无
- **证据**：`useComicDetail.ts`（约 286 行）与 `useDownloadedComicDetail.ts`（约 204 行）0 测试；在线/已下载两条链路的 state、续读回退、辅助请求失败静默都只靠 `detailState` 纯函数的 4 个用例兜底。
- **`useComicDetail` 用例**：
  - 成功 → `state.kind === "online"`；失败 → `error`；加载中 → `loading`；缺 platform/id → `invalidInput`。
  - 过期 `getComic` 响应不覆盖新作品；`getDownload`/`getHistoryEntry` 失败不破坏在线详情。
  - `downloadChapters`：manhuagui 按分组扁平化、jm 取 `extra.chapters`、nhentai 空数组。
  - 续读：历史章节存在 → 原章节/页码；jm/manhuagui 章节失效 → 首章第 1 页；nhentai 保留页码。
  - `reloadDownload(true)` 不切 `downloadLoading`，失败保留旧值。
- **`useDownloadedComicDetail` 用例**：
  - `getDownload` 返回 null → `error` 为 `notFound`，`state.kind === "error"`。
  - 只有已完成章节可读；`startChapterId` 优先 `readChapterId`，否则首个完成章节。
  - 续读章节失效 → 首个完成章节第 1 页；nhentai 无 chapterId 的历史保留页码。
  - `favoriteSnapshot` 来自下载索引；过期响应丢弃。
- **验收**：
  - [ ] 测试不联网、不挂载真实视图；`pnpm test`、`pnpm build` 通过。
- **建议提交**：`test(detail): cover online and downloaded detail data`

#### P4-7 IPC 入口与局部守卫单测（test）

- **状态**：todo
- **依赖**：无
- **证据**：`src/api/client.ts`、`src/composables/useCachedImage.ts`、`src/stores/search.ts` 均 0 测试；`client.ts` 是“唯一 IPC 入口”约定，`useCachedImage`/`search` 是记录在案的局部 `requestId` 例外。
- **用例**：
  - `invokeCommand` 默认先 `waitForEchStartup()`；`invokeLocal` 不等待；参数原样传给 `invoke`；invoke 错误不被吞掉；`waitForEch: false` 生效。
  - `useCachedImage`：有 `localPath` 时直接用 `convertFileSrc` 且不请求；已解析路径命中不请求；未命中时 loading → src；过期响应不覆盖；`fail()` 只对远程 URL `forgetCachedImage`；依赖变化 reset。
  - `search` store：空 query 不搜索；新搜索立即清空旧结果；过期响应丢弃；`changePage` 忽略越界/同页；loading 在 finally 复位。
- **验收**：
  - [ ] `pnpm test`、`pnpm build` 通过；不改变现有 `api/*.ts` 导出。
- **建议提交**：`test(web): cover ipc entry, image cache and search guards`

#### P4-8 去重与整洁（chore，低优先）

- **状态**：todo
- **依赖**：无
- **证据**：`KNOWN_PLATFORMS` 在 `favorites.rs`、`history.rs`、`downloads/mod.rs` 各一份；`useFavoriteAction.ts` 有一处两条语句挤在同一行（`});        if (latest.isCurrent(token)) {`），`useDownloadAction.ts` 有超长行；项目没有 formatter。
- **做法**：
  - Rust：新增 `src-tauri/src/platforms.rs`（或复用 `image::platform` 的公开常量），只放 `KNOWN_PLATFORMS` 与 `is_known_platform()`；三处改为引用，错误文案/错误码不变；补一个“未知平台被拒绝”的单测。不要顺手改校验顺序或 sanitize 语义。
  - 前端：只修格式（拆行），不重排逻辑。
- **不要做**：不要合并 `Intl.DateTimeFormat`（`LogsView` 用系统 locale，`NhentaiDetail` 是平台字段格式）；不要引入 Prettier/ESLint。
- **验收**：
  - [ ] `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`pnpm test`、`pnpm build` 通过。
- **建议提交**：
  1. `refactor(rust): share the known-platform list`
  2. `style(web): wrap statements in detail actions`

#### P4-9 阅读器路由编排可测化（conditional）

- **状态**：conditional（未开始；只有 P4-1~P4-7 完成后才评估）
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

### P4 完成

- [ ] P4-1~P4-3、P4-5~P4-7 全部状态为 done（P4-8 低优先、P4-9 条件项）。
- [ ] `pnpm test` 覆盖新增用例；`pnpm build`、`cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 全绿。
- [ ] 新增测试不依赖网络、真实 Tauri runtime、WebDriver 或真机。
- [ ] 测试暴露的行为缺陷有独立 `fix` 提交和回归用例。
- [ ] 没有为了可测性改公开 composable/API 签名、IPC、路由、持久化或错误码。
- [ ] 每个任务独立提交，未混入无关格式化。

### 整体完成

- [ ] P4 完成。
- [ ] P3-1 只在 wire 变化时按需推进；P3-3 有 benchmark 证据再做。
- [ ] 下一次新增列表页/平台/下载能力可以复用现有 composable、trait 和契约测试，不需要再复制三份。

---

## 8. 执行备注

> 只记录当前未完成任务的偏离与结论；完成的任务随任务卡一起删除。

- 2026-10-27 / P4 规划 / 现状复核：前端 130 个测试、Rust 169 个测试均通过；临时 spike 验证了 jsdom 下伪造 PointerEvent + stub pointer capture 可驱动手势，`NMessageProvider` 包裹可测详情动作（spike 已删除）。未改动业务代码。
