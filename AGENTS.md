# AGENTS.md

面向 AI 编码代理的仓库协作指南。修改代码前先读本文件；跨层契约变化时同步更新。实现细节以代码和测试为准，本文件只记录代理容易做错、且从代码中不易发现的部分。不要写回逐文件清单、测试输出或可以直接从代码读到的细节。

## 项目速览

- 早期 Tauri 2 漫画阅读器：Vue 3.5 + TypeScript + Vite 8，Rust 后端；桌面端与移动端（Android 已初始化，iOS 未初始化）。
- 已有：manhuagui/JM/nhentai 搜索与详情；nhentai、JM 与 manhuagui 单页阅读器；阅读历史；收藏；按作品下载与离线阅读；设置（主题、语言、阅读方向、HTTP 代理、ECH、日志）；日志页。
- 尚无：漫画库、通用章节列表、数据库、按章下载/导出。下载页不是占位页，不要按未实现处理。
- README 是上游模板，不可信。以本文件、`package.json`、`src-tauri/Cargo.toml`、`tauri.conf.json` 和代码为准。
- 重构与缺陷修复任务见 [`REFACTOR_PLAN.md`](REFACTOR_PLAN.md)；执行大规模改动前先读任务卡、验收与禁止事项。

## 环境与命令

- Node `^20.19.0 || >=22.12.0`，只用 pnpm（不要 npm/yarn）；Rust stable 1.85+（edition 2024），并安装各平台 Tauri 环境依赖（Windows 需 WebView2）。移动端另需 Android SDK/NDK/JDK。Python 只用于本地 `scripts/`，不参与构建。
- 前端命令在仓库根目录，Rust 命令在 `src-tauri/`：
  - `pnpm build`：`vue-tsc --noEmit` + Vite 构建；不编译 Rust。
  - `pnpm test`：Vitest 跑一次前端单元测试；`pnpm test:watch` 监听。测试与源码同级（`src/**/*.test.ts`），必须用 `@tauri-apps/api/mocks` 模拟 IPC。
  - `pnpm test:e2e`：构建 E2E 调试包并跑 `e2e/` 下的 WebDriver 套件；需要 `tauri-driver` 和 `e2e/.drivers/msedgedriver.exe`（版本匹配 WebView2，已 gitignore）。只预置并清理 `%APPDATA%\com.meik.comics-reader.e2e`，不碰真实数据；无 CI。
  - `pnpm tauri dev`：完整应用；验证 IPC、图片、路由、主题等必须用它。
  - `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`：Rust 门禁。
  - Windows 下若 `PATH` 里的 Android JDK `bin` 排在 `System32` 前，测试二进制会加载其中的 `api-ms-win-crt-*` 桩 DLL 并以 `STATUS_ENTRYPOINT_NOT_FOUND`（0xc0000139）退出；先从 `PATH` 移除该目录再跑。Rust 侧只用单元测试，不要引入 `tauri/test` mock runtime：它会把 wry/comctl32 链进测试二进制，需要额外 manifest。
  - `pnpm tauri <args>`：透传 CLI；`pnpm tauri build` 的 `bundle.active` 为 false，不生成安装包。
- `pnpm dev` 和 `pnpm preview` 只有前端，没有 Tauri API。
- 端口固定 1420（`strictPort`）；`vite.config.ts` 与 `tauri.conf.json` 的 `build.devUrl` 必须一致。
- 预计 Vite 主 bundle >500kB 警告；除非任务就是优化体积，否则不处理。

## 代码地图

- `src/main.ts`：启动顺序；挂载前恢复主题、语言、日志、代理、ECH。
- `src/App.vue`：Naive UI Provider、消息 Provider、顶部安全区、RouterView；没有 NDialogProvider，删除确认用 `NPopconfirm`/`NModal`。
- `src/views/`：页面（含收藏、下载列表）；`src/components/`：搜索、结果卡片、收藏分组选择器、详情、阅读器。
- `src/stores/`：`settings`（唯一持久化）、`search`（内存态）、`runtime`（ECH 启动状态）。
- `src/composables/`：主题、语言、错误文案、图片缓存、缩放、长按手势、阅读历史写入、分页资源、请求竞态守卫。
- `src/api/`：`client.ts` 是唯一 IPC 入口（`invokeCommand`/`invokeLocal`），其余为各领域封装；`src/i18n/`：类型化语言包；`src/router/`：hash 路由、守卫、窗口标题。
- `src/test/setup.ts`：Vitest 全局 setup（WebCrypto polyfill、每例 `clearMocks()`）；测试配置在 `vitest.config.ts`。
- `e2e/`：WebDriver 配置、离线种子数据与 specs；`run.mjs` 启动 tauri-driver 并清理测试数据。`src-tauri/tauri.e2e.conf.json` 覆盖 identifier。
- `src-tauri/src/`：
  - `lib.rs`：Builder、插件、命令注册；`error.rs`：AppError/ErrorCode。
  - `image/`：HTTP、缓存、平台搜索/详情/阅读页源。
  - `ech/`：ECH bootstrap、DoH、TLS；`history.rs`（JSONL 历史）、`favorites.rs`（v2 JSON 收藏与分组）、`downloads/`（索引状态机、worker、命令、model/index/local）、`logging.rs`（JSONL 日志）、`storage.rs`（收藏/下载索引的原子写、备份、分页、时间戳）。
