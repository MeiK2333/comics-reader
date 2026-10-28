<div align="center">

# Comics Reader

**基于 Tauri 2 的跨平台漫画阅读器 —— 在七个站点搜索、阅读、收藏和下载。**

[English](README.md) · [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3.5-42B883?logo=vuedotjs&logoColor=white)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)

</div>

---

## 截图

<p align="center">
  <img src="docs/images/home.png" width="15%" alt="首页" />
  <img src="docs/images/platforms.png" width="15%" alt="站点选择器" />
  <img src="docs/images/search.png" width="15%" alt="搜索结果" />
  <img src="docs/images/detail.png" width="15%" alt="作品详情" />
  <img src="docs/images/settings.png" width="15%" alt="设置页" />
  <img src="docs/images/dark.png" width="15%" alt="深色主题" />
</p>

<sub>截图取自真实构建，窗口 1200×800。拍摄说明见 [`docs/images/README.md`](docs/images/README.md)。</sub>

## 功能

- **七个漫画来源** —— 在同一个界面里完成搜索、查看详情和阅读。
- **为长时间阅读打造** —— 三页轨道无缝滑动，支持从左到右、从右到左和上下三种阅读方向，双指或滚轮缩放、拖拽平移、键盘快捷键，并在首页/尾页自动切换章节。
- **阅读历史** —— 按作品和章节记录，随时从上次的位置继续。
- **带分组的收藏** —— 把收藏的作品整理进自定义分组，配有独立的分组栏。
- **下载与离线阅读** —— 整部作品下载到本地，断网也能继续看。
- **内置网络工具** —— 提供 HTTP 代理设置和加密客户端握手（ECH）支持，应对直连受限的网络环境。
- **日志查看器** —— 在应用内直接查看并导出运行日志。
- **图片缓存管理** —— 在设置里查看缓存占用并一键清理。
- **浅色与深色主题**，中英文界面，同时适配桌面端与 Android。

## 支持的平台

