# AGENTS.md

面向 AI 编码代理的仓库协作指南。修改代码前请先阅读本文件；当命令、架构或跨层契约发生变化时，请同步更新本文件。

## 项目概览

这是一个**早期阶段的 Tauri 2 漫画阅读桌面应用**，前端使用 **Vue 3 + TypeScript + Vite**，后端使用 **Rust**。

当前界面仍是脚手架，还不是完整的阅读器：

- 首页是 Google 风格搜索页：选择平台、输入关键词后进入独立的 `/search` 结果页；结果页负责分页与错误/空态，缩略图由 Rust `fetch_image` 缓存后通过资产协议显示；
- 结果卡片进入 `/comic/:platform/:id` 详情页，manhuagui/JM/nhentai 各自渲染封面、元数据和标签；详情页可直接刷新或通过深链接打开；
- `search_comics` / `get_comic` 为 manhuagui、jm、nhentai 提供统一的关键词搜索与精确 id 详情，搜索与详情分别返回带 `platform` 判别的强类型扩展字段；
- 首页搜索框下方有收藏、下载、历史、设置快捷入口；前三个目前指向占位页，设置页已提供主题、语言、HTTP 代理与 ECH 开关。
- 主题、语言、HTTP 代理与 ECH 偏好从 webview 本地恢复，并可在设置页修改；代理配置与 ECH 开关会同步到 Rust 的 HTTP client。

不要假设项目里已经有漫画库、章节浏览、下载器、数据库或阅读器引擎；这些都属于待实现的新功能。

`README.md` 仍是上游 Tauri/Vue 模板，不能可靠描述本项目。请以本文件、`package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 为准。

当前 ECH 接入的实施计划见 [ECH_INTEGRATION_PLAN.md](ECH_INTEGRATION_PLAN.md)；该文件是多智能体协作和交接的协调文档。

### 技术栈

- **前端：** Vue 3.5（`<script setup>`）、Vue Router 4（hash 路由）、Pinia 4 + `pinia-plugin-persistedstate`、vue-i18n 11、Naive UI 2（自动导入）、Sass、TypeScript 6。
- **后端：** Rust edition 2024、Tauri 2（启用 `protocol-asset`）、`reqwest`（rustls）负责抓取、`rustls`/`tokio-rustls`（ring + aws-lc-rs）用于 ECH 握手、`hickory-resolver`（DoH）查询 ECH 配置、`scraper`/`regex`/`html-escape` 解析 HTML、`serde_json` 解析 JSON、`sha2` 生成缓存键、`thiserror` 2、Tauri 插件 `opener`。
- **工具链：** pnpm（单包项目，不是 monorepo）、Vite 8、`vue-tsc`。
- **平台：** Windows/macOS/Linux 桌面端。Android 工程已在 `src-tauri/gen/android` 初始化；iOS 尚未初始化。

## 环境与命令

### 环境要求

- Node.js `^20.19.0 || >=22.12.0`（Vite 8 的要求）与 pnpm。`pnpm-lock.yaml` 已提交；请使用 pnpm，不要用 npm 或 yarn。
- 支持 edition 2024 的 Rust stable（1.85+），以及对应平台的 [Tauri 环境依赖](https://tauri.app/start/prerequisites/)（Windows 需要 WebView2）。
- Python 3.15+ 仅用于本地 `scripts/` 工具；该目录已被 gitignore，不属于应用本身。
- 只有在处理移动端时才需要 Android SDK、NDK 与 JDK。

### 常用命令

除特别说明外，均在仓库根目录执行。

| 命令 | 作用 |
| --- | --- |
| `pnpm install` | 根据 `pnpm-lock.yaml` 安装前端依赖。 |
| `pnpm dev` | 只启动 Vite 开发服务器（`http://localhost:1420`）。此时 Tauri API 不可用。 |
| `pnpm tauri dev` | 启动完整桌面应用；通过 `beforeDevCommand` 自动拉起 Vite。实际测试请用这个命令。 |
| `pnpm build` | 执行 `vue-tsc --noEmit`，再将前端构建到 `dist/`。不会编译 Rust。 |
| `pnpm tauri build` | 完整 release 构建。`bundle.active` 为 `false`，因此不会生成安装包。 |
| `pnpm preview` | 预览已构建的前端。仍然没有 Tauri API。 |
| `pnpm tauri <args>` | 透传给 Tauri CLI，例如 `pnpm tauri info`、`pnpm tauri android dev`。 |
| `cargo test` | 运行 Rust 单元测试，在 `src-tauri/` 下执行。 |
| `cargo fmt --check` | 检查 Rust 格式，在 `src-tauri/` 下执行。 |
| `cargo clippy --all-targets -- -D warnings` | 以 warnings 作为错误进行 Rust lint，在 `src-tauri/` 下执行。 |

说明：

- Vite 固定使用 `1420` 端口，并设置了 `strictPort: true`。端口被占用时开发服务器会直接失败；不要在未同步修改 `build.devUrl` 与 `vite.config.ts` 中 HMR 配置的情况下改端口。
- `pnpm build` 不会编译 Rust；修改 Rust 后必须单独运行 Cargo 检查。
- `scripts/` 已写入根目录 `.gitignore`，是本地诊断工作区，不参与构建，也不会随应用发布。

## 仓库结构