- `capabilities/default.json`：窗口权限；`tauri.conf.json`：窗口、构建、资产协议、打包。
- 忽略：`node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/**/build/`、`scripts/.venv/`。

## 核心契约

### 分层与数据归属

- UI 只通过 `invoke`、`convertFileSrc` 和窗口 API 与 Rust 交互。
- Rust 命令返回 `Result<T, AppError>`；用户文案只在前端语言包，Rust 不返回展示字符串。
- 设置保存在 localStorage 的 `comics-reader:settings`（插件前缀 `comics-reader:`）；`settings` 是唯一持久化 store。Rust 不持久化设置，代理/ECH/日志开关是运行时状态，启动和设置变更时用 `set_http_proxy`、`set_ech_enabled`、`set_logging_enabled` 同步。
- 应用数据：历史 `app_data_dir()/history.jsonl`，收藏 `app_data_dir()/favorites.json`（version 2，含分组），下载 `app_data_dir()/downloads.json` + `app_data_dir()/downloads/`，日志 `app_log_dir()/app.log`，图片缓存 `app_cache_dir()/images/`。历史、收藏、下载与日志内容不能放 localStorage（日志开关偏好仍属于 `settings`）；图片缓存是可丢弃数据。
- 主题写入 `document.documentElement.dataset.theme`（`light`/`dark`），`App.vue` 同时传 Naive UI 的 `darkTheme`。
- Tauri 不会自动同步 `document.title`；`router/index.ts` 显式 `setTitle`，切换语言后 `useLocale` 再次调用。
- 路由用 `createWebHashHistory()`；不要改 web history。
- 已持久化字段不要直接重命名/删除；没有迁移层，旧值会静默丢失。

### 错误契约

- 序列化形状：`{ code, message, details }`。`code` 稳定、机器可读；`message` 是技术英文；`details` 只放扁平 string/number，供 i18n 参数使用。
- 新增或修改错误码必须同时改五处：`error.rs`（变体/`ErrorCode`/`code`/`details`）、`src/errors.ts`、`useErrorMessage.ts`、`en.json`、`zh-CN.json`。只改 Rust 会在 UI 退化为 `unknown`。
- 序列化结构变化要同步 `error.rs` 的单元测试。
- 图片错误码：`httpStatus` / `emptyResponse` / `notImage` / `tooLarge`；`details` 分别带 `url`/`status`/`contentType`/`limit`。
- 检索/详情：未知平台、空 query、page<1、非法 id 用 `invalidInput`；上游 HTTP/JSON/HTML 变化复用 `httpStatus`/`network`/`json`/`unexpected`，不新增错误码。

### i18n

- `en.json` 是主 schema，`MessageKey` 由它派生；`zh-CN` 缺 key 会编译失败。
- 用 `translate()` 或 `useAppI18n()`；不要用 `useI18n().t()` 传裸字符串。
- 新 key 同时写 `en.json` 与 `zh-CN.json`，占位符一致；新增语言还要改 `SUPPORTED_LOCALES`、`messages`、`NAIVE_LOCALES`、`detectSystemLocale()`。
- 错误以 code/details 保存，渲染时翻译；参考 `search` store 的 `error` ref。

### 设置与运行时

- `useTheme()` 和 `useLocale()` 只在 shell 调用一次；其他组件直接读 `settings` store。
- 阅读方向 `readerDirection`：`ltr`/`rtl`/`ttb`，默认 `ltr`，只在前端。
- 阅读器预加载 `readerPreloadCount`：3–10，默认 3，控制当前章节向后预热的页数，也用作末页预加载下一章时预热的图片数；不是磁盘缓存淘汰上限，不同步 Rust。
- 末页预加载下一章 `readerPreloadNextChapter`：默认开；到达当前章末页且有下一章时，先取下一章页源，再预热前 `readerPreloadCount` 张图片。只在前端，失败静默，不改变翻页/跳章。
- 代理：无认证 `http://host:port`；启动和保存时 `set_http_proxy` 重建共享 client。关闭开关立即 `set_http_proxy(null)`，无需保存；开启或改地址要保存成功后才写 store。
- ECH：`set_ech_enabled` 同步；开启时自动关代理；设置页打开代理前先关闭 ECH；ECH 开启时 `set_http_proxy(Some)` 返回 `invalidInput`。启动时异步初始化，失败回滚 `settings.ech`；`runtime.echStarting` 为 true 时首页显示等待，搜索/详情/缩略图请求都等待初始化。
- 日志：`set_logging_enabled` 同步，默认关；开关失败回滚并提示。日志页 `/settings/logs` 只读最近 500 条，按时间倒序；导出用原生保存对话框；`app.log` 超 1MiB 轮转为 `app.log.1`，只留一份；清空同时删除两个文件，不弹确认。
- 设置页“网络设置”把 HTTP 代理与 ECH 放在一起；代理开关文案“HTTP 代理”；不要恢复旧分组/旧文案。