| 平台 | 搜索 | 详情 | 阅读 | 说明 |
| --- | :---: | :---: | :---: | --- |
| [JM](https://18comic.vip/) | ✅ | ✅ | ✅ | 走移动端 APP API，页图为分块乱序排列，由本地解码还原。 |
| [nhentai](https://nhentai.net/) | ✅ | ✅ | ✅ | 整个作品视为单个章节。 |
| [拷贝漫画](https://www.mangacopy.com/) | ✅ | ✅ | ✅ | 章节列表按站点分区展示；页图为 AES 加密。 |
| [manhuagui](https://www.manhuagui.com/) | ✅ | ✅ | ✅ | 章节列表按更新批次分组。 |
| [漫蛙](https://manwa.me/) | ✅ | ✅ | ✅ | 仅支持匿名内容；使用多个可互换的图源 CDN。 |
| [E-Hentai](https://e-hentai.org/) | ✅ | ✅ | ✅ | 仅支持匿名图库；不支持 ExHentai、不支持登录、不提供原图。 |
| [pixiv](https://www.pixiv.net/) | ✅ | ✅ | ✅ | 仅支持公开作品，R-18 和登录限定作品无法匿名访问。 |

### 开始之前请知悉

- **完全不使用账号。** 应用不会要求登录，也不会保存 Cookie。所有平台都以匿名方式访问，因此需要登录才能看到的内容自然无法获取。
- **部分站点需要代理。** 视网络环境而定，JM、E-Hentai、pixiv 和漫蛙可能无法直连访问。如果某个站点加载失败，请在设置里配置 HTTP 代理或开启 ECH。
- **应用本身不托管内容。** 它只跳转到第三方站点，并渲染其返回的内容。详见[法律声明与免责声明](#法律声明与免责声明)。

## 快速开始

### 环境要求

| 工具 | 版本 |
| --- | --- |
| [Node.js](https://nodejs.org/) | `^22.12.0` \|\| `^24.0.0` \|\| `>=26.0.0` |
| [pnpm](https://pnpm.io/) | 最新版 |
| [Rust](https://www.rust-lang.org/tools/install) | stable，1.85 及以上 |
| [Tauri 依赖](https://tauri.app/start/prerequisites/) | Windows 需 WebView2，macOS 需 Xcode，Linux 需 `webkit2gtk` |
| Android SDK / NDK / JDK | 仅在需要构建 Android 版本时安装 |

### 从源码运行

```bash
git clone https://github.com/MeiK2333/comics-reader.git
cd comics-reader
pnpm install

pnpm tauri dev
```

`pnpm tauri dev` 会启动完整应用，前端支持热更新。

> `pnpm dev` 和 `pnpm preview` **只启动前端**，端口 1420。此时没有 Tauri
> 能力（搜索、图片、下载、存储都不可用），需要真实功能时请使用
> `pnpm tauri dev`。

### 从源码构建

```bash
pnpm tauri build
```

> 打包步骤默认关闭（`src-tauri/tauri.conf.json` 中 `bundle.active` 为
> `false`），因此该命令只产出二进制文件而不生成安装包。需要可分发的安装包
> 时，请在配置文件中显式开启。

## 下载

预构建的安装包发布在
[GitHub Releases 页面](https://github.com/MeiK2333/comics-reader/releases)：

| 平台 | 产物 |
| --- | --- |
| Windows | `comics-reader_<版本>_x64-setup.exe`、`comics-reader_<版本>_x64_en-US.msi`、`comics-reader_<版本>_x64.exe` |
| Android | `comics-reader_<版本>_<abi>.apk`（`arm64-v8a`、`armeabi-v7a`、`x86_64`、`x86`） |

具体版本与平台相关说明请查看发布说明。系统需满足 Tauri 2 的运行条件 —— 在
Windows 上意味着已安装
[WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)。

## 架构

```
┌──────────────────────────────────────────┐
│  前端 —— Vue 3 + TypeScript + Vite        │
│  views / components / composables / api  │
└──────────────┬───────────────────────────┘
               │  Tauri IPC（invoke）
               │  命令返回 Result<T, AppError>
┌──────────────▼───────────────────────────┐
│  后端 —— Rust（Tauri 2）                   │
│  platforms/ · image/ · ech/ · downloads/ │
│  history.rs · favorites.rs · storage.rs  │
└──────────────┬───────────────────────────┘
               │  HTTPS（reqwest + rustls）
┌──────────────▼───────────────────────────┐
│  上游漫画站点                            │
└──────────────────────────────────────────┘
```

代码库遵循两条原则：

- **前端从不直接访问网络。** 所有 HTTP 请求、页面图片和缓存查询都通过 Rust
  命令完成。前端只调用 `invoke` 并渲染结果。
- **平台相关代码彼此隔离。** 每个平台在前后端各占一个目录，共享的图片管线
  通过一组很小的 trait（`ImageProfile`、`Catalog`）与之交互。共享代码里不会
  出现任何平台名。

## 项目结构

```
comics-reader/
├── src/                    # 前端（Vue 3 + TypeScript）
│   ├── api/                # 每个 IPC 命令的类型化封装
│   ├── components/         # 通用 UI，含阅读器覆盖层
│   ├── composables/        # 可复用行为（缩放、手势、预加载……）
│   ├── platforms/<id>/     # 平台前端：卡片、详情、类型、文案
│   ├── i18n/messages/      # en.json（母版）与 zh-CN.json
│   ├── router/             # 路由与导航栈
│   ├── stores/             # Pinia store（设置、搜索、运行时）
│   └── views/              # 每个界面一个组件
├── src-tauri/
│   ├── src/
│   │   ├── platforms/      # 平台 Rust 实现与 fixture
│   │   ├── image/          # 抓取、缓存键、缓存淘汰
│   │   ├── ech/            # ECH 引导与 TLS
│   │   ├── downloads/      # 下载队列、worker 与磁盘布局
│   │   ├── history.rs      # 阅读历史（JSONL）
│   │   ├── favorites.rs    # 收藏与分组
│   │   └── storage.rs      # 路径与持久化辅助
│   └── capabilities/       # 主窗口的 Tauri 权限
├── contracts/              # 跨层数据结构的固定 JSON 形状
├── e2e/                    # WebDriver 端到端测试
├── docs/images/            # 本 README 使用的截图
├── AGENTS.md               # 面向贡献者的完整工程约定
└── ROADMAP.md              # 计划支持的站点与功能
```

## 开发

前端命令在仓库根目录执行，Rust 命令在 `src-tauri/` 下执行。

| 命令 | 作用 |
| --- | --- |
| `pnpm dev` | 仅前端，端口 1420，无 Tauri 能力。 |
| `pnpm tauri dev` | 完整应用，支持热更新。**日常开发请用这个。** |
| `pnpm build` | 用 `vue-tsc` 做类型检查并构建前端，不编译 Rust。 |
| `pnpm test` | 跑一遍 Vitest 测试（jsdom）。 |
| `pnpm test:watch` | Vitest 监听模式。 |
| `pnpm test:e2e` | 构建 debug 包并运行 `e2e/` 下的 WebDriver 测试。仅限 Windows，需要 `tauri-driver` 和匹配的 Edge WebDriver。 |
| `pnpm tauri build` | 为当前平台构建应用。 |
| `cargo test` | 运行 Rust 测试。 |
| `cargo fmt --check` | 检查 Rust 格式。 |
| `cargo clippy --all-targets -- -D warnings` | 以警告视为错误来 lint Rust。 |

项目没有 CI，因此本地检查就是唯一门禁：前端改动跑 `pnpm test` 和
`pnpm build`，Rust 改动跑 `cargo test`、`cargo fmt --check` 和
`cargo clippy --all-targets -- -D warnings`。

### 新增一个平台

平台需要在前后端各加一处，并各自注册一行：

1. `src-tauri/src/platforms/<id>.rs` —— 实现 `ImageProfile`（必需）和
   `Catalog`（搜索 / 详情 / 阅读）。
2. `src-tauri/src/platforms/registry.rs` —— 注册。
3. `src/platforms/<id>/` —— `index.ts`、`types.ts`、`messages.ts`、
   `ResultCard.vue`、`Detail.vue`、图标以及 `fixtures/wire.json`。
4. `src/platforms/index.ts` —— 加入 `platformList`。

详细说明见前端侧的
[`src/platforms/README.md`](src/platforms/README.md) 和后端侧的
[`src-tauri/src/platforms/README.md`](src-tauri/src/platforms/README.md)。

## 参与贡献

欢迎贡献代码。请先阅读 [`AGENTS.md`](AGENTS.md)，其中记录了跨层契约、错误码
与 i18n 规则，以及一些容易被破坏的约束。

几条约定：

- 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)，
  使用祈使句、小写主题词，每次提交只包含一个逻辑变更。
- 默认分支为 `master`。
- 不要提交构建产物、生成文件或无关改动。

## 路线图

计划支持的站点：[包子漫画](https://www.baozimh.com/)、
[manhuabika](https://manhuabika.com/) 和
[热辣漫画](https://www.manga2026.com/)。

计划中的功能：漫画库、独立的章节列表页、按章勾选与 CBZ / 文件夹导出，以及用
数据库替代目前的扁平文件存储。

当前清单见 [`ROADMAP.md`](ROADMAP.md)。

## 法律声明与免责声明

Comics Reader 是一个客户端。它不托管、不镜像、不存储漫画内容 —— 只是向第三方
站点发起请求，并在你自己的窗口里展示这些站点返回的内容。你下载的图片会缓存
或保存在本地设备上，供离线阅读。

- 你需要自行遵守所访问站点的服务条款，以及适用于你所在辖区的版权法律。
- 应用不提供账号，也无法访问付费或登录限定内容，请勿尝试用它绕过这些限制。
- 本项目与任何一个受支持的漫画站点均无隶属、背书或赞助关系。站点名称与图标
  归各自所有者所有，仅用于标识来源。
- 软件按「现状」提供，不附带任何形式的担保。维护者不对使用它造成的任何损失
  承担责任。

## 许可证

基于 [MIT 许可证](LICENSE) 发布。版权所有 © 2026 MeiK2333。