```text
.
├─ src/                          Vue 应用
│  ├─ main.ts                    启动：Pinia、i18n、router；挂载前应用主题/语言/代理并启动 ECH 初始化
│  ├─ App.vue                    Naive UI Provider、顶部安全区与 RouterView 外壳
│  ├─ components/
│  │  ├─ ComicThumb.vue          结果缩略图：Rust 缓存 + 资产协议
│  │  ├─ ComicSearchBar.vue      搜索表单：平台选择、关键词输入、提交
│  │  ├─ HomeShortcuts.vue       首页快捷入口：收藏/下载/历史/设置
│  │  ├─ ShortcutIcon.vue        快捷入口 SVG 图标
│  │  ├─ PlatformPicker.vue      平台下拉菜单与图标
│  │  ├─ ComicSearchResults.vue  结果区：错误/加载/空态、网格、分页
│  │  ├─ ComicResultCard.vue     结果卡片分发器：按 item.platform 选择平台卡片
│  │  ├─ result-cards/
│  │  │  ├─ ResultCardShell.vue  平台卡片共用外壳：封面、标题、内容插槽
│  │  │  ├─ ManhuaguiResultCard.vue
│  │  │  ├─ JmResultCard.vue
│  │  │  └─ NhentaiResultCard.vue
│  │  └─ comic-detail/
│  │     ├─ DetailShell.vue      详情页共用封面、标题与摘要外壳
│  │     ├─ DetailSection.vue    详情分节容器
│  │     ├─ DetailTagGroup.vue   详情标签组
│  │     ├─ ManhuaguiDetail.vue
│  │     ├─ JmDetail.vue
│  │     └─ NhentaiDetail.vue
│  ├─ assets/platforms/          平台真实图标（favicon/logo；PNG 或按像素重绘的 SVG）
│  ├─ views/
│  │  ├─ HomeView.vue            首页：搜索入口
│  │  ├─ SearchView.vue          结果页：首页图标、搜索栏与结果区
│  │  ├─ ComicDetailView.vue     详情页：加载/错误/重试与平台内容分发
│  │  ├─ ShortcutPlaceholderView.vue  收藏/下载/历史的占位页
│  │  ├─ SettingsView.vue        设置页：主题、语言、代理与 ECH
│  │  └─ NotFoundView.vue        404 页面
│  ├─ router/index.ts            Hash 路由、守卫、详情路由、滚动行为与原生窗口标题
│  ├─ stores/
│  │  ├─ index.ts                Pinia + 持久化命名空间
│  │  ├─ runtime.ts              非持久化启动状态：ECH 初始化等待
│  │  ├─ search.ts               内存态搜索会话：平台、关键词、分页与请求竞态
│  │  └─ settings.ts             持久化的主题、语言、代理与 ECH 偏好
│  ├─ composables/
│  │  ├─ useRouteTransition.ts   首页/结果页的 View Transition 导航
│  │  ├─ useTheme.ts             系统/浅色/深色解析 + <html data-theme>
│  │  ├─ useLocale.ts            语言、Naive UI locale 对象、窗口标题同步
│  │  └─ useErrorMessage.ts      错误码 → 本地化消息
│  ├─ i18n/
│  │  ├─ index.ts                类型化 i18n 实例、语言检测、translate()
│  │  └─ messages/               en.json（主 schema）与 zh-CN.json
│  ├─ shortcuts.ts               首页快捷入口的 feature/path/label 映射
│  ├─ api/comics.ts              平台检索/详情的前端类型与 invoke 封装
│  ├─ api/proxy.ts               代理配置的前端校验与 set_http_proxy 封装
│  ├─ api/ech.ts                 ECH 开关与状态命令的 invoke 封装
│  ├─ errors.ts                  前端错误契约与 AppError
│  ├─ styles/base.scss           全局 reset/基础样式与安全区留白
│  └─ components.d.ts            由 unplugin-vue-components 生成；不要手改
├─ src-tauri/
│  ├─ src/lib.rs                 Tauri Builder、命令注册、插件初始化
│  ├─ src/error.rs               AppError 序列化、错误码、测试
│  ├─ src/ech/                   ECH bootstrap / DoH / TLS 模块
│  │  ├─ mod.rs                  命令、EchStatus 与模块导出
│  │  ├─ bootstrap.rs            AliDNS/360 仅查询 cloudflare-ech.com 的 ECH
│  │  ├─ doh.rs                  Cloudflare DoH over ECH（固定 104.16.248.249 / 104.16.249.249）
│  │  ├─ session.rs              EchSession、CloudflareResolver（60 秒 DoH 缓存）与 reqwest TLS 配置
│  │  └─ tls.rs                  rustls ECH 握手与普通 TLS fallback
│  ├─ src/image/                 图片抓取、缓存与平台检索
│  │  ├─ mod.rs                  fetch_image / search_comics / get_comic / set_http_proxy
│  │  ├─ cache.rs                缓存键、查找、原子写入、格式识别
│  │  ├─ http.rs                 共享 client、代理、ECH runtime（10 分钟懒刷新）、图片下载与文档抓取
│  │  ├─ model.rs                搜索/详情 wire 类型
│  │  └─ platform/               平台 trait、请求头与 HTML/JSON 解析
│  ├─ src/main.rs                桌面端二进制入口
│  ├─ capabilities/default.json  窗口权限（标题、opener）
│  ├─ tauri.conf.json            窗口、构建、安全与打包配置
│  ├─ icons/                     Tauri 生成的应用图标
│  └─ gen/
│     ├─ android/                已纳入版本管理的 Android 工程（Gradle/Kotlin/资源）
│     └─ schemas/                生成的 capability schema；已被 git 忽略
├─ public/                       由 Vite 复制的静态资源
├─ scripts/                      仅本地使用的 Python DoH 诊断脚本（已 gitignore）
├─ vite.config.ts                Vue、Naive UI 自动导入、Tauri 开发服务器设置
└─ pnpm-workspace.yaml           仅 pnpm 配置；不是 workspace monorepo
```

搜索或审查代码时请忽略：`node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/**/build/`、`scripts/.venv/`。

## 架构与数据流

### 启动顺序

1. `src/main.ts` 创建 Vue 应用，并注册 Pinia、i18n 与 router。
2. 同步读取 `useSettingsStore(pinia)`（持久化状态会从 `localStorage` 恢复），在挂载前应用已保存的主题、语言与 HTTP 代理（代理通过 `set_http_proxy` 写入 Rust）；若持久化 ECH 为 true，同时启动异步 `set_ech_enabled(true)`，并把 `runtime.echStarting` 置为 true；首页在该标志为 true 时显示等待，不渲染搜索栏，初始化完成/失败后再允许搜索。
3. 应用挂载到 `#app`。
4. `App.vue` 通过 `NConfigProvider` 提供 Naive UI 的主题与语言，并用 `NMessageProvider` 包裹路由视图。`useTheme()` 与 `useLocale()` 负责保持运行时状态同步，且只在这个外壳中调用一次。

### 分层