### 网络、图片缓存、平台

- 图片全部由 Rust 处理：`ComicThumb` → `fetch_image` → `convertFileSrc` 资产协议。不要直接用远程 URL、blob/object URL 或浏览器 `fetch`。
- 资产协议三处同时成立：`tauri` 的 `protocol-asset` feature、`tauri.conf.json` 的 `app.security.assetProtocol.enable`、覆盖 `$APPCACHE/images/**` 与 `$APPDATA/downloads/**` 的 scope；缺一为 403/空白。
- 缓存键 = 平台 + 平台 `cache_version` + URL 的 SHA-256；manhuagui/JM 忽略 query。写入 `app_cache_dir()/images/<platform>/<hash>.<ext>`；格式按魔数，扩展名决定 MIME。修改平台请求头/缓存语义要提升该平台 `cache_version`；改 URL 归一化或目录提升 `CACHE_VERSION`。启动后后台清扫：超过 30 天或总量超过 512 MiB 的图片会被删除，最近 1 小时文件受保护；下载目录不参与清扫。
- 所有远程请求走 `HttpClient::send`（代理/ECH）：ECH 关闭用普通 client（可走代理），开启用 `no_proxy` 的 ECH/plain client。不要另建 client。
- ECH 内部细节（`cloudflare-ech.com` 配置、Cloudflare DoH、TTL、fallback 缓存）以 `image/http/`、`ech/` 和测试为准。必须保留的约束：代理与 ECH 互斥；启动初始化要等待；非 Cloudflare 站点 plain TLS 回退是预期。
- 平台集中在 `image/platform/`：`ImageProfile` 负责图片抓取（`id`/`matches`/`configure`/`cache`/fallback/decode），`Catalog: ImageProfile` 负责搜索/详情/阅读（`prepare`/`note_request_outcome`/request+parse）。当前 `generic` 只有 profile，`manhuagui`/`jm`/`nhentai` 是 catalog。`platform` 参数可省略时按 host 识别；已知平台显式传，JM 尤其。JM 在首个搜索/详情/阅读命令前通过 `prepare` 请求 `/setting` 同步 `jm3_version`（进程一次，失败下次重试），`tokenparam` 使用运行时版本。镜像健康只在平台解析成功后记为成功；HTTP 2xx 的挑战页不算。
- 搜索会话由 `search` store 管理；`/search` 不携带 query，刷新/深链会被守卫送回首页；首页图标固定回首页。
- 详情 `/comic/:platform/:id` 不依赖搜索会话，直接 `get_comic`，刷新/深链可用；在线详情失败、或等待约 1.5s 仍未返回而本地有已完成章节的下载时，先用 `get_download`（返回 `DownloadDetail`：列表摘要 + `chapters`）渲染离线详情（在线详情随后返回则替换为完整详情），封面用 `coverPath`，开始阅读取首个已下载章节，继续阅读优先历史章节/页码，章节不可用时回退首个已下载章节第 1 页。返回目标优先读导航 `state.from`（收藏/历史/下载/搜索/首页），缺失时才回退上一页判断，最后看搜索会话；但必须走 `useBackNavigation`：上一页就是目标时 `router.back()`，否则 `router.replace`。
- 搜索返回 `{ total, page_size, result: [{ id, title, thumb, platform, extra }] }`，详情 `{ platform, title, thumb, extra }`；`platform` 判别，`extra` 强类型（Rust `SearchExtra`/`DetailExtra`，前端判别联合）。不要退回 `Record<string, unknown>` 或 JSON 兜底。
- 阅读页源 `get_reader_chapter(platform, id, chapterId?)`；nhentai、JM、manhuagui 均已实现。页图片仍逐页 `fetch_image`，不新增缓存目录。JM 页图在缓存前由 Rust 按 `scramble_id` 还原并重编码为 JPEG（缓存键带 `?decoded=v1`，算法变更必须提升 `IMAGE_DECODE_VERSION`；`scramble_id` 每章节请求一次并缓存）。空页统一返回 `notFound`。
- manhuagui 详情优先解 `#__VIEWSTATE`（R18/隐藏列表，LZString），失败再读 `.chapter-list`；`h4` 与 `.chapter-list` 配对成 `ManhuaguiExtra.chapterGroups`，每个 `ul` 内的 `li` 逆序得到阅读顺序；无标题分组传空标题由前端本地化。搜索结果 `chapterGroups` 为空。阅读页地址 `/comic/{id}/{chapterId}.html`，`chapterId` 必需；详情“开始阅读”取分组扁平化后的首章，`ComicReaderView` 对无章节深链先取详情首章并 `router.replace`。
- 详情章节用 `NCollapse` 按分组展示，默认展开继续阅读所在组、否则首组，并提供展开全部/收起全部；不要恢复单块 `max-height` 内滚动。
- manhuagui 页源按 `lanyeeee/manhuagui-downloader` 的 `decrypt.rs` 算法：正则提取 packed payload、`a/c` 与压缩字典，`lz_str` 解压字典后按 token 替换，解析 `files/path/bname/cname/bpic/nextId/prevId`。图片为 `https://i.hamreus.com{path}{file}`，先去掉文件名的 `.webp` 后缀且不带 query（`cf.hamreus.com` 备用）；`cache_version` 因 URL 归一化改为 3。上一/下一章优先用 `nextId/prevId`，缺失才解析 HTML 链接；不要在前端拼 manhuagui 图片地址。
- `nhentai.net` 在 AliDNS/360 的地址可能被污染；ECH 探针已通过 Cloudflare DoH 解析目标，可直连。JM 走移动端 APP API（`image/platform/jm_api.rs`）：HTML 站点在部分网络对搜索和专辑都返回 403，APP 域名可用；搜索/详情需要 token 签名与 AES 解密，封面由 album id 推导，不需要 Cookie 或 TLS 指纹。请求仍走 `HttpClient`。
- Nhentai 详情封面用 `t.nhentai.net` 缩略图：`i.nhentai.net` 在部分网络不可达，240px 封面足够。

