# AGENTS.md

面向 AI 编码代理的仓库协作指南。修改代码前请先阅读本文件；当命令、架构或跨层契约发生变化时，请同步更新本文件。

## 项目概览

这是一个**早期阶段的 Tauri 2 漫画阅读桌面应用**，前端使用 **Vue 3 + TypeScript + Vite**，后端使用 **Rust**。

当前界面仍是脚手架，还不是完整的阅读器：

- 图片 URL 测试器由 Rust 命令抓取远程图片并缓存到应用缓存目录，前端通过资产协议读取缓存文件；请求附带 manhuagui 等站点防盗链所需的 `Referer`；
- 主题与语言设置保存在 webview 本地。

不要假设项目里已经有漫画库、章节浏览、下载器、数据库或阅读器引擎；这些都属于待实现的新功能。

`README.md` 仍是上游 Tauri/Vue 模板，不能可靠描述本项目。请以本文件、`package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 为准。

### 技术栈

- **前端：** Vue 3.5（`<script setup>`）、Vue Router 4（hash 路由）、Pinia 4 + `pinia-plugin-persistedstate`、vue-i18n 11、Naive UI 2（自动导入）、Sass、TypeScript 6。
- **后端：** Rust edition 2024、Tauri 2（启用 `protocol-asset`）、`reqwest`（rustls）负责抓取、`sha2` 生成缓存键、`thiserror` 2、Tauri 插件 `opener`。
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
│  ├─ main.ts                    启动：Pinia、i18n、router；挂载前应用主题/语言
│  ├─ App.vue                    Naive UI Provider 与 RouterView 外壳
│  ├─ views/
│  │  ├─ HomeView.vue            图片测试器、主题/语言设置
│  │  └─ NotFoundView.vue        404 页面
│  ├─ router/index.ts            Hash 路由、路由元数据、原生窗口标题
│  ├─ stores/
│  │  ├─ index.ts                Pinia + 持久化命名空间
│  │  └─ settings.ts             持久化的主题与语言偏好
│  ├─ composables/
│  │  ├─ useTheme.ts             系统/浅色/深色解析 + <html data-theme>
│  │  ├─ useLocale.ts            语言、Naive UI locale 对象、窗口标题同步
│  │  └─ useErrorMessage.ts      错误码 → 本地化消息
│  ├─ i18n/
│  │  ├─ index.ts                类型化 i18n 实例、语言检测、translate()
│  │  └─ messages/               en.json（主 schema）与 zh-CN.json
│  ├─ errors.ts                  前端错误契约与 AppError
│  ├─ styles/base.scss           全局 reset/基础样式
│  └─ components.d.ts            由 unplugin-vue-components 生成；不要手改
├─ src-tauri/
│  ├─ src/lib.rs                 Tauri Builder、命令注册、插件初始化
│  ├─ src/error.rs               AppError 序列化、错误码、测试
│  ├─ src/image_cache.rs         远程图片下载、校验与磁盘缓存
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
2. 同步读取 `useSettingsStore(pinia)`（持久化状态会从 `localStorage` 恢复），在挂载前应用已保存的主题并解析语言，确保首屏就是正确状态。
3. 应用挂载到 `#app`。
4. `App.vue` 通过 `NConfigProvider` 提供 Naive UI 的主题与语言，并用 `NMessageProvider` 包裹路由视图。`useTheme()` 与 `useLocale()` 负责保持运行时状态同步，且只在这个外壳中调用一次。

### 分层

- Vue UI 只通过 Tauri JS API 与系统交互：`@tauri-apps/api/core` 的 `invoke`、`convertFileSrc` 以及窗口 API。
- Rust 命令返回数据或类型化错误。**面向用户的文案只放在前端语言包中**，绝不放在 Rust 命令返回值里。
- 设置保存在 webview 的 `localStorage` 中，键名为 `comics-reader:<store-id>`；Rust 侧不持有这些设置。
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

### i18n 契约

- `src/i18n/messages/en.json` 是主 schema。`MessageSchema = typeof en`，`MessageKey` 由它派生，因此其他语言包缺少 key 会直接编译报错。
- 新代码请使用 `src/i18n` 导出的 `translate()` 或 `useAppI18n()`。不要用 `useI18n().t()` 传裸字符串，那会绕过编译期 key 校验。
- 每个新 key 必须在同一次改动中同时加入 `en.json` 与 `zh-CN.json`，并保持占位符（`{name}`、`{status}`）一致。
- 错误要以数据形式保存（code/details），在渲染时翻译，而不是提前拼成字符串。`HomeView` 的 `imageError` 是参考模式，这样切换语言时提示会同步更新。
- 新增语言时需要更新：`SUPPORTED_LOCALES`、`messages` 映射、`useLocale.ts` 中的 `NAIVE_LOCALES`、`HomeView.vue` 的语言选项，必要时还要改 `detectSystemLocale()`。

### 设置、主题与持久化

- `src/stores/settings.ts` 是唯一持久化的 store。插件在 `src/stores/index.ts` 中为键名加上 `comics-reader:` 前缀。
- 只持久化用户偏好或小型持久状态。不要持久化大对象、瞬态加载标记或密钥。
- 给 `SettingsState` 新增字段会自动持久化，老用户通常会回落到 store 的初始值。重命名或删除已持久化字段则需要迁移，否则旧数据会被静默忽略。
- `useTheme()` 与 `useLocale()` 只在 shell 层调用一次。其他组件直接读 `useSettingsStore()`；重复调用 `useTheme()` 会重复注册 media query 监听器。

### 远程 HTTP 与图片缓存