- Vue UI 只通过 Tauri JS API 与系统交互：`@tauri-apps/api/core` 的 `invoke`、`convertFileSrc` 以及窗口 API。
- Rust 命令返回数据或类型化错误。**面向用户的文案只放在前端语言包中**，绝不放在 Rust 命令返回值里。
- 设置保存在 webview 的 `localStorage` 中，键名为 `comics-reader:<store-id>`。Rust 不持久化设置；HTTP 代理与 ECH 开关是运行时状态，前端在启动和保存设置时分别通过 `set_http_proxy` 与 `set_ech_enabled` 同步到内存中的共享 client。
- 路由使用 `createWebHashHistory()`。不要在未处理 Tauri webview 路由回退的情况下改成 web history。
- 文档主题写入 `document.documentElement.dataset.theme`（`light` / `dark`）；`App.vue` 同时给 Naive UI 传入匹配的 `darkTheme` 对象。
- Tauri 不会自动把 `document.title` 同步到原生窗口。`router/index.ts` 会显式调用当前 Tauri 窗口的 `setTitle()`；切换语言后 `useLocale` 会再次调用。

### 错误契约（关键）

Rust 命令返回 `Result<T, AppError>`。失败时会序列化为：

```json
{ "code": "invalidInput", "message": "invalid `url`: scheme must be http or https", "details": { "field": "url" } }
```

`code` 是稳定、机器可读的标识；`message` 是供日志与兜底展示的技术英文；`details` 是作为 i18n 参数使用的原始上下文。

该契约横跨五处：

1. `src-tauri/src/error.rs` — `AppError` 变体、`ErrorCode`、`code()`、`details()` 与 `ErrorCode::as_str()`。
2. `src/errors.ts` — `ErrorCode` 联合类型与 `ERROR_CODES` 列表。（`"unknown"` 是仅前端使用的兜底值。）
3. `src/composables/useErrorMessage.ts` — 穷举式 `ERROR_MESSAGE_KEYS` 映射。
4. `src/i18n/messages/en.json` — `errors.<code>` 文案。
5. `src/i18n/messages/zh-CN.json` — 对应中文翻译。

规则：

- `details` 必须保持扁平、原始类型。前端归一化只保留 string 与 number；不要传 boolean、数组或嵌套对象。
- 命令不要返回裸字符串，也不要 panic；应新增 `AppError` 变体。
- 只加 Rust 变体而不加前端映射时，运行时会回退为 `"unknown"`。必须在同一次改动中更新所有层。
- 序列化后的 payload 结构若有变化，必须同步更新 `error.rs` 中的单元测试。
- 图片抓取使用 `httpStatus`、`emptyResponse`、`notImage`、`tooLarge` 四个错误码；`details` 分别带 `url`、`status`、`contentType`、`limit`。
- 检索/详情对未知平台、空 query、page < 1、非数字 id 使用 `invalidInput`；上游 HTTP 错误、JSON 解析错误与 HTML 结构变化分别复用 `httpStatus`/`network`/`json`/`unexpected`，不新增错误码。

### i18n 契约

- `src/i18n/messages/en.json` 是主 schema。`MessageSchema = typeof en`，`MessageKey` 由它派生，因此其他语言包缺少 key 会直接编译报错。
- 新代码请使用 `src/i18n` 导出的 `translate()` 或 `useAppI18n()`。不要用 `useI18n().t()` 传裸字符串，那会绕过编译期 key 校验。
- 每个新 key 必须在同一次改动中同时加入 `en.json` 与 `zh-CN.json`，并保持占位符（`{name}`、`{status}`）一致。
- 错误要以数据形式保存（code/details），在渲染时翻译，而不是提前拼成字符串。搜索 store（`src/stores/search.ts`）的 `error` ref 是参考模式，这样切换语言时提示会同步更新。
- 新增语言时需要更新：`SUPPORTED_LOCALES`、`messages` 映射、`useLocale.ts` 中的 `NAIVE_LOCALES`，必要时还要改 `detectSystemLocale()`。

### 设置、主题与持久化

- `src/stores/settings.ts` 是唯一持久化的 store；`src/stores/search.ts` 只保存内存态搜索会话，刷新后失效。插件在 `src/stores/index.ts` 中为键名加上 `comics-reader:` 前缀。
- 只持久化用户偏好或小型持久状态。不要持久化大对象、瞬态加载标记或密钥。
- 给 `SettingsState` 新增字段会自动持久化，老用户通常会回落到 store 的初始值。重命名或删除已持久化字段则需要迁移，否则旧数据会被静默忽略。
- `useTheme()` 与 `useLocale()` 只在 shell 层调用一次。其他组件直接读 `useSettingsStore()`；重复调用 `useTheme()` 会重复注册 media query 监听器。
- 首页提供设置快捷入口，设置页可切换主题与语言，并可保存 HTTP 代理；主题/语言立即生效。关闭代理开关会立即通过 `set_http_proxy(null)` 清除代理并写回 store，无需点击保存；开启代理或修改地址仍需点击“保存并应用”，且只有 `set_http_proxy` 成功后才写回 store，避免持久化无法应用的地址。
- `settings.ech` 通过 `set_ech_enabled` 同步到 Rust。开启时设置页先关闭代理，后端 `enable_ech` 也会清空代理；关闭时 `disable_ech` 丢弃 runtime，普通 client 恢复。启动时若持久化 ECH 为 true，`main.ts` 在挂载前异步初始化，失败会回滚 `settings.ech`。`set_http_proxy(Some)` 在 ECH 开启时返回 `invalidInput`，设置页在 ECH 开启时禁用代理控件。`get_ech_status` 仅保留为诊断命令，设置页不展示状态。启动阶段若 ECH 正在初始化，首页显示等待状态并隐藏搜索栏。
- 设置页的“网络设置”区域把 HTTP 代理与 ECH 放在一起，不再为 ECH 单独分组；区块标题用“网络设置”，代理开关文案用“HTTP 代理”，不要写回旧的“使用代理”或单独 ECH 章节。

### 远程 HTTP、图片缓存与平台检索