### 阅读器

- 路由 `/comic/:platform/:id/read/:chapterId?`，适用 nhentai、JM 与 manhuagui；`ComicReaderView` 负责加载页源、写 `?page=`、返回详情和错误态。JM 详情页在多章时列出章节并传入所选 id，单章本子直接进第一章；省略 `chapterId` 时后端退化为用 album id 作为章节。manhuagui 详情按分组折叠列出章节并把扁平化首章传给“开始阅读”，阅读器对缺失章节的深链会先解析首章。
- 单页视口：上/中/下三页槽位组成 3 倍轨道，当前页居中；翻页整体滑动，无背景缝。
- `readerDirection` 决定轨道轴和手势/方向键：`ltr` 横向、`rtl` 横向反向（下一页在左）、`ttb` 纵向；缩放后拖拽平移，滚轮翻页。
- 上下栏是绝对定位覆盖层，默认隐藏，不占 stage 布局；点击中间 1/3 切换显隐，左右 1/3 按方向翻页；不要在滑动、滚轮或翻页时自动唤出。
- 页码用 `router.replace` 写 `?page=`，不新增历史；越界夹到 `1..pages.length`；空页显示空态。
- 首/尾页继续翻页自动切章：第一页上一页进入上一章最后一页（内部 `?page=last`，加载后替换为真实页码），最后一页下一页进入下一章第一页；没有上一/下一章时用 `useBackNavigation` 回详情。所有入口（按钮、点击区、滑动释放、滚轮、方向键）共用这条边界逻辑；底部“上一章/下一章”按钮仍从章节第一页打开。
- 右侧 HUD 只在菜单隐藏时显示页码，顶部不占 stage 布局；使用 `--app-safe-area-right`，点击穿透。移动端电量用 `navigator.getBattery()`，菜单展开时放顶栏、隐藏时放 HUD；桌面或不支持时隐藏。
- 移动端双指缩放只处理两个触摸指针，1x–4x；缩放中围绕两指中点平移，松开一指后转为单指平移，且不得触发 tap 翻页。桌面鼠标/触控板行为不变。
- 章内预加载走 `fetchCachedImage`（同一 `platform + url` 合并请求），由 `useReaderPreload` 以 200ms 防抖、2 并发预热后续页；失败静默，切页/切章停止旧队列。末页的跨章预热由 `useNextChapterPreload` 调 `prefetchReaderChapter` 取下一章页源，再用 `prefetchCachedImages` 预热前 N 张；不把章节数据展示或写入历史。
- 为避免末页切章时“重新加载”：`src/api/reader.ts` 缓存最近 4 个 `ReaderChapter` 页源并共享 in-flight `get_reader_chapter`，`ComicReaderView` 命中时直接 apply；`src/api/images.ts` 缓存最近 64 个已解析图片路径，`useCachedImage` 命中时直接 `convertFileSrc`，不再进入 loading。页面列表/图片路径缓存都只在内存中有界保留。
- 页图片仍走 `fetch_image`，不要直接用 CDN URL。缩放、全屏、`Home`/`End` 等现有行为不要破坏。
- JM 章内用页源返回的 `previous`/`next` 导航，不要在前端自行拼接章节；manhuagui 也从页源解析上一/下一章，缺失时由详情页章节列表兜底。

### 阅读历史

