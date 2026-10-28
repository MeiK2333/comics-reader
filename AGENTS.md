# AGENTS.md

面向 AI 编码代理的仓库协作指南。代码与测试是最终事实；本文件只记录跨层契约、非显然的取舍和代理容易做错的约束，不维护逐文件清单。`README.md` 是上游模板，不可信。跨层契约变化时同步本文件与对应 fixture。

## 项目状态

- Tauri 2 漫画阅读器：Vue 3.5 + TypeScript + Vite 8，Rust edition 2024；桌面端与 Android 已初始化，iOS 未初始化。
- 已实现：五平台搜索/详情/阅读、历史、收藏分组、按作品下载与离线阅读、设置与日志。
- 未实现：漫画库、独立章节列表页、数据库、按章下载/导出。下载页不是占位页。
- 尚未正式发布：没有迁移层；持久化字段和文件格式可以直接改，也可以丢弃旧格式；正式发布后必须迁移，不能静默丢数据。

## 环境与命令

- Node `^22.12.0 || ^24.0.0 || >=26.0.0`（Vitest 5），只用 pnpm；Rust stable 1.85+；Windows 需 WebView2，Android 需 SDK/NDK/JDK。
- 前端命令在仓库根目录，Rust 命令在 `src-tauri/`：
  - `pnpm build`：`vue-tsc --noEmit` + Vite 构建，不编译 Rust。
  - `pnpm test`：Vitest 跑一次（`src/**/*.test.ts`，jsdom）。涉及 IPC 的测试用 `@tauri-apps/api/mocks`，纯函数/组件测试不必强行 mock。
  - `pnpm test:e2e`：构建 E2E 调试包并跑 `e2e/` 的 WebDriver 套件；需要 `tauri-driver` 与匹配 WebView2 的 `e2e/.drivers/msedgedriver.exe`；只使用 `%APPDATA%\com.meik.comics-reader.e2e` 并在结束时清理，不碰真实数据。
  - `pnpm tauri dev`：完整应用。IPC、图片、路由、主题、i18n、阅读器、收藏、下载等运行时问题必须用它验证。
  - `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`：Rust 门禁。
  - `pnpm tauri build`：`bundle.active` 为 false，不生成安装包；需要安装包时显式开启。
- `pnpm dev` / `pnpm preview` 只有前端，没有 Tauri API；端口固定 1420（`strictPort`），`vite.config.ts` 与 `tauri.conf.json` 的 `build.devUrl` 必须一致。
- Windows 下若 `PATH` 中 Android JDK 的 `bin` 排在 `System32` 前，测试二进制会加载其中的 `api-ms-win-crt-*` 桩 DLL 并以 `STATUS_ENTRYPOINT_NOT_FOUND` 退出；先移除该目录。Rust 测试不要引入 `tauri/test` mock runtime（会把 wry/comctl32 链进测试二进制）。

## 核心契约

### 分层与数据

- UI 只能通过 `invoke`、`convertFileSrc` 和窗口 API 访问 Rust；命令返回 `Result<T, AppError>`。`code` 稳定、机器可读，`message` 是技术英文；用户文案只在前端语言包。
- 所有 IPC 走 `src/api/client.ts`：网络命令用 `invokeCommand`（等待 ECH），本地索引命令用 `invokeLocal` 或 `invokeCommand({ waitForEch: false })`；不要在组件里直接 `invoke`。
- 设置保存在 localStorage 的 `comics-reader:settings`（插件前缀 `comics-reader:`）；`settings` 是唯一持久化 store。Rust 不持久化设置；代理/ECH/日志开关是运行时状态，启动和设置变更时分别用 `set_http_proxy`、`set_ech_enabled`、`set_logging_enabled` 同步。
- 应用数据：历史 `app_data_dir()/history.jsonl`；收藏 `app_data_dir()/favorites.json`（version 1）；下载 `app_data_dir()/downloads.json` + `app_data_dir()/downloads/`；日志 `app_log_dir()/app.log`；图片缓存 `app_cache_dir()/images/`。这些内容不能放 localStorage（日志开关偏好仍属于 `settings`）；图片缓存可丢弃。当前没有单实例插件，多实例并发写不保证一致。
- 主题写入 `document.documentElement.dataset.theme`（`light`/`dark`），`App.vue` 同时传 Naive UI `darkTheme`。`setWindowTitle` 同时设置 `document.title` 和原生窗口标题，导航后与语言切换后都要调用。路由用 `createWebHashHistory()`。
- 启动顺序：先应用主题/语言；Tauri 下同步日志、代理，再启动 ECH 初始化。ECH 不阻塞挂载：`runtime.echStarting` 为 true 时首页显示等待，网络命令通过 `invokeCommand` 的 ECH 门禁等待初始化完成。