- 图片抓取全部在 Rust 中完成：`HomeView.vue` 调用 `fetch_image` 命令（`src-tauri/src/image_cache.rs`），由 `reqwest` 携带 `Referer` / `User-Agent` 请求远程图片，webview 不再直接访问图片站点。
- 命令校验响应（非 2xx、空响应、超过 32 MiB、非图片格式）后，用 URL 的 SHA-256 作为文件名写入 `app_cache_dir()/images/<hash>.<ext>`；同一 URL 再次请求会直接命中缓存，不再联网。
- 前端用 `convertFileSrc(cached.path)` 生成资产协议 URL 交给 `<img>`，从本地缓存目录读取图片。不要再改回 blob/object URL 或浏览器 `fetch`。
- 资产协议需要三处同时成立：`tauri` 的 `protocol-asset` feature、`tauri.conf.json` 的 `app.security.assetProtocol.enable`、覆盖 `$APPCACHE/images/**` 的 scope。缺任一项通常表现为图片 403 或空白。
- 图片格式优先按魔数判断，文件扩展名决定资产协议返回的 MIME；新增格式时要同时更新 `ImageFormat` 的魔数、扩展名与 MIME 映射。
- `CACHE_VERSION` 变化会让旧缓存自然失效；当前没有自动清理或容量上限，缓存会随浏览增长。

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

### 注释

本仓库的注释解释**为什么**，而不是复述代码在做什么。请沿用这种风格：在不易理解的决定旁写简短、完整的句子（例如 hydration 顺序、缓存失效、资产协议 scope、capability 原因）。不要给显而易见的代码加旁白。

## 修改位置速查

| 任务 | 主要文件 |
| --- | --- |
| 新增或修改 Rust 命令 | `src-tauri/src/lib.rs`；并在 `generate_handler!` 中注册 |
| 新增错误码 | `src-tauri/src/error.rs`、`src/errors.ts`、`useErrorMessage.ts`、两个语言包 |
| 新增或修改界面文案 | `src/i18n/messages/en.json` 与 `zh-CN.json`；通过 `useAppI18n()` 使用 |
| 新增路由或页面标题 | `src/router/index.ts`（`RouteTitleKey`、route meta）+ 语言包 |
| 新增持久化设置 | `src/stores/settings.ts`、相关 composable、`HomeView.vue` 的设置卡片 |
| 修改窗口权限 | `src-tauri/capabilities/default.json` |
| 修改窗口/构建/打包配置 | `src-tauri/tauri.conf.json` |
| 新增 Tauri 插件 | `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`package.json`（若需要 JS API）、capabilities |
| 修改远程抓取的站点、请求头或缓存策略 | `src-tauri/src/image_cache.rs`、`src-tauri/tauri.conf.json`（资产协议 scope） |
| 修改全局样式 | `src/styles/base.scss`；组件样式保持在组件内 |
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

### 新增路由

1. 在 `src/views/` 下添加组件。
2. 添加路由并设置 `meta.titleKey`，同时扩展 `RouteTitleKey` 与 `translateTitleKey()`。
3. 在两个语言包中添加标题 key。
4. 验证原生窗口标题会变化，并确认 404 兜底仍然正常。

## 验证

### 必须执行的检查

- **前端改动：** 运行 `pnpm build`。它会执行 `vue-tsc --noEmit` 与生产构建。
- **Rust 改动：** 在 `src-tauri/` 下运行 `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。
- **运行时相关改动**（IPC、图片缓存、主题、i18n、路由）：运行 `pnpm tauri dev`，在真实窗口中冒烟测试。只跑 `pnpm dev` 无法验证 Tauri API。
- **Android 改动：** 在模拟器或真机上运行 `pnpm tauri android dev`。

当前 Vite 构建会对主 bundle（>500 kB）给出 chunk 体积警告。这是预期行为，不是失败；除非任务本身就是优化包体积，否则不要追这个警告。

### 手动冒烟测试

- 图片测试器：输入有效图片 URL 能显示预览；再点一次相同 URL 应直接命中缓存。无效 URL 显示本地化错误。错误显示期间切换语言，确认提示同步更新。
- 主题：切换浅色/深色/跟随系统，确认立即生效，并在重启后保持。
- 语言：切换 English/简体中文，确认界面、`document.documentElement.lang`、Naive UI 文案与原生窗口标题都更新，且重启后保持。
- 访问 `#/does-not-exist`：显示 404 页面与本地化标题，返回首页链接可用。

### 测试现状

- Rust 测试覆盖错误契约（`src-tauri/src/error.rs`）与图片缓存的纯逻辑（`src-tauri/src/image_cache.rs`），共 9 个测试。
- 前端没有单元/E2E 测试框架（没有 Vitest、Playwright，也没有 `pnpm test`），仓库也没有 CI。不要声称存在并未运行的测试覆盖。如果新增测试体系，请在 `package.json` 中加脚本并同步更新本文件。

## 陷阱与刻意保留的现状

- **`pnpm dev` 不是完整应用。** 它只提供 UI，没有 Tauri，因此 `invoke`、`convertFileSrc` 与窗口 API 都会失败。浏览器兜底逻辑已被有意移除。
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
- **缓存键是 URL 的哈希。** 修改 URL 归一化、请求头或缓存目录时请提升 `CACHE_VERSION`，否则旧缓存会继续命中；当前没有自动清理策略。
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
- [ ] 当改动涉及 IPC、图片缓存、主题、i18n 或路由时，已用 `pnpm tauri dev` 做运行时冒烟测试。
- [ ] 没有修改或暂存生成文件与构建产物。
- [ ] 如果命令、架构或契约发生变化，本文件已同步更新。