- 按作品 `platform + id` 记录，只保留最后章节/页码；进入阅读器成功加载页源时写入，翻页防抖，离开/切章 flush；只浏览详情不产生记录。
- Rust 拥有：`app_data_dir()/history.jsonl`，每行 `record`（旧版 `remove` 仍重放）；首次命令加载时重放为内存索引；上限 10,000 条，超出丢最旧；事件数超过 2 倍上限或加载时裁剪过会压缩；读取过滤未知平台/空字段/非法页码，按 `updatedAt` 倒序。
- 命令：`list_history(offset?, limit?)`、`get_history_entry(platform, id)`、`record_history(entry)`；每页默认 50、最大 200；只取当前页，不把全部条目传前端。
- 历史行点击进入漫画详情，不再直接进阅读器；详情页用 `get_history_entry` 展示“继续阅读”，恢复时带历史的 `chapterId`/`page` 进 `comic-reader`，JM 章节失效时回退第一章。
- 历史不可删除/清空：没有 `remove_history`/`clear_history` 命令和前端入口；不要以“管理历史”为由恢复。作品只在超过上限时被挤出。
- 历史文件是应用数据，不是设置：不要放 localStorage，不要每次翻页重写整个 JSON，不要全量传前端。记录失败静默（Rust 记日志），不能影响阅读。
- `ReaderChapter.thumb` 仅供历史封面，仍走 `ComicThumb` 缓存。当前无单实例插件，多实例并发写不保证一致。

### 收藏

- 按作品 `platform + id` 收藏，键与历史相同；只存快照 `title`、`thumb`、`addedAt`、`groupId`，不做远程元数据刷新；上游作品被删除后收藏仍保留，用户可手动取消。
- Rust 拥有：`app_data_dir()/favorites.json`，`version: 2`，包含 `groups` 与 `entries`；首次命令加载为内存索引，整表原子重写（临时文件 + `sync_all` + rename），不要改成每次设置读取或 localStorage。v1 无分组数据在加载时迁移到默认分组并写回 v2，不能丢旧收藏。
- 一个作品只属于一个分组；移动分组就是更新 `groupId`。默认分组 id 固定为 `default`，名称存空字符串并由前端按语言显示；默认分组不可改名/删除。
- 命令：`list_favorites(offset?, limit?, groupId?)`、`get_favorite(platform, id)`、`add_favorite(entry) -> FavoriteEntry`、`remove_favorite`、`list_favorite_groups`、`create_favorite_group(name)`、`rename_favorite_group(groupId, name)`、`delete_favorite_group(groupId)`；每页默认 50、最大 200。
- `add` 幂等：新条目无 `groupId` 进默认分组；已有条目无 `groupId` 保留原分组；显式传 `groupId` 则移动；始终保留 `addedAt` 并刷新标题/封面，返回保存后的 entry。删除分组把其中收藏移到默认分组。
- 收藏页支持“全部 / 默认分组 / 自定义分组”筛选、计数、创建/重命名/删除分组；列表按 `addedAt` 倒序，点击条目进详情，取消收藏不弹确认；缩略图仍走 `ComicThumb`，不要为每条收藏请求远程详情。
- 详情页收藏按钮：短按 toggle（未收藏进默认分组，已收藏取消收藏）；长按（或右键 / `Shift+Enter`）打开分组选择器，未收藏时选择后加入，已收藏时移动。长按 500ms，移动/取消/离开要取消，长按后的 click 必须抑制，否则会误取消收藏；选择器含新建分组并提示长按用法。
- 详情页先 `get_favorite` 取当前分组；查询失败按未收藏处理，`add` 的幂等语义保证不会误移分组。请求带 token，切换作品丢弃旧结果。
- 从收藏页、历史页、下载页或搜索结果进详情时写导航 `state.from`；详情优先用 `state.from`，`previousPath()` 只兜底旧入口，最后才看 `search.active`。收藏页自身返回首页。
- 文件损坏按空列表并备份 `.bak`；更高 `version` 返回 `unexpected` 并拒绝写入。收藏不用新增错误码，复用 `invalidInput`/`io`/`json`/`tauri`/`unexpected`。

### 下载

- v1 只按作品下载整部章节，不支持按章勾选；详情页提交当前详情里的完整章节列表，nhentai 传空列表由 Rust 合成单章。下载页不是缓存页，列表项只进漫画详情，不直接打开阅读器。
- Rust 拥有 `app_data_dir()/downloads.json`（version 1）和 `app_data_dir()/downloads/<platform>/<work-id>/`；每章完成写 `manifest.json`，页面与索引原子写入。不要把下载放 `app_cache_dir()`，也不要依赖图片缓存清理。
- 下载复用 `image::fetch_image_inner` / `get_reader_chapter_inner`，页面从缓存复制到下载目录；重复磁盘占用是接受的取舍，不要另写平台请求逻辑。
- 同一时间只跑一个作品；章节按详情顺序，页页串行。索引只在开始、章节边界、暂停和结束时写，不要每页重写整个索引。
- 状态为 `queued/downloading/paused/completed/partial/failed`；启动/首次加载把遗留 active 状态改为 `paused`，不自动联网续传。暂停/续传用 run id 让旧 worker 失效；run id 进程内单调分配、删除后也不复用；删除只允许非 active 状态。
- `get_downloaded_chapter` 返回带 `localPath` 的 `ReaderChapter`；`get_reader_chapter` 的前端封装先查本地再等 ECH。本地章节的 `previous`/`next` 只包含已下载章节，离线翻章不会跳到需要联网的章节。详情页离线回退的“开始阅读/继续阅读”也只用已完成下载章节，不被在线详情阻塞。
- 资产协议必须同时放行 `$APPCACHE/images/**` 与 `$APPDATA/downloads/**`；`ReaderPageImage`/`useCachedImage`/`ComicThumb` 优先使用 `localPath`，阅读器预加载必须跳过本地页。
- 下载进度用活动期间轮询（下载页 1s、详情页 1.5s），不要为此增加事件权限；页面卸载必须清定时器。
- 下载错误复用现有错误码；索引/manifest 损坏按空/`None` 处理并记日志，更高版本返回 `unexpected` 并拒绝写入。