### 错误

- 序列化形状 `{ code, message, details }`；`details` 只放扁平 string/number，供 i18n 参数使用。图片错误码固定为 `httpStatus` / `emptyResponse` / `notImage` / `tooLarge`；上游搜索/详情变化复用 `httpStatus` / `network` / `json` / `unexpected`，不要新增。
- 新增或修改错误码必须同时改五处：`error.rs`（变体、`ErrorCode`、`code`、`details`）、`src/errors.ts`、`useErrorMessage.ts`、`en.json`、`zh-CN.json`。漏改会在 UI 退化为 `unknown`。

### i18n

- `en.json` 是共享文案 master schema，`MessageKey` 由它派生；`zh-CN` 缺 key 会编译失败。共享文案用 `translate()` / `useAppI18n()`；不要用裸字符串调 `useI18n().t()`。
- 平台标签和专属字段/分组文案写 `src/platforms/<id>/messages.ts`，用 `usePlatformText(messages)`；平台名用 `platformLabel(id)`。两种语言必须同键且占位符一致（`messages.test.ts` 校验）。
- 新增语言：改 `SUPPORTED_LOCALES`、`messages`、`NAIVE_LOCALES`、`detectSystemLocale()`，并给每个平台 `messages.ts` 补该语言。

### 设置、网络与日志

- 阅读方向 `readerDirection`：`ltr` / `rtl` / `ttb`，默认 `ltr`，只在前端。`readerPreloadCount` 3–10，默认 3：当前章向后预热的页数，也是末页预热下一章时的图片数；不是磁盘缓存上限，不同步 Rust。`readerPreloadNextChapter` 默认开：末页取下一章页源并预热前 N 张，只预热，不改变翻页/跳章/历史。
- 代理：无认证 `http://host:port`；启动和保存时 `set_http_proxy` 重建共享 client。关闭开关立即 `set_http_proxy(null)`，无需保存；开启或改地址要保存成功后才写 store。
- ECH：与代理互斥；开启时自动关代理；ECH 开启时 `set_http_proxy(Some)` 返回 `invalidInput`。启动异步初始化，失败回滚 `settings.ech`；初始化期间首页等待，网络命令经门禁等待；非 Cloudflare 站点 plain TLS 回退是预期。plain TLS 仍失败时再尝试不带 SNI 的 TLS（仅传输层失败时；证书仍按目标域名和 WebPKI 校验，因此只有默认虚拟主机证书匹配时才会成功）；成功同样按 host 缓存 10 分钟。内部细节以 `ech/`、`image/http/` 和测试为准。
- 日志：默认关；`set_logging_enabled` 同步，失败回滚并提示。日志页 `/settings/logs` 只读最近 500 条（时间倒序），导出走原生保存对话框；`app.log` 超 1 MiB 轮转为 `app.log.1`（只留一份）；清空同时删两个文件、不弹确认、不显示路径。

### 图片与缓存