- 图片抓取全部在 Rust 中完成：`ComicThumb.vue` 对可见缩略图调用 `fetch_image` 命令（`src-tauri/src/image/`），由 `reqwest` 请求并缓存，webview 通过资产协议读取文件；首页本身不再直接请求图片站点。
- 搜索会话由 `src/stores/search.ts` 管理平台、关键词、分页与请求竞态；`HomeView.vue` 只负责搜索入口，提交后跳转无查询参数的 `/search`；`SearchView.vue` 绑定 store 并复用 `ComicSearchResults.vue`；`/search` 的守卫在没有内存态搜索时重定向首页，首页图标始终回首页；两页之间通过 `useRouteTransition` 触发 View Transition，不支持时代码会直接导航。`ComicSearchBar.vue`（含 `PlatformPicker.vue`）负责输入，`ComicSearchResults.vue` 负责错误/加载/空态与分页，`ComicResultCard.vue` 只按 `item.platform` 分发到 `result-cards/` 下对应平台的卡片，字段解释与排版留在各平台卡片内；`ComicThumb.vue` 用 `IntersectionObserver` 懒加载缩略图，避免一次性请求整页图片。
- 平台差异集中在 `src-tauri/src/image/platform/`：每个平台实现 `Platform`（`id`、`matches`、`configure`），主流程只负责下载、校验与缓存。当前支持 `generic`、`manhuagui`、`jm`、`nhentai`。
- 命令的 `platform` 参数可省略：省略时按 URL host 自动识别，未知 host 回退 `generic`；调用方已知平台时应显式传入，JM 等域名轮换的站点尤其应显式指定。
- 命令校验响应（非 2xx、空响应、超过 32 MiB、非图片格式）后，用“平台 + 平台缓存版本 + 缓存用 URL”的 SHA-256 作为文件名；manhuagui 和 jm 会在计算 key 前去掉图片 URL 的 query（签名/过期参数不影响命中），但下载仍使用完整 URL，写入 `app_cache_dir()/images/<platform>/<hash>.<ext>`；同一 URL 在同一平台下再次请求会直接命中缓存。
- `generic` 只带浏览器 UA/Accept，不设置 `Referer`；各平台模块负责 `Referer`、`Accept-Language` 等 header。修改某平台请求头时递增它的 `cache_version`，只失效该平台缓存。
- 前端用 `convertFileSrc(cached.path)` 生成资产协议 URL 交给 `<img>`，从本地缓存目录读取图片。不要再改回 blob/object URL 或浏览器 `fetch`。
- 资产协议需要三处同时成立：`tauri` 的 `protocol-asset` feature、`tauri.conf.json` 的 `app.security.assetProtocol.enable`、覆盖 `$APPCACHE/images/**` 的 scope。缺任一项通常表现为图片 403 或空白。
- 图片格式优先按魔数判断，文件扩展名决定资产协议返回的 MIME；新增格式时要同时更新 `ImageFormat` 的魔数、扩展名与 MIME 映射。
- `CACHE_VERSION` 变化会让旧缓存自然失效；当前没有自动清理或容量上限，缓存会随浏览增长。
- 用户可在设置页启用自定义 HTTP 代理（无认证，`http://host:port`）。启动时和保存后由 `set_http_proxy` 重建共享 `reqwest::Client`；`fetch_image`、`search_comics` 与 `get_comic` 都通过 `HttpClient::send`：ECH 关闭时走普通 client（可走代理），ECH 开启时走 `no_proxy` 的 ECH/plain client。代理不参与图片缓存键；关闭代理时保留 reqwest 默认行为（环境变量代理仍可能生效）。
- `fetch_ech_config` 在 `src-tauri/src/ech/bootstrap.rs` 中用 `hickory-resolver` 的 DoH 传输查询 `cloudflare-ech.com` 的 HTTPS RR，返回全部记录与 base64 ECHConfigList；它仍是诊断命令，不参与业务请求，业务请求使用 `set_ech_enabled` 构建的 runtime。端点 host 先经系统解析，DoH 连接的 TLS 由 `webpki-roots` 校验；hickory 不读取应用的 HTTP 代理设置。
- `probe_ech_handshake(domain, dohEndpoint)` 在 `src-tauri/src/ech/session.rs` / `tls.rs` 中固定查询 `cloudflare-ech.com` 的 ECHConfigList，再把同一份配置应用到目标域名；目标 A/AAAA 通过 `src-tauri/src/ech/doh.rs` 的 Cloudflare DoH over ECH 解析，固定连接 `104.16.248.249` / `104.16.249.249`，不再使用系统 DNS 或 AliDNS/360 解析目标。握手返回 `configDomain`、`echStatus`、TLS 版本、ALPN、HTTP 状态与 `fallback`；非 Cloudflare 站点 ECH 失败后自动回退普通 TLS。该探针仍独立于业务请求；由于 reqwest/hickory 使用 ring 而 ECH 使用 aws-lc-rs，`run()` 启动时会安装 ring 作为进程默认 provider。
- ECH runtime 的配置 TTL 为 10 分钟；到期后的首个 HTTPS 请求会重新 bootstrap 并替换 runtime，失败时保留旧 runtime 并在 60 秒后重试。Cloudflare DoH 的 A/AAAA 结果按 60 秒缓存，ECH 与 plain client 共享同一个 `CloudflareResolver`，避免回退时重复查询。
- `nhentai.net` 在 AliDNS/360 上的地址答案会被污染或不一致（虚假 IPv6、0.0.0.0 或错误 IPv4），但 `probe_ech_handshake` 已通过 Cloudflare DoH 解析目标，因此可直接直连：测试 `queries_nhentai_via_cloudflare_doh_over_ech` 结果为 `echStatus=accepted`、TLS1.3、HTTP 200、`fallback=false`，全程未使用本地代理。
- 检索与详情命令为 `search_comics(platform, query, page)` 与 `get_comic(platform, id)`。平台通过 `search_request`/`parse_search`/`detail_request`/`parse_detail` 构建 URL 并解析响应，共享的 `fetch_document` 负责请求、状态码与大小限制。
- 详情页为 `/comic/:platform/:id`，不依赖内存态搜索会话，刷新或直接打开也能请求 `get_comic`；返回按钮在搜索会话仍有效时回 `/search`，否则回首页。路由 `scrollBehavior` 让新页面从顶部开始，详情加载后用 `setWindowTitle` 把漫画标题写入原生窗口标题。
- Nhentai 详情封面使用 `t.nhentai.net` 缩略图：`i.nhentai.net` 的封面 CDN 在部分代理网络下不可达，而详情页封面宽度只有 240px，缩略图足够。
- 搜索返回 `{ total, page_size, result: [{ id, title, thumb, platform, extra }] }`，详情返回 `{ platform, title, thumb, extra }`；`platform` 是判别字段，`extra` 是各平台强类型扩展字段对象（camelCase），不要把公共字段塞进去。搜索与详情的 `extra` 在 Rust 侧分别是 `SearchExtra` / `DetailExtra` 枚举，前端在 `src/api/comics.ts` 是判别联合；卡片和详情页都不要退回 `Record<string, unknown>` 或 JSON 兜底。
- 当前 `page_size`：manhuagui 10、jm 80、nhentai 取 API 的 `per_page`（通常 25）。前端类型与调用封装在 `src/api/comics.ts`，首页已接入。
- JM 搜索可用，但 album 详情在部分网络环境会被 Cloudflare 返回 403；这不是解析错误，后续可能需要 Cookie/TLS 指纹或可用的代理出口。