### 安全与生成文件

- 命令必须注册进 `lib.rs` 的 `generate_handler!`；插件权限必须进 `capabilities/default.json`，否则运行时 “not allowed”。
- CSP 为 `null`；不要引入远程 HTML 或 `v-html`。
- 不要手改生成文件：`src/components.d.ts`、`src-tauri/gen/schemas/`、`src-tauri/gen/android/**/build/`、`src-tauri/target/`、`dist/`、`src-tauri/icons/` 下的 PNG。用工具重新生成。
- `bundle.active` 为 `false`；需要安装包时显式开启。

## 编码约定

### TypeScript / Vue

- `<script setup lang="ts">` + Composition API；strict、`noUnusedLocals`、`noUnusedParameters` 已开启，不留未使用 import/变量。
- 避免 `any`；未知输入用 `unknown` + 类型守卫。仅类型导入用 `import type`；解构 store 用 `storeToRefs()`。
- 依赖设置/语言的值用 `computed`；下拉选项和消息在 `computed` 内构造。
- 异步用 `try/catch/finally` 并复位 loading；错误用 `useErrorMessage()` 呈现。
- 监听器/定时器在 `onBeforeUnmount`/`onScopeDispose` 清理；只有确实忽略 Promise 才用 `void promise.catch(...)`。

### Rust

- 命令返回 `error.rs` 的 `Result<T>`；失败用 `AppError`，优先 `From` + `?`，不要手写映射。
- 命令参数简单可序列化；返回数据，不返回展示文案。
- 日志用 `logging::info`/`warn`/`error`；开关关闭时它们直接返回，不要在调用点手写文件；`target` 保持稳定，不记录认证信息或秘密。
- 新命令必须注册；错误序列化和新 wire 结构补测试（`#[cfg(test)] mod tests`）。
- 运行 `cargo fmt`；Clippy 以 `-D warnings` 通过；使用默认 rustfmt/clippy 配置。

### UI / 样式 / 安全区

- 全局 reset/基础样式在 `src/styles/base.scss`；组件样式放 `<style scoped>`。
- 优先 Naive UI 组件；自动导入已配置，不要手改 `components.d.ts`；`useMessage()` 必须在 `NMessageProvider` 下，`App.vue` 已提供。详情加载态用 `ComicDetailSkeleton`（`NSkeleton`）并保持 hero/body 布局，不要退回通用 `NSpin`。
- 用现有主题机制（`NConfigProvider`、`darkTheme`），不要散落 CSS 变量/内联颜色。
- 顶部安全区由 `App.vue` 的 `.safe-area-top` 统一负责：sticky、使用 `--app-safe-area-top` 和 `--app-background`。页面不要用 `env(safe-area-inset-top)` 的 margin/padding，也不要用 `max-width`/`pointer: coarse` 模拟。`index.html` 的 viewport 必须带 `viewport-fit=cover`，否则 `safe-area-inset-*` 全为 0。
- 四边安全区统一暴露为 `--app-safe-area-top/right/bottom/left`。`NMessageProvider` 的 teleport container 用 `container-style` 避开顶部/左右；Modal 的 `.n-modal-body-wrapper` 用它约束四边。新增 toast、Modal、Popover 等浮层必须接入安全区，不能只给页面内容加 padding。
- 阅读器底部覆盖栏用 `padding-bottom: env(safe-area-inset-bottom)`；不要再加独立占位条。
- 设置项只保留简洁标签；设置导航项整行可点击；日志页返回按钮用无文字 chevron 图标，工具栏只用图标按钮（带本地化 `aria-label`/`title`），清空不弹确认，不显示日志路径。
- 注释解释“为什么”，不复述代码；只写不易理解的决策。

## 常见改动入口