- 图片全部由 Rust 处理：`ComicThumb` → `fetch_image` → `convertFileSrc` 资产协议。不要直接用远程 URL、blob/object URL 或浏览器 `fetch`。
- 资产协议三处同时成立：`tauri` 的 `protocol-asset` feature、`tauri.conf.json` 的 `app.security.assetProtocol.enable`、覆盖 `$APPCACHE/images/**` 与 `$APPDATA/downloads/**` 的 scope；缺一为 403/空白。
- 缓存键 = SHA-256（`CACHE_VERSION` + 平台 id + 平台 `cache_version` + 归一化 URL）；写入 `app_cache_dir()/images/<platform>/<hash>.<ext>`，格式按魔数、扩展名决定 MIME。manhuagui 去 query；JM 去 query，`/media/` 还忽略 host；copymanga 去 query 并把 `mangafuna.xyz` 归一化为 `mangafunb.fun`；E-Hentai 的 `/s/...` 页面去 query，真实 H@H 图片 URL 不参与缓存键。当前 `CACHE_VERSION = v1`、各平台 `cache_version = 1`；发布后改平台请求头/语义要提升平台版本，改 URL 归一化/目录要提升 `CACHE_VERSION`。启动后清扫：超 30 天或超 512 MiB 时删除，最近 1 小时保护；下载不参与。
- 设置页“存储”区通过 `get_image_cache_usage` / `clear_image_cache` 展示并清空图片缓存：只操作 `app_cache_dir()/images/`，`downloads/` 不参与。下载排队或进行中时 `clear_image_cache` 返回 `invalidInput`（`details.field = "downloads"`），前端提示先暂停；下载 worker 从图片缓存复制页面，所以清除要占用下载队列槽位。清除成功后前端必须调用 `src/api/images.ts` 的 `clearResolvedImages()` 丢弃内存中的旧路径，清除前发出的 in-flight 结果也不得写回。
- 业务请求统一走 `HttpClient::send`，代理/ECH 才生效；ECH bootstrap/DoH 用自己的 client，不要为业务另建 client。平台可以通过 `ImageProfile::resolve_image_url` 把稳定页面 URL 解析成短时效图片 URL（E-Hentai 的 `/s/...` 页面）。

### 平台注册与解析

- 平台集中在 `src-tauri/src/platforms/`，由 `registry.rs` 单一注册表提供 id、profile、catalog 查找。`ImageProfile` 管图片抓取/缓存钩子，`Catalog: ImageProfile` 另加搜索/详情/阅读能力；`generic` 只有 profile，`manhuagui` / `jm` / `nhentai` / `copymanga` / `ehentai` 是 catalog，只有 catalog 能写历史、收藏和下载。
- `RequestSpec` 默认 GET；需要 JSON POST 的平台用 `json_post`。`Catalog::reader_chapter` 允许平台覆盖多文档页列表，`ImageProfile::resolve_image_url` 默认恒等。
- 只有 `fetch_image` 的 `platform` 参数可省略（按 host 识别，未知 host 落到 generic）；搜索/详情/阅读必须显式传平台。JM 尤其要传 `platform: "jm"`。
- JM 在首个搜索/详情/阅读命令前用 `prepare` 请求 `/setting` 同步 `jm3_version`（成功一次即缓存，失败下次重试），`tokenparam` 用运行时版本。镜像健康只在平台解析成功后记为成功；HTTP 2xx 的挑战页不算。
- JM 走移动端 APP API（`jm_api.rs`）：HTML 站点在部分网络对搜索/专辑都 403；搜索/详情需要 token 签名与 AES 解密，封面由 album id 推导，不需要 Cookie 或 TLS 指纹。页图由 `preprocess` 按 `scramble_id` 还原，需要还原时重新编码为 JPEG，缓存键带 `?decoded=v1`；算法变更要提升 `platforms/jm.rs` 的 `IMAGE_DECODE_VERSION`，`scramble_id` 每章节请求一次并缓存。
- manhuagui 详情优先解 `#__VIEWSTATE`（R18/隐藏列表），失败再读 `.chapter-list`；`h4` 与列表配对为 `chapterGroups`，`ul` 内 `li` 逆序；空标题由前端本地化。阅读页 `/comic/{id}/{chapterId}.html`，`chapterId` 必需；页图 `https://i.hamreus.com{path}{file}`，去 `.webp`/query（`cf` 备用），章节优先 `nextId`/`prevId`，缺失才解析 HTML；不要前端拼地址。`cache_version = 1`，发布后改 URL 归一化要提升。
- copymanga 搜索/详情/章节列表走 APP API `api.manga2026.com`（`/api/v3/...`，`platform: 3`、`version: 3.0.0`），阅读页走 `www.mangacopy.com` 的 `/comic/{path_word}/chapter/{uuid}` HTML，并用页面里的 `cct`/`contentKey` 做 AES-128-CBC 解密取图。详情按 `groups` 分多次请求（`comic_detail` 覆盖默认单请求路径），核心 `chapters` 按分组顺序拍平，`extra.chapterGroups` 保留分组；作品 id 是 `path_word`，章节 id 是 UUID，不能用 `numeric_id`，下载/离线章节键走 `normalize_chapter_id`。阅读页不提供封面，`ReaderChapter.thumb` 为空，历史列表显示首字占位；上一/下一章从页脚解析。图片 CDN 在 `mangafunb.fun` 与 `mangafuna.xyz` 之间互为备用。
- E-Hentai 用 `e-hentai.org` 的 HTML 搜索与 gallery 页，加 `api.e-hentai.org/api.php` 的 `gdata` POST。作品 id 是 `gid-token`（后端再拆 `gid/token`），`single_chapter = true`。搜索是 `next=` 游标分页，每页 25 条且没有总数；游标保存在进程内，`SearchResponse.has_next` 驱动 UI 只露出下一页，`total` 只是已看到的结果数。阅读页列表由 gdata 的 `filecount` 和 `/g/{gid}/{token}/?p=N`（每页 20 个 `/s/` 链接）组成；`ReaderPage.url` 是稳定的 `/s/...` 页面，`resolve_image_url` 在取图时解析出带短时效 `keystamp` 的 H@H URL，缓存键仍用页面 URL。只支持匿名 E-Hentai，不支持 ExHentai/Cookie，也不使用需要登录的 `fullimg` 原图。
- manhuagui 章节用 `NCollapse` 按分组展示，默认展开继续阅读所在组、否则首组，并提供展开/收起全部；copymanga 同样按分组展示，只有一组时直接铺开；JM 多章时列出章节，单章直接进第一章；详情加载态统一用 `ComicDetailSkeleton`，不要退回 `NSpin`。
- 解析依赖上游结构：改 selector/字段就补 fixture，保持 `extra` 字段名稳定。新增平台见“常见改动入口”。