## 编码约定

### TypeScript 与 Vue

- 使用 `<script setup lang="ts">` 与 Composition API。组件保持职责单一；可复用逻辑抽到 `src/composables/use*.ts`。
- TypeScript 开启了 strict、`noUnusedLocals` 与 `noUnusedParameters`。不要留下未使用的 import 或变量。
- 避免 `any`。未知输入用 `unknown` + 类型守卫（参考 `src/errors.ts`）。
- 仅类型导入使用 `import type`。保持现有 import 分组：Vue、Tauri、Naive UI、本地模块。
- 解构响应式 store 状态时使用 `storeToRefs()`。
- 依赖设置或语言的值用 `computed`，确保变化时重新计算。下拉选项与消息应在 `computed` 中构造，不要在模块顶层构造。
- 异步逻辑使用 `try` / `catch` / `finally`，并重置 loading 标记；错误通过 `useErrorMessage()` 呈现。
- 在 `onBeforeUnmount` / `onScopeDispose` 中清理监听器与定时器。
- 只有确实要忽略 Promise 时才使用 `void promise.catch(...)`，参考 `router/index.ts`。

### Rust

- 命令使用 `error.rs` 中的 `Result<T>` 别名，失败返回 `AppError` 变体。优先使用已有的 `From` 转换配合 `?`，不要手写映射。
- 命令参数保持简单、可序列化。返回数据，不要返回展示文案。
- 每个命令都必须在 `lib.rs` 的 `tauri::generate_handler![...]` 中注册；未注册的命令会在运行时失败。
- 错误序列化或任何新的 wire 结构都要补测试。测试写在代码旁的 `#[cfg(test)] mod tests` 中。
- 运行 `cargo fmt`，并满足带 `-D warnings` 的 Clippy。项目没有自定义 `rustfmt.toml` 或 `clippy.toml`，使用默认配置。

### 样式与 UI

- 全局 reset/基础样式放在 `src/styles/base.scss`；组件专属样式放在 `<style scoped>` 中。
- 优先使用 Naive UI 组件（`NButton`、`NInput`、`NCard` 等），不要重复造基础组件。自动导入已在 `vite.config.ts` 配置；不要手动修改 `src/components.d.ts`。
- 调用 `useMessage()` 的组件必须位于 `NMessageProvider` 之下，`App.vue` 已提供。若需要 dialog、notification 等 Provider，也在 `App.vue` 中添加。
- 优先使用现有 Naive UI 主题机制（`NConfigProvider`、`darkTheme`），不要零散地写 CSS 变量或内联颜色。

### 移动端安全区

- 所有路由共享 `App.vue` 中、`RouterView` 之外的 `.safe-area-top`。`base.scss` 将它实现为 `position: sticky; top: 0; height: env(safe-area-inset-top, 0px)` 的色块：它参与首屏布局，把内容推到安全区下方；滚动时保持吸顶并遮住其下内容，避免内容显示到状态栏或刘海区域。
- 安全区留白只能由这个全局元素负责。不要在页面或组件上写 `env(safe-area-inset-top)` 的 `margin`/`padding`，也不要用 `max-width`、`pointer: coarse` 模拟固定顶部边距；平台不提供安全区信息时，`env()` 自然为 0，按普通页面处理。
- `.safe-area-top` 的背景必须使用 `base.scss` 的 `--app-background`，与 `body` 的明暗主题背景保持一致；不要给它写死颜色。
- 页面自己的 `padding-top` 只负责视觉留白，不能当作安全区使用。新增路由只要放在现有 `RouterView` 内，就自动位于安全区下方。
- 如果新增 fixed/sticky 的顶部控件，必须放在安全区条下方或显式避开它，不能覆盖 `.safe-area-top`。
- 当前只处理顶部安全区；以后需要底部或左右安全区时，按同样方式增加独立占位元素，不要复用或扩大顶部条。

### 注释

本仓库的注释解释**为什么**，而不是复述代码在做什么。请沿用这种风格：在不易理解的决定旁写简短、完整的句子（例如 hydration 顺序、缓存失效、资产协议 scope、capability 原因）。不要给显而易见的代码加旁白。

## 修改位置速查