| 改动 | 关注点 |
| --- | --- |
| 新 Tauri 命令 | `src-tauri/src/lib.rs` 定义并注册；前端 `src/api/*.ts` 封装；失败返回 `AppError`。 |
| 新 Tauri 插件 | `src-tauri/Cargo.toml`、`lib.rs`、`package.json`（如需 JS API）、`capabilities/default.json`。 |
| 新错误码 | `error.rs`、`src/errors.ts`、`useErrorMessage.ts`、两个语言包，五处一次改完。 |
| 新界面文案 | `en.json` + `zh-CN.json`；通过 `useAppI18n()` 使用。 |
| 新路由 | `src/views/` + `router/index.ts`（`RouteTitleKey`/meta）+ 两个语言包；路由放在现有 RouterView 内继承安全区。 |
| 新平台 | `image/platform/` 实现 trait 并注册；`model.rs` + `src/api/comics.ts` 判别联合；`PlatformPicker`、结果卡片、详情组件、图标、fixture、`cache_version`。 |
| 改设置 | `stores/settings.ts` + `SettingsView.vue`；若同步 Rust，加对应 `src/api/*.ts` 与命令。 |
| 改阅读器/页源 | `components/reader/`、`views/ComicReaderView.vue`、`router/index.ts`、`api/reader.ts`、`image/model.rs`、`image/platform/`、`image/mod.rs`。 |
| 改收藏 | `src-tauri/src/favorites.rs`、`src/api/favorites.ts`、`FavoritesView.vue`、`ComicDetailView.vue`、`FavoriteGroupPicker.vue`、`useLongPress.ts`、两个语言包。 |
| 改下载 | `src-tauri/src/downloads/`、`src/api/downloads.ts`、`DownloadsView.vue`、`ComicDetailView.vue`、`OfflineDetail.vue`、`api/reader.ts`、`ReaderPageImage.vue`、`useCachedImage.ts`、`tauri.conf.json` 资产 scope。 |
| 改图片/网络 | `src-tauri/src/image/`、`src-tauri/tauri.conf.json`（资产 scope）；请求必须走 `HttpClient`。 |

## 易错点

- `pnpm dev` 没有 Tauri API；运行时验证必须 `pnpm tauri dev`。
- `/search` 无 query、刷新回首页是刻意取舍；不要改成持久化搜索。
- 详情页与搜索会话解耦；不要强行复用 `search` store。
- 缩略图必须走 `ComicThumb` → `fetch_image` → 资产协议；否则防盗链站点 403。
- ECH 与代理互斥；ECH 启动异步门禁；非 Cloudflare plain fallback 正常；`www.manhuagui.com` 的 `notOffered`/TLS1.2 不是 bug。
- 顶部安全区不能用页面 padding；阅读器底部安全区由覆盖栏自己处理。
- 设置持久化没有迁移层；重命名字段必须迁移。
- 历史不可删除/清空；不要加回 remove/clear。
- 收藏可取消；不要照搬历史的不可删除约束。收藏列表项进详情，不直接进阅读器；不要为每条收藏请求远程详情。
- 一个作品只属于一个分组；默认分组 id 固定 `default`，不可删除/改名。收藏 v1 文件必须迁移到 v2，不要只改 version。
- 详情页收藏按钮是短按 toggle + 长按分组；长按必须抑制随后的 click，否则会误取消。右键 / `Shift+Enter` 也要能打开选择器。
- 从收藏/历史/下载/搜索结果进详情时写 `state.from`；详情优先读它，`previousPath()` 只兜底旧入口，最后才看 `search.active`。
- 历史行、收藏行、下载行都进详情，不直接进阅读器；详情“继续阅读”用 `get_history_entry` 的 `chapterId`/`page`，JM 章节失效时回退第一章。断网时详情用下载索引回退，开始/继续阅读只走本地章节。
- 返回按钮不要用 `router.push`：用 `useBackNavigation` 在上一页等于目标时 pop，否则 replace；push 会把当前页留在历史栈，系统返回会重新进入刚离开的页面（搜索 → 漫画 → 返回 → 漫画）。阅读器切章同样用 `router.replace`。
- 首页/结果页用 View Transition；不要用延迟 `router.push` 等动画；不支持时直接导航。
- 生成文件不要手改；`README.md` 只是上游模板。
- 缓存启动后会按年龄/容量清扫（30 天或 512 MiB，最近 1 小时文件受保护），不要假设缓存永久保留；图片格式和缓存键变更仍要提升版本。
- 阅读器边界切换必须走 `page-boundary` + `router.replace`；上一章末页用内部 `?page=last`，加载后必须换成真实页码，不能留在 URL 里。
- `readerPreloadNextChapter` 只预热下一章图片，不改变末页自动跳章；跨章预热必须用 `get_reader_chapter` + `fetch_image`，失败静默且不能阻塞阅读。
- 只预热磁盘而不复用 `ReaderChapter`/已解析路径，用户仍会看到章节 loading 与图片 spinner；跨章预加载必须同时走页源缓存和图片路径缓存。
- 预加载也只能走 `fetch_image`，不要直接请求 CDN；同 URL 请求由 `src/api/images.ts` 合并，避免和可见槽位重复下载。
- 双指手势结束后必须抑制随后的 click/tap，否则会误触左右翻页；电量 API 不可用时隐藏，不要显示假数据。
- JM 域名常轮换；调用显式传 `platform: "jm"`；遇到 403 先确认走的是 APP API 而不是 HTML。
- 平台解析依赖上游结构；改 selector 必须补 fixture，并保持 `extra` 字段名稳定。
- manhuagui 阅读的 `chapterId` 是必需参数；详情页传首章，无章节深链由 `ComicReaderView` 先解析首章。页图不带签名 query，必须去掉 `.webp` 后缀；`cache_version` 已升到 3。R18 `__VIEWSTATE` 优先解压，失败才回退静态列表。
- 下载不是按章选择；详情页提交完整章节列表，nhentai 传空列表。不要把下载文件写进图片缓存目录，也不要用缓存淘汰逻辑管理下载。
- `get_reader_chapter` 的本地查询必须先于 `waitForEchStartup`，否则离线阅读会被网络初始化卡住；本地页的 `localPath` 必须让 `useReaderPreload` 跳过。
- 重启后遗留的 `queued/downloading` 必须转 `paused`，不能自动联网续传；暂停/续传必须用 run id 防止旧 worker 覆盖新状态。
- 无 CI；`pnpm test`、`cargo test`、`pnpm build` 等本地检查是唯一门禁。