### 搜索、详情与返回

- 搜索会话由 `search` store 管理；`/search` 不携带 query，刷新/深链会被守卫送回首页。搜索页首页图标用 `useBackNavigation`（优先 pop），不要 push 重复首页。
- 搜索返回 `{ total, page_size, result: [{ id, title, thumb, platform, extra }] }`，详情 `{ platform, title, thumb, chapters, extra }`；`platform` 判别，`chapters` 是跨平台核心章节（nhentai/E-Hentai 合成一个 id/title 均为 null 的章节），`extra` 是平台 typed struct 序列化值。不要退回 `Record<string, unknown>` 或 JSON 兜底。游标分页平台额外返回可选 `has_next`，前端用它计算下一页，不要用 `total` 硬凑。
- JM/nhentai 搜索框内容为纯数字时，搜索按钮左侧显示直达按钮；点击用 `router.push` 直接进详情，并按来源写 `state.from`（首页/搜索），不要先发搜索请求。
- 在线详情 `/comic/:platform/:id` 不依赖搜索会话，直接 `get_comic`，刷新/深链可用；失败只显示在线错误态，不用下载索引回退。已下载详情独立为 `/downloads/:platform/:id`，数据来自 `get_download`（摘要 + `chapters` + `detail` 快照；继续阅读/收藏另读 `get_history_entry`/`get_favorite`），用同一套平台组件渲染快照，封面 `coverPath`，开始读首个已完成章节，继续读优先历史章节/页码，失效回退首章第 1 页。
- 进入详情写显式 `state.from`（收藏/历史/下载/搜索/首页），详情只读它；缺失时回首页，不再用 `previousPath()` 或 `search.active` 推断。返回必须走 `useBackNavigation`：上一页就是目标时 `router.back()`；目标在更早历史时 `router.go(-n)` 一次退到；否则 `router.replace`。不要用 `router.push` 做返回。
- 首页→搜索结果用 `useRouteTransition()` 的 `startViewTransition`；不要用延迟 `router.push` 等动画。从搜索结果回首页仍走 `goBack`（优先 pop）。

### 阅读器