| 任务 | 主要文件 |
| --- | --- |
| 新增或修改 Rust 命令 | `src-tauri/src/lib.rs`；并在 `generate_handler!` 中注册 |
| 修改 ECH/DoH 与请求路径 | `src-tauri/src/ech/`、`src-tauri/src/image/http.rs`、`src-tauri/src/lib.rs`（命令注册）、`src-tauri/Cargo.toml` |
| 新增错误码 | `src-tauri/src/error.rs`、`src/errors.ts`、`useErrorMessage.ts`、两个语言包 |
| 新增或修改界面文案 | `src/i18n/messages/en.json` 与 `zh-CN.json`；通过 `useAppI18n()` 使用 |
| 新增路由或页面标题 | `src/router/index.ts`（`RouteTitleKey`、route meta）+ 语言包 |
| 新增持久化设置 | `src/stores/settings.ts`、`src/views/SettingsView.vue`、相关 composable |
| 修改窗口权限 | `src-tauri/capabilities/default.json` |
| 修改窗口/构建/打包配置 | `src-tauri/tauri.conf.json` |
| 新增 Tauri 插件 | `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`package.json`（若需要 JS API）、capabilities |
| 修改远程抓取的站点、请求头或缓存策略 | `src-tauri/src/image/`、`src-tauri/tauri.conf.json`（资产协议 scope） |
| 新增图片平台 | `src-tauri/src/image/platform/`（实现 trait、注册、host/header 与测试） |
| 新增/修改平台搜索或详情 | `src-tauri/src/image/platform/`、`src-tauri/src/image/model.rs`、`src/api/comics.ts` |
| 新增/修改详情页 | `src/views/ComicDetailView.vue`、`src/components/comic-detail/`、`src/router/index.ts`、`src-tauri/src/image/model.rs` |
| 修改首页或设置 UI | `src/views/HomeView.vue`、`src/views/SearchView.vue`、`src/views/SettingsView.vue`、`src/views/ShortcutPlaceholderView.vue`、`src/components/HomeShortcuts.vue`、`src/components/ShortcutIcon.vue`、`src/shortcuts.ts`、`src/stores/search.ts`、`src/stores/runtime.ts`、`src/composables/useRouteTransition.ts`、`src/components/ComicSearchBar.vue`、`src/components/ComicSearchResults.vue`、`src/components/ComicResultCard.vue`、`src/components/result-cards/`、`src/components/PlatformPicker.vue`、`src/components/ComicThumb.vue`、`src/assets/platforms/`、`src/api/comics.ts` |
| 修改全局样式或顶部安全区 | `src/styles/base.scss`、`src/App.vue`；组件样式保持在组件内 |
| 修改 Android 原生配置 | `src-tauri/gen/android/`（已跟踪，但大部分是生成内容，修改前要确认） |
| 重新生成应用图标 | `src-tauri/icons/`，通过 `pnpm tauri icon <source>` 生成；不要手改 |

## 常见任务配方

### 新增 Tauri 命令

1. 在 `src-tauri/src/lib.rs`（或新模块）中定义 `#[tauri::command] fn my_command(...) -> Result<T>`。
2. 在 `tauri::generate_handler![...]` 中注册。
3. 前端通过 `invoke<T>("my_command", { ... })` 调用。
4. 失败时返回 `AppError`。如果引入了新错误码，按上面的错误契约完整更新。
5. 运行 `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`，再运行 `pnpm build`。

### 新增 Tauri 插件或权限

1. 在 `src-tauri/Cargo.toml` 中添加 Rust crate，并在 `lib.rs` 中通过 `.plugin(...)` 初始化。
2. 如果前端需要调用插件 API，再用 `pnpm add` 添加 JS 包。
3. 只把功能所需的权限加入 `capabilities/default.json`。
4. 运行一次 `pnpm tauri dev`，让 Tauri 重新生成 capability schema 并校验配置。
5. 不要为了读取缓存图片而引入 `fs` 插件或大范围权限；应使用 Rust 命令 + 资产协议。

### 新增平台（图片/搜索/详情）

1. 在 `src-tauri/src/image/platform/` 下新增模块，定义 `id()`、`matches()` 与 `configure()`（Referer、Accept-Language 等）。
2. 需要检索/详情时再实现 `search_request`/`parse_search`/`detail_request`/`parse_detail`；HTML 用 `scraper`，JSON 用 `serde` 结构体。
3. 在 `platform/mod.rs` 的 `PLATFORMS` 中注册；在 `model.rs` 的 `SearchPlatform`/`SearchExtra` 中注册平台判别与 extra 类型；把 host 后缀、base URL 和 headers 放在该平台模块内。
4. 平台请求头或缓存语义变化时递增该平台的 `cache_version()`；id、公共字段名和 `extra` 结构保持稳定。若 URL query 只是签名/过期参数，实现 `cache_key_url` 在缓存键中去掉 query。
5. 补 parser fixture 测试：真实 HTML/JSON 片段、URL 编码、total/page_size、thumb 绝对化、`extra` 字段。
6. 前端同步更新 `src/api/comics.ts` 的 `ComicPlatform` 与各平台 `Extra`/`SearchItem` 类型，在 `PlatformPicker.vue` 的平台列表中注册新平台，在 `ComicResultCard.vue` 中增加分发分支，并在 `src/components/result-cards/` 新增该平台卡片；图标放到 `src/assets/platforms/`，优先使用平台站点资源；可按官方 favicon 的像素网格重绘 SVG，但不要凭空绘制。
7. 若平台支持详情，再实现 `DetailExtra` 与前端详情联合类型，在 `src/components/comic-detail/` 新增内容组件，并在 `ComicDetailView.vue` 中分发。

### 新增路由

1. 在 `src/views/` 下添加组件。
2. 添加路由并设置 `meta.titleKey`，同时扩展 `RouteTitleKey` 与 `translateTitleKey()`。
3. 在两个语言包中添加标题 key。
4. 验证原生窗口标题会变化，并确认 404 兜底仍然正常。
5. 新路由放在现有 `RouterView` 内即可继承顶部安全区，不要在新页面重复添加安全区 padding/margin。

## 验证

### 必须执行的检查