## 验证

- 前端改动：`pnpm test` + `pnpm build`。
- Rust 改动：在 `src-tauri/` 运行 `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。
- E2E（本地 Windows）：`pnpm test:e2e`；只使用 `e2e/` 预置的离线下载/收藏，不访问上游。
- IPC、图片缓存、搜索、主题、i18n、路由、阅读器、历史、收藏分组、下载：`pnpm tauri dev` 真实窗口冒烟；`pnpm dev` 不够。
- Android：模拟器/真机 `pnpm tauri android dev`。
- 手动冒烟（按改动选做）：
  - 搜索 → 结果 → 详情加载骨架 → 详情 → 返回；缩略图/分页/空态/错误。
  - 设置：主题/语言立即生效并重启恢复；预加载页数 3/10 持久化；末页预加载下一章开关；代理保存/关闭；ECH 切换与启动恢复。
  - 日志：开关、日志页图标、导出、清空、重启恢复。
  - 阅读器：nhentai 页图片、方向、滑动/滚轮/键盘、缩放、全屏、点击中间显隐/左右翻页、`?page=`、返回详情；JM 章节入口、前后章、页图还原无错位；manhuagui 分组折叠/展开全部/继续阅读所在组自动展开/开始阅读/前后章/页图去 `.webp`/无章节深链首章解析；首/尾页自动切章及首章/末章回详情。
  - 移动端阅读器：菜单隐藏时右上角页码 HUD、电量显示与插拔更新、双指缩放/缩放后单指平移、不误翻页；HUD 避开右侧安全区。
  - 历史：行进详情、详情返回历史、“继续阅读”恢复章节/页码、“开始阅读”从头、更新后返回详情。
  - 收藏分组：v1→v2 迁移、创建/重命名/删除、计数/筛选、短按默认、长按选择、右键 / `Shift+Enter`、从收藏进详情返回收藏。
  - 下载：三平台各下载一本、封面/页图、详情按钮状态、暂停/继续/删除、重启后恢复为暂停；下载页点条目进详情，断网后详情回退显示已下载章节，开始阅读/继续阅读（含历史页码）可用；删除后阅读器不再命中旧本地页。
  - 移动端安全区：所有页面从安全区下方开始，滚动不盖状态栏；toast 与 Modal 也不越过顶部/左右/底部安全区。
- Rust ↔ TS wire 契约：`contracts/reader-chapter.json` 与 `contracts/download-detail.json` 同时被 Rust 序列化测试和 `src/api/contract.test.ts` 的类型/字段断言引用；改对应 Rust 结构或 TS 类型时必须同步 fixture。
- 前端已有 Vitest 单元测试（`src/**/*.test.ts`），覆盖错误契约、图片/章节缓存时序、预加载、长按、返回栈、设置持久化，并有少量组件测试（电量、详情骨架、阅读页图片）；`e2e/` 另有本地 WebDriver 套件，仍无 CI，不要声称存在。新增测试体系要加 `package.json` 脚本并同步本文件。
- 前端测试跑在 jsdom、不启动 Tauri；Pinia 4 把插件排队到 `app.use(pinia)` 之后，测试里创建 store 前要先 `createApp(...).use(pinia)`，否则持久化插件不生效。
- Rust 测试数量和联网测试的实测结果会变化；以 `cargo test` 输出为准，不要把具体数字当契约。

## 尚未实现

- **JM 域名自动更新**：域名列表内置，失败会换域名但不会拉取 JM 发布的最新域名；如需更新，从发布页获取，不要搬 Python 登录/收藏/下载框架。
- **按章下载/导出**：v1 只支持按作品下载整部章节；按章勾选、CBZ/文件夹导出未实现。存储模型已按章组织，扩展时不需要改索引版本。
- **更多平台**：按平台改动入口实现；先确认接口、登录/付费/地区限制与抓取许可；至少补搜索/详情 fixture；保持判别联合的编译期安全。计划中的平台见 `ROADMAP.md`。

## 提交

- Conventional Commits，祈使句、小写 subject，一次一个逻辑变更。
- 依赖变化同时提交 `pnpm-lock.yaml` 与 `src-tauri/Cargo.lock`。
- 不要提交 `node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/schemas/`、`scripts/` 或未跟踪的编辑器文件（已提交的 `extensions.json` 除外）。
- 默认分支 `master`；无 CI，本地检查是唯一门禁。
- 没有 ESLint/Prettier 配置；不要格式化无关文件，保持 diff 聚焦。