- 阅读器是详情的子路由：在线 `/comic/:platform/:id/read/:chapterId?`，已下载 `/downloads/:platform/:id/read/:chapterId?`。详情作为父路由保持挂载，关闭阅读器只弹出子路由，不重新请求详情；阅读器只从详情进入。`ComicReaderView` 负责页源、`?page=`、返回详情与错误态。
- 单页视口：三页槽位（横向左/中/右，纵向上/中/下）组成 3 倍轨道，当前页居中，翻页整体滑动，无背景缝；方向决定轴、手势和方向键。缩放后拖拽平移，滚轮翻页。
- 上下栏绝对定位、默认隐藏、不占 stage；点中间 1/3 显隐，左右 1/3 翻页；滑动/滚轮/翻页不自动唤出。HUD 仅在菜单隐藏时显示页码，避开右侧安全区并点击穿透；移动端电量用 `navigator.getBattery()`，菜单展开时放顶栏、隐藏时放 HUD，桌面或不支持时隐藏。
- 页码用 `router.replace` 写 `?page=`，不新增历史，越界夹到 `1..pages.length`，空页显示空态。首/尾页继续翻页自动切章：上一章从末页进入（`?page=last`，加载后换真实页码），下一章从第一页进入；没有相邻章时用 `useBackNavigation` 回详情。按钮、点击区、滑动、滚轮、方向键共用这条边界逻辑；底部章节按钮仍从第一页打开。
- 章内预加载走 `fetchCachedImage`（同 `platform + url` 合并），`useReaderPreload` 200ms 防抖、2 并发；失败静默，切页/切章停止旧队列。末页跨章由 `useNextChapterPreload` 取下一章页源并预热前 N 张，不展示、不写历史。`src/api/reader.ts` 缓存最近 4 个 `ReaderChapter` 并共享 in-flight，`src/api/images.ts` 缓存最近 64 个已解析路径，均只在内存中有界保留。
- 页图仍走 `fetch_image`，不要直接请求 CDN；本地 `localPath` 必须让预加载跳过。JM/manhuagui/copymanga 的上一/下一章从页源解析，不要前端拼接；JM 省略 `chapterId` 时用 album id，manhuagui 无章节深链先取详情首章并 `router.replace`；E-Hentai 的页图是 `/s/...` 页面，由 `resolve_image_url` 解析，不要前端拼 H@H 地址；空页统一 `notFound`。
- 移动端双指缩放只处理两个触摸指针，1x–4x，围绕两指中点平移；松开一指转单指平移，且不得触发 tap 翻页。桌面鼠标/触控板行为不变；缩放、全屏、`Home`/`End` 等现有行为不要破坏。

### 阅读历史

- 按作品 `platform + id` 记录，只保留最后章节/页码；成功加载页源时写入，翻页防抖，离开/切章 flush；只浏览详情不产生记录。
- `history.jsonl` 每行一个 `HistoryEntry`，首次命令重放为内存索引；上限 10,000，超出丢最旧；行数超过 2 倍上限或加载时裁剪过会压缩；读取跳过坏行并过滤未知平台/空字段/非法页码，按 `updatedAt` 倒序。
- 命令支持分页列表、单条读取和写入；每页默认 50、最大 200，只取当前页。记录失败静默（Rust 记日志）。历史不可删除/清空：没有 `remove_history`/`clear_history`，不要恢复。
- 历史行进在线详情；详情“继续阅读”恢复 `chapterId`/`page`，章节失效时回退首章；已下载详情走 `download-reader`。`ReaderChapter.thumb` 仅供历史封面，仍走 `ComicThumb`。

### 收藏

- 按 `platform + id` 收藏，只存快照 `title` / `thumb` / `addedAt` / `groupId`，不做远程刷新；上游删除后仍保留。
- `favorites.json`（version 1，`groups` + `entries`）首次加载为内存索引，整表原子重写（临时文件 + `sync_all` + rename）。只接受当前版本；旧版文件按不支持处理，不做迁移。
- 一个作品只属于一个分组；移动分组只改 `groupId`；默认分组 id 固定 `default`，名称空字符串由前端本地化，不可改名/删除。
- `add` 幂等：无 `groupId` 的新条目进默认分组，已有条目保留原分组，显式 `groupId` 则移动；保留 `addedAt` 并刷新标题/封面；删除分组把收藏移到默认分组。
- 收藏页支持全部/默认/自定义筛选、计数、创建/重命名/删除；按 `addedAt` 倒序；点击进详情；取消收藏不弹确认；缩略图走 `ComicThumb`，不要逐条请求远程详情。
- 详情页收藏按钮：短按 toggle（未收藏进默认分组，已收藏取消），长按/右键/`Shift+Enter` 打开分组选择器；长按 500ms，移动/取消/离开要取消，长按后的 click 必须抑制。先 `get_favorite` 取分组；查询失败按未收藏处理；请求带 token，切换作品丢弃旧结果。
- 损坏文件按空列表处理，并在下一次写入前备份 `.bak`；更高 version 返回 `unexpected` 且拒绝写入；复用 `invalidInput` / `io` / `json` / `tauri` / `unexpected`。