- **前端改动：** 运行 `pnpm build`。它会执行 `vue-tsc --noEmit` 与生产构建。
- **Rust 改动：** 在 `src-tauri/` 下运行 `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。
- **运行时相关改动**（IPC、图片缓存、平台检索、搜索 UI、主题、i18n、路由）：运行 `pnpm tauri dev`，在真实窗口中冒烟测试。只跑 `pnpm dev` 无法验证 Tauri API。
- **Android 改动：** 在模拟器或真机上运行 `pnpm tauri android dev`。

当前 Vite 构建会对主 bundle（>500 kB）给出 chunk 体积警告。这是预期行为，不是失败；除非任务本身就是优化包体积，否则不要追这个警告。

### 手动冒烟测试

- 搜索流程：从首页选择平台、输入关键词进入 `#/search`，确认结果数/卡片/缩略图/分页正常；切换平台、重新搜索、翻页都留在结果页；点击首页图标固定回首页，并与结果页搜索栏的反向动画同时完成；直接打开或刷新 `#/search` 回到首页；无结果与错误状态正常。
- 首页快捷入口：收藏、下载、历史进入占位页，设置进入设置页；窗口标题与正文标题正确，返回首页可用。
- 平台检索：分别用 manhuagui/nhentai 搜索关键词并取一条详情，确认 total/page_size/result 与 thumb/extras；JM 若被 Cloudflare 403，应返回本地化 httpStatus 错误而不是崩溃。
- 设置页：切换主题立即生效并在重启后恢复；切换语言后文案、Naive UI 与窗口标题同步更新。
- 设置页代理：启用并填写可达代理后搜索与缩略图正常；非法地址有本地化提示且不持久化；关闭开关后立即恢复默认网络行为（无需保存）；开启或修改地址后仍需保存并应用；重启后配置恢复。
- ECH：代理开启时打开 ECH，确认代理立即关闭且持久化；启用后搜索 nhentai 成功、缩略图正常；搜索 manhuagui/JM 走 fallback；关闭 ECH 后代理控件恢复可用；持久化为 `ech: true` + 代理开启后重启，确认启动阶段代理被自动关闭且 ECH 初始化成功；禁用后再启用，确认 runtime 重建且没有残留代理。
- ECH 握手探针：调用 `probe_ech_handshake`，传 `domain: "crypto.cloudflare.com"` 与 `dohEndpoint: "https://dns.alidns.com/dns-query"`，确认返回的 `configDomain` 为 `cloudflare-ech.com`、`echStatus` 为 `accepted`、TLS 版本为 `TLSv1.3`、`fallback` 为 `false`；同一份配置对 `www.cloudflare.com`、`example.com` 等 Cloudflare 站点也应生效；对 `www.qq.com` 等非 Cloudflare 站点应返回 `fallback: true`、`echStatus: "notOffered"`。
- 移动端安全区：在真机或 DevTools 中临时给 `.safe-area-top` 设置高度（如 32px），确认所有页面从安全区下方开始；滚动后安全区条仍固定在顶部并遮住其下内容；切换深色主题时背景色一致。
- 详情页：从结果卡片进入三个平台详情，确认标题/封面/信息/标签；直接刷新 `#/comic/...` 可加载；返回按钮回到原搜索结果；非法 id 显示本地化未找到；窄窗口切换单列。
- 访问 `#/does-not-exist`：显示 404 页面与本地化标题，返回首页链接可用。

### 测试现状

- Rust 测试覆盖错误契约（`src-tauri/src/error.rs`）、图片缓存、HTTP 基础头、平台识别/请求头与三平台搜索/详情 parser fixture（`src-tauri/src/image/`），以及 ECH 的 URL 解析、SVCB 提取、Cloudflare DoH 查询构造、ECH/普通 TLS 配置、resolver 端口映射与 60 秒缓存 TTL、reqwest ECH client（`src-tauri/src/ech/`、`src-tauri/src/image/http.rs`），共 70 个测试，其中 9 个联网测试默认 `ignored`。
- ECH 联网测试使用 `dns.alidns.com` 与 `doh.360.cn`：`queries_cloudflare_ech_over_doh` 验证 bootstrap；`performs_ech_handshake` 用同一份 `cloudflare-ech.com` 配置对 5 个 Cloudflare 站点握手；`queries_cloudflare_dns_over_ech` 验证 Cloudflare DoH over ECH 解析 `example.com`；`queries_nhentai_via_cloudflare_doh_over_ech` 验证 `nhentai.net` 直连（accepted、TLS1.3、HTTP 200、无本地代理）；`falls_back_to_plain_tls_for_non_cloudflare_site` 验证 `www.qq.com` 回退普通 TLS（`fallback=true`、`echStatus=notOffered`）；`reqwest_client_uses_ech_and_cloudflare_dns` 验证 reqwest ECH client（nhentai 200/HTTP2、qq ECH 失败分类、plain 回退）；`ech_client_fetches_and_falls_back` 验证 `HttpClient` runtime、TTL 懒刷新、代理互斥与禁用恢复；`enables_ech_with_the_default_bootstrap` 验证默认 AliDNS/360 bootstrap；`nhentai_search_detail_and_image_work_with_ech` 验证 nhentai 搜索、详情和详情缩略图的完整 ECH 链路。hickory DoH 查询测试要求 HTTP/2，经过只支持 HTTP/1.1 的代理时会超时。
- 前端没有单元/E2E 测试框架（没有 Vitest、Playwright，也没有 `pnpm test`），仓库也没有 CI。不要声称存在并未运行的测试覆盖。如果新增测试体系，请在 `package.json` 中加脚本并同步更新本文件。

## 陷阱与刻意保留的现状