### 下载

- v1 只按作品下载整部章节，不支持按章勾选；详情页提交完整章节列表，nhentai 传空列表由 Rust 合成单章。下载页不是缓存页，列表项只进 `/downloads/:platform/:id`，不直接开阅读器。
- 下载开始时存在线 `ComicDetail` 快照（去掉章节列表）；`get_download` 返回 `DownloadDetail`（摘要 + `chapters` + `detail`）。已下载详情用同一套平台组件渲染快照，隐藏在线章节，只展示已下载章节；旧记录无快照时回退到封面/标题/章节视图，不做迁移。
- 文件：`downloads.json`（version 1）+ `downloads/<platform>/<work-id>/`；每章完成写 `manifest.json`，页面与索引原子写。不要放 `app_cache_dir()`，也不要依赖图片缓存清理。复用 `image::fetch_image_inner` / `get_reader_chapter_inner`，页面从缓存复制；重复磁盘占用可接受，不要另写平台请求逻辑。
- 同一时间一个作品；章节按详情顺序、页页串行；索引只在开始、章节边界、暂停和结束写，不要每页重写。
- 状态 `queued` / `downloading` / `paused` / `completed` / `partial` / `failed`；启动/首次加载把遗留 active 改为 `paused`，不自动联网续传。暂停/续传用 run id 使旧 worker 失效；run id 进程内单调分配、删除后也不复用；只允许删除非 active。
- `get_downloaded_chapter` 返回带 `localPath` 的 `ReaderChapter`；前端 `get_reader_chapter` 封装先查本地再等 ECH。本地章节的 `previous`/`next` 只含已下载章节，离线翻章不会跳到联网章节。已下载详情只用已完成章节；在线详情不回退下载索引。
- 本地路径必须同时被资产协议放行；`ReaderPageImage` / `useCachedImage` / `ComicThumb` 优先 `localPath`，阅读器预加载跳过本地页。
- 进度用活动期间轮询（下载页 1s、详情页 1.5s），页面卸载清定时器。索引/manifest 损坏按空/None 处理并记日志；更高版本返回 `unexpected` 并拒绝写入。

### 安全与生成文件

- 命令必须注册进 `lib.rs` 的 `generate_handler!`；插件权限必须进 `capabilities/default.json`，否则运行时 “not allowed”。CSP 为 `null`；不要引入远程 HTML 或 `v-html`。
- 不要手改生成文件：`src/components.d.ts`、`src-tauri/gen/schemas/`、`src-tauri/gen/android/**/build/`、`src-tauri/target/`、`dist/`、`src-tauri/icons/` 下的 PNG。用工具重新生成。`bundle.active` 为 false；需要安装包时显式开启。

## 编码约定

### TypeScript / Vue

- `<script setup lang="ts">` + Composition API；避免 `any`，未知输入用 `unknown` + 类型守卫；仅类型导入用 `import type`；解构 store 用 `storeToRefs()`。
- 依赖设置/语言的值用 `computed`；下拉选项和消息在 `computed` 内构造。
- 异步用 `try/catch/finally` 并复位 loading；错误用 `useErrorMessage()` 呈现；监听器/定时器在 `onBeforeUnmount` / `onScopeDispose` 清理。

### Rust

- 命令返回 `error.rs` 的 `Result<T>`；失败用 `AppError`，优先 `From` + `?`；参数简单可序列化，返回数据而不是展示文案。
- 日志用 `logging::info` / `warn` / `error`；开关关闭时它们直接返回；`target` 保持稳定，不记录认证信息或秘密。
- 新命令必须注册；错误序列化和新 wire 结构补测试（`#[cfg(test)] mod tests`）。运行 `cargo fmt`，Clippy 以 `-D warnings` 通过。

### UI / 样式 / 安全区