- **`pnpm dev` 不是完整应用。** 它只提供 UI，没有 Tauri，因此 `invoke`、`convertFileSrc` 与窗口 API 都会失败。浏览器兜底逻辑已被有意移除。
- **ECH 已接入 HTTPS 请求。** `settings.ech` 经 `set_ech_enabled` 同步到 Rust；`HttpClient::send` 对 HTTPS 使用 ECH runtime，HTTP 保持普通 client。ECH client 使用 `cloudflare-ech.com` 的共享配置，目标 A/AAAA 通过 Cloudflare DoH over ECH（固定 IP `104.16.248.249` / `104.16.249.249`），非 Cloudflare 站点在 `is_connect()/is_timeout()` 时回退 plain TLS（会暴露目标 SNI）。代理与 ECH 互斥；`set_http_proxy(Some)` 在 ECH 开启时返回 `invalidInput`。配置 TTL 为 10 分钟，到期后的首个 HTTPS 请求会懒刷新；DoH A/AAAA 按 60 秒缓存，ECH 与 plain client 共享 resolver；刷新失败保留旧 runtime 并在 60 秒后重试。
- **启动 ECH 是异步的，首页会等待。** `main.ts` 不等待 `set_ech_enabled(true)` 就挂载，但 `HomeView` 在 `runtime.echStarting` 为 true 时只显示等待、不渲染搜索栏；初始化完成/失败后恢复。后端用 `ech_control_generation` 保证迟到的 bootstrap 不会覆盖用户之后的关闭。直接打开详情深链接仍可能在初始化完成前请求；要严格“ECH 开启即不泄漏”，还需给请求加门槛。
- **代理是 Rust 的运行时状态。** 不要把代理持久化到 Rust；启动和设置页保存时通过 `set_http_proxy` 同步，命令失败时保留旧 client。
- **顶部安全区不要用页面 padding。** 页面滚动时 padding 会跟着滚走；顶部安全区由 `App.vue` 的 sticky `.safe-area-top` 统一留白，滚动内容不会显示到状态栏或刘海区域。
- **主题与语言只能通过设置页修改。** 偏好保存在 `comics-reader:settings`；不要在组件里绕过 `useSettingsStore`，也不要重复调用 `useTheme()`/`useLocale()`。
- **`/search` 不携带查询参数。** 结果页依赖内存态 `search` store；刷新或直接打开 `#/search` 会被路由守卫送回首页，这是刻意取舍。首页图标固定回首页，不使用 `router.back()`。
- **详情页与搜索会话解耦。** `/comic/:platform/:id` 直接请求 `get_comic`，刷新/深链接可用；只有搜索会话仍存在时，返回按钮才回 `/search`，否则回首页。
- **首页与结果页之间使用 View Transition。** 搜索表单、搜索按钮和首页图标通过 `view-transition-name` 跨路由衔接；不要用延迟 `router.push` 的方式等待收起动画，否则会产生卡顿感。WebView 不支持该 API 时应直接导航。
- **缩略图不要直接使用远程 URL。** 搜索结果缩略图必须走 `ComicThumb` → `fetch_image` → 资产协议，否则防盗链站点会 403。
- **Rust 与前端构建是分开的。** Vite 不监听 `src-tauri/**`；修改 Rust 后请用 `pnpm tauri dev` / `pnpm tauri build`。`pnpm build` 永远不会编译 Rust。
- **开发端口固定。** `1420` 与 `build.devUrl` 必须保持一致；只改一处会破坏应用。
- **生成文件。** 不要手改 `src/components.d.ts`、`src-tauri/gen/schemas/`、`src-tauri/gen/android/**/build/`、`src-tauri/target/`、`dist/` 或 `src-tauri/icons/` 下的 PNG；应通过工具重新生成。
- **命令未注册会在运行时失败。** 只写函数不够，必须加入 `generate_handler!`。
- **capability 决定运行时权限。** 即使 Rust 插件已安装，缺少权限仍会报 "not allowed"。
- **资产协议需要三处同时成立。** `tauri` 的 `protocol-asset` feature、`tauri.conf.json` 的 `assetProtocol.enable`，以及覆盖 `$APPCACHE/images/**` 的 scope；缺一不可，失败通常表现为图片请求 403 或空白。
- **错误码必须两端同步。** 只改 Rust 会在 UI 中退化为 `"unknown"`；只改 TS 通常会触发穷举映射的类型错误。请同时更新两端与语言包。
- **翻译只在编译期做部分校验。** 缺少 `zh-CN` key 会编译报错，但多余 key 不会；请严格镜像英文结构。
- **持久化没有迁移层。** 重命名设置字段会让用户静默回到默认值；要保留旧 key 或添加迁移。
- **`bundle.active` 为 `false`。** `pnpm tauri build` 产出可执行文件，但不会生成安装包。需要打包时请显式开启。
- **CSP 已关闭（`csp: null`）。** webview 没有内容安全策略保护。不要在未处理该问题的前提下引入远程 HTML 或 `v-html` 渲染。
- **缓存键包含平台与 URL 的哈希。** 同一 URL 在不同平台下会分别缓存；manhuagui/JM 的图片缓存键会忽略 query，避免签名参数造成重复文件。修改 URL 归一化或缓存目录时提升 `CACHE_VERSION`，修改某平台请提升它的 `cache_version`。当前没有自动清理策略。
- **JM 域名经常轮换，详情页可能被 Cloudflare 拦截。** host 自动识别只是兜底；调用方应显式传 `platform: "jm"`，域名变化时只改 `src-tauri/src/image/platform/jm.rs` 中的常量。搜索可用但详情 403 时需要 Cookie/TLS 指纹或可用的代理出口，不要把它当成 parser 失败。
- **平台解析依赖上游结构。** manhuagui/JM 是 HTML 抓取，nhentai 是 v2 JSON；上游改版会破坏 parser。修改 selector 时同步补 fixture 测试，并保持 `extra` 字段名稳定。
- **`scripts/` 仅限本地。** 它被 gitignore，需要 Python 3.15+，不属于应用构建。不要把应用代码放进去。
- **README 已过时。** 它只是上游模板；如有文档需求，请更新 README 或本文件。

## Git 与提交规范

- 提交风格遵循现有历史：Conventional Commits，scope 可选。
  - `feat(reader): add chapter list command`
  - `fix(image): handle 403 from image host`
  - `refactor(i18n): derive locale options from catalogs`
  - `docs: update AGENTS.md`
- 使用祈使句、小写 subject。每次提交只做一个逻辑变更。
- 依赖变化时同时提交 `pnpm-lock.yaml` 与 `src-tauri/Cargo.lock`。
- 不要提交 `node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/schemas/`、`scripts/` 或未跟踪的编辑器文件（`.vscode/*` 已被忽略，但已提交的 `extensions.json` 除外）。
- 默认分支是 `master`。仓库没有 CI，上面的本地检查就是唯一门禁。
- 保持 diff 聚焦。项目没有 ESLint/Prettier 配置，不要格式化无关文件。

## 最终检查清单

交付改动前：

- [ ] `pnpm build` 通过（前端改动）。
- [ ] `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过（Rust 改动）。
- [ ] 新增界面文案同时存在于 `en.json` 与 `zh-CN.json`。
- [ ] 新增或修改的错误码已贯通 Rust、`errors.ts`、`useErrorMessage.ts` 与两个语言包。
- [ ] Tauri 命令已注册，所需 capability 已添加。
- [ ] 当改动涉及 IPC、图片缓存、平台检索、搜索 UI、主题、i18n 或路由时，已用 `pnpm tauri dev` 做运行时冒烟测试。
- [ ] 移动端安全区改动已验证：所有页面从安全区下方开始，滚动时内容不会显示在状态栏/刘海区域。
- [ ] 没有修改或暂存生成文件与构建产物。
- [ ] 如果命令、架构或契约发生变化，本文件已同步更新。