- 全局 reset 在 `src/styles/base.scss`；组件样式放 `<style scoped>`。优先 Naive UI；自动导入已配置，不要手改 `components.d.ts`。
- `useMessage()` 必须在 `NMessageProvider` 下；没有 `NDialogProvider`，删除确认用 `NPopconfirm` / `NModal`，不要引入 `useDialog()`。详情加载态用 `ComicDetailSkeleton` 并保持 hero/body 布局，不要退回通用 `NSpin`。
- 顶部安全区由 `App.vue` 的 `.safe-area-top` 统一负责（sticky、`--app-safe-area-top`、`--app-background`）；页面不要用 `env(safe-area-inset-top)` 的 margin/padding 或 `max-width` / `pointer: coarse` 模拟。`index.html` 的 viewport 必须带 `viewport-fit=cover`。
- 四边安全区统一暴露为 `--app-safe-area-top/right/bottom/left`；`NMessageProvider` 的 teleport container 用 `container-style` 避开顶部/左右，Modal 的 `.n-modal-body-wrapper` 用它约束四边。新增 toast/Modal/Popover 等浮层必须接入安全区。
- 阅读器底部覆盖栏用 `padding-bottom: env(safe-area-inset-bottom)`，不要加独立占位条。

## 常见改动入口

| 改动 | 关注点 |
| --- | --- |
| 新 Tauri 命令 | `src-tauri/src/lib.rs` 定义并注册；`src/api/*.ts` 封装；失败返回 `AppError`。 |
| 新错误码 | `error.rs`、`src/errors.ts`、`useErrorMessage.ts`、两个语言包，五处一次改完。 |
| 新界面文案 | 共享文案写 `en.json` + `zh-CN.json`；平台专属写 `src/platforms/<id>/messages.ts`，用 `usePlatformText`。 |
| 新路由 | `src/views/` + `router/index.ts`（`RouteTitleKey`/meta）+ 两个语言包；放在现有 RouterView 内并继承安全区。 |
| 新平台 | 前端：`src/platforms/<id>/`（types/index/messages/ResultCard/Detail/icon/fixtures），注册 `platformList` 并补 `PlatformCard`/`PlatformDetail` 分支；后端：`src-tauri/src/platforms/<id>.rs` 实现 trait，`platforms/mod.rs` 加 `mod`，`platforms/registry.rs` 注册。 |
| 改阅读器/页源 | 见「阅读器」；`components/reader/`、`ComicReaderView`、`api/reader.ts`、`src/platforms/`、`src-tauri/src/platforms/`、`image/mod.rs`。 |
| 改收藏/下载 | 见「收藏」「下载」；对应 View、`api/*.ts`、Rust 模块；下载还要改 `tauri.conf.json` 资产 scope。 |
| 改图片/网络 | `src-tauri/src/image/`、`tauri.conf.json` 资产 scope；业务请求必须走 `HttpClient`。 |

## 验证

- 前端改动：`pnpm test` + `pnpm build`；Rust 改动：`cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`；E2E（本地 Windows）：`pnpm test:e2e`，只用 `e2e/` 预置的离线数据，不访问上游。没有 CI，本地检查是唯一门禁。
- 契约：改 Rust wire 结构或 TS 类型时同步 `src/platforms/<id>/fixtures/wire.json` 与 `contracts/*.json`。
- 手动冒烟（按改动选做）：搜索→详情→返回；设置持久化/代理/ECH/日志；阅读器五平台、手势/缩放、首尾切章；历史/收藏/下载的返回、继续阅读、分组、暂停/续传/删除与离线详情；移动端安全区与电量。
- 前端测试跑在 jsdom，不启动 Tauri；Pinia 4 把插件排队到 `app.use(pinia)` 之后，测试里创建 store 前要 `createApp(...).use(pinia)`，否则持久化不生效。

## 尚未实现

- JM 域名自动更新：内置域名失败会轮换，但不会抓发布页；需要时从发布页获取，不要搬 Python 登录/收藏/下载框架。
- 按章下载/导出：v1 整部作品；按章勾选、CBZ/文件夹导出未实现。存储已按章组织，扩展时不需要改索引版本。
- 更多平台：按“新平台”入口实现；先确认接口、登录/付费/地区限制与抓取许可；至少补搜索/详情 fixture。

## 提交

- Conventional Commits，祈使句、小写 subject，一次一个逻辑变更；依赖变化提交两个 lockfile；不提交构建产物、生成文件、`scripts/` 或无关文件。
- 默认分支 `master`；没有 ESLint/Prettier 配置；保持 diff 聚焦。
