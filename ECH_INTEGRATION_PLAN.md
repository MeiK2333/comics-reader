# ECH 接入实施计划

> 状态：进行中（ECH-0 ~ ECH-6 已完成，ECH-7 手动冒烟待完成）
> 基线：`ec61310 feat(ech): add DoH ECH bootstrap and rustls handshake probe`  
> 更新时间：2026-10-25  
> 适用项目：`comics-reader`（Tauri 2 + Vue 3 + Rust）
>
> 本文既是当前 ECH 探针的现状说明，也是后续把 ECH 接入实际图片/搜索请求的实施计划。后续可能由多个智能体协作完成，请先阅读第 9 节的协作约定。

---

## 1. 背景与当前状态

### 1.1 已完成

当前仓库已经有独立的 ECH 探针，但**还没有接入任何业务请求**。

已有模块：

```text
src-tauri/src/ech/
├─ mod.rs          命令导出
├─ bootstrap.rs    AliDNS/360 只查询 cloudflare-ech.com 的 ECH
├─ doh.rs          Cloudflare DoH over ECH，固定 IP 解析目标 A/AAAA
├─ session.rs      EchSession：一次 bootstrap，复用 ECH 配置
└─ tls.rs          rustls ECH 握手 + 普通 TLS fallback
```

已有命令：

- `fetch_ech_config(dohEndpoint)`：只用于检查 bootstrap 结果。
- `probe_ech_handshake(domain, dohEndpoint)`：完整探针，返回 `EchHandshake`。

当前探针行为：

1. AliDNS/360 只查询 `cloudflare-ech.com` 的 HTTPS RR，得到共享 ECHConfigList。
2. 用该配置构造 rustls `ClientConfig`（aws-lc-rs HPKE）。
3. 目标 A/AAAA 通过 Cloudflare DoH over ECH 解析，固定连接：
   - `104.16.248.249`
   - `104.16.249.249`
4. 目标 TCP/TLS 直连，ECH 失败后回退普通 TLS。
5. 不读取应用的 HTTP 代理。

当前验证结果（2026-10-25）：

```text
cargo test --lib                                              52 passed; 5 ignored
cargo test --lib -- --ignored performs_ech_handshake          1 passed
cargo test --lib -- --ignored queries_nhentai_via_cloudflare_doh_over_ech  1 passed
cargo test --lib -- --ignored falls_back_to_plain_tls_for_non_cloudflare_site  1 passed
cargo test --lib -- --ignored queries_cloudflare_dns_over_ech 1 passed
cargo test --lib -- --ignored queries_cloudflare_ech_over_doh 1 passed
cargo fmt --check                                             passed
cargo clippy --all-targets -- -D warnings                     passed
cargo build                                                   passed
```

实测：

- `cloudflare-ech.com`、`cloudflare-dns.com`、`crypto.cloudflare.com`、`www.cloudflare.com`、`example.com`：ECH accepted，TLS1.3。
- `nhentai.net`：ECH accepted、TLS1.3、HTTP 200，无本地代理。
- `www.qq.com`：ECH 失败，fallback 普通 TLS、HTTP 200。

### 1.2 尚未完成

- 还没有在真实 Tauri 窗口中做 ECH UI 冒烟（设置页开关、启动恢复、代理控件）；nhentai 搜索/详情/图片链路由 ignored live test 覆盖。
- 启动时的 ECH 初始化是异步的，首个请求可能在 runtime 就绪前走普通 client；迟到的启动 bootstrap 会通过控制代际丢弃，不会覆盖用户关闭。

---

## 2. 目标与非目标

### 2.1 目标

ECH 开启后：

1. 所有 HTTPS 业务请求使用带 ECH 的 reqwest client。
2. 目标 A/AAAA 只通过 Cloudflare DoH over ECH 解析，不使用系统 DNS，也不使用 AliDNS/360。
3. ECH 被 Cloudflare 站点接受时正常返回数据。
4. 非 Cloudflare 站点 ECH 失败时自动回退普通 TLS，不中断业务。
5. ECH 开启时禁用 HTTP 代理；代理与 ECH 互斥。
6. 前端设置页和启动流程能初始化 ECH，并在失败时给出可理解的反馈。
7. 至少验证 nhentai 的搜索、详情和图片完整链路。

### 2.2 非目标

- 不实现 ODoH、ECH 服务端、ECH 证书/密钥管理。
- 不把 HTTP 流量代理到 Cloudflare。
- 不为 HTTP（非 HTTPS）请求启用 ECH。
- 不在首版做多平台灰度开关；要么全局 ECH，要么关闭。
- 不在首版解决 Android 上的 aws-lc-rs/ECH 兼容性（需要单独验证）。

---

## 3. 目标架构

```text
前端 ECH 开关
  │
  ├─ startup / toggle
  ▼
Rust set_ech_enabled(true, bootstrapDoh)
  │
  ├─ AliDNS/360 DoH：仅查询 cloudflare-ech.com 的 ECHConfigList
  ├─ 构建 rustls ECH ClientConfig（aws-lc-rs HPKE）
  ├─ 构建 Cloudflare DoH over ECH resolver
  │     ├─ 固定 IP 104.16.248.249 / 104.16.249.249
  │     └─ 目标 A/AAAA 只从这里解析
  │
  ├─ ECH reqwest client
  │     ├─ preconfigured ECH TLS
  │     ├─ custom DNS resolver = Cloudflare DoH
  │     └─ no_proxy
  │
  └─ plain TLS fallback client
        ├─ ordinary rustls TLS
        ├─ custom DNS resolver = Cloudflare DoH
        └─ no_proxy

业务请求（fetch_image / search_comics / get_comic）
  └─ HttpClient::send
        ├─ ECH 开启：先 ech_client
        │     └─ connect/TLS 失败 → plain_client 重试一次
        └─ ECH 关闭：normal_client（可走现有代理）
```

### 关键不变量

- AliDNS/360 只出现在 `bootstrap.rs`，且只查询 `cloudflare-ech.com`。
- 目标域名永不进入系统 DNS 或 AliDNS/360（ECH 开启时）。
- Cloudflare DoH 自身使用固定 IP，不再依赖任何 DNS。
- ECH 开启时 `no_proxy()`；代理与 ECH 不能同时生效。
- 普通 TLS fallback 只改变 TLS，不改变目标 DNS 来源。
- 图片缓存键、资产协议、平台 headers 保持不变。

---

## 4. 关键设计决策

### 4.1 reqwest 集成方式

优先方案：

- `reqwest::ClientBuilder::use_preconfigured_tls(ech_tls)` 注入 ECH。
- `reqwest::ClientBuilder::dns_resolver(Arc<CloudflareResolver>)` 替换目标 DNS。
- `.no_proxy()`。

需要先做 Phase 0 spike，验证：

1. reqwest 是否接受 rustls ECH `ClientConfig`。
2. preconfigured TLS 的 ALPN 行为是否符合 reqwest/hyper 预期。
3. 自定义 `reqwest::dns::Resolve` 与 ECH 是否兼容。
4. 是否支持 HTTP/2。

如果 reqwest 无法可靠承载 ECH，备选方案：

- 使用 hyper + 自定义 connector（TLS + DNS）。
- 或仅对 nhentai 等 Cloudflare 平台使用独立的 ECH 请求通道，其他请求保持 reqwest 普通路径。

### 4.2 ALPN

- 探针的 `ech/tls.rs` 给 ECH config 设置 `http/1.1` ALPN，适合手动 `TlsStream` 探针。
- **Phase 0 实测结论（reqwest 0.12.28）**：`use_preconfigured_tls` 走 `TlsBackend::BuiltRustls`，reqwest 原样使用传入的 `ClientConfig`，不会覆盖 `alpn_protocols`。因此“空 ALPN 让 reqwest 协商”不成立：空列表只会走 HTTP/1.1。
- 给 reqwest 用的 ECH TLS config 必须显式设置 `["h2", "http/1.1"]`；探针继续用 `["http/1.1"]`。
- 因此 `EchSession` 需要提供两套 TLS config：
  - probe config（`http/1.1`）
  - reqwest config（`h2` + `http/1.1`）

Phase 0 已确认：显式 ALPN 下 reqwest 与 ECH 配置可以正常协商 HTTP/2，不会报错或强制降级。

### 4.3 回退语义

- 业务请求都是 GET，可安全重试。
- `HttpClient::send` 先走 ECH client。
- 仅在连接/TLS 类错误时用 plain client 重试一次。
- HTTP 状态错误、body 错误、decode 错误不重试。
- 建议判断：`error.is_connect() || error.is_timeout()`；如果 reqwest 将 TLS 错误归为其他类别，需要用 Phase 0 的实际错误枚举修正。
- 回退会暴露目标 SNI，这是非 Cloudflare 站点的预期代价。

### 4.4 生命周期

- ECH 开关打开时初始化一次 `EchRuntime`，不在每个请求 bootstrap。
- `EchRuntime` 至少包含：
  - `Arc<EchSession>`
  - `ech_client`
  - `plain_client`
  - `created_at` / `expires_at`
- 关闭 ECH 时销毁 runtime，恢复普通 client。
- ECH 初始化失败：保持 ECH 关闭，返回可展示错误；前端回滚开关。
- 是否需要后台定时刷新见 4.5。

### 4.5 缓存与刷新

首版建议：

- ECH 配置按固定 TTL（10 分钟）或 HTTPS RR TTL 刷新。
- Cloudflare DoH 的 A/AAAA 结果做带 TTL 的内存缓存。
- 刷新失败保留旧 runtime；连续失败再关闭 ECH。
- 不在请求热路径上重复 bootstrap。

实现（2026-10-25）：TTL 固定 10 分钟；到期后的首个 HTTPS 请求懒刷新，`tokio::sync::Mutex` 串行化；刷新成功原子替换 runtime，失败保留旧 runtime 并在 60 秒后重试；DoH A/AAAA 缓存 60 秒，ECH/plain client 共享同一个 resolver。

后续可加：

- 后台 `tokio::spawn` 定时刷新并 `RwLock` 切换 runtime。
- 失败重试/指数退避。
- 状态命令 `get_ech_status` 供设置页展示。

### 4.6 代理与 ECH

- ECH 开启 → 代理关闭。
- 前端现有行为：打开 ECH 时清除代理；启动时若 ECH 开启且代理开启，先关闭代理。
- 后端必须再做一次约束：
  - `set_ech_enabled(true)` 时清除代理。
  - `set_http_proxy(Some(...))` 在 ECH 开启时返回 `invalidInput`。
- ECH 的两个 reqwest client 都 `.no_proxy()`。

### 4.7 错误契约

- bootstrap 失败、Cloudflare DoH 失败、TLS 失败复用现有 `network` / `unexpected` / `invalidInput`。
- 如果设置页需要专门的“ECH 初始化失败”提示，新增错误码必须同时更新五处：
  1. `src-tauri/src/error.rs`
  2. `src/errors.ts`
  3. `src/composables/useErrorMessage.ts`
  4. `src/i18n/messages/en.json`
  5. `src/i18n/messages/zh-CN.json`
- 首版也可以先用 `network`，避免扩大契约。

---

## 5. 建议新增/修改的接口

### 5.1 `EchSession`

```rust
impl EchSession {
    pub(crate) async fn bootstrap(bootstrap_doh: &str) -> Result<Self>;

    pub(crate) async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>>;

    pub(crate) async fn handshake(&self, domain: &str) -> Result<EchHandshake>;

    /// 给 reqwest 用的 ECH TLS config，不预设 ALPN。
    pub(crate) fn reqwest_ech_tls(&self) -> Result<rustls::ClientConfig>;

    /// 给 fallback reqwest 用的普通 TLS config。
    pub(crate) fn reqwest_plain_tls(&self) -> Result<rustls::ClientConfig>;

    /// 目标 DNS resolver。
    pub(crate) fn resolver(&self) -> Arc<CloudflareResolver>;
}
```

### 5.2 `CloudflareResolver`

```rust
pub(crate) struct CloudflareResolver {
    session: Arc<EchSession>,
}

impl reqwest::dns::Resolve for CloudflareResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        // session.resolve(name.as_str()) -> Vec<SocketAddr>
    }
}
```

- 返回端口 `0`，让 reqwest 按 URL scheme 补 80/443。
- 必须 `Send + Sync`。

### 5.3 `HttpClient`

```rust
struct ClientState {
    proxy: Option<String>,
    normal_client: Option<reqwest::Client>,
    ech: Option<EchRuntime>,
}

struct EchRuntime {
    session: Arc<EchSession>,
    ech_client: reqwest::Client,
    plain_client: reqwest::Client,
    expires_at: SystemTime,
}

impl HttpClient {
    pub(super) fn get(&self) -> Result<reqwest::Client>; // 普通 client
    pub(super) async fn send<F>(&self, url: &Url, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder;
}
```

### 5.4 Tauri 命令

```rust
#[tauri::command]
async fn set_ech_enabled(
    http: State<'_, HttpClient>,
    enabled: bool,
    bootstrap_doh: Option<String>,
) -> Result<()>;

#[tauri::command]
fn get_ech_status(http: State<'_, HttpClient>) -> Result<EchStatus>;
```

`EchStatus` 可包含：

```json
{
  "enabled": true,
  "ready": true,
  "configDomain": "cloudflare-ech.com",
  "bootstrapDoh": "https://dns.alidns.com/dns-query",
  "expiresAt": 1234567890
}
```

### 5.5 前端

新增 `src/api/ech.ts`：

```ts
export async function setEchEnabled(
  enabled: boolean,
  bootstrapDoh?: string,
): Promise<void>;

export async function getEchStatus(): Promise<EchStatus>;
```

修改：

- `src/main.ts`：启动时根据持久化 ECH 设置初始化。
- `src/views/SettingsView.vue`：ECH 开关改为异步，失败回滚。
- 如新增错误码，同步 `en.json` / `zh-CN.json`。

---

## 6. 任务拆分

> 任务 ID 用于多智能体协作。每个任务只改自己列出的文件；跨文件任务必须串行。

| ID | 任务 | 状态 | 依赖 | 可并行组 | 主要文件 |
| --- | --- | --- | --- | --- | --- |
| ECH-0 | reqwest + ECH + 自定义 resolver spike | 已完成 | 无 | 先做，阻塞后续 | 临时测试/`ech/` |
| ECH-1 | `EchSession` 暴露 reqwest TLS/resolver | 已完成 | ECH-0 | Rust 组 | `ech/session.rs`、`ech/doh.rs`、`ech/tls.rs` |
| ECH-2 | `HttpClient` ECH runtime、双 client、`send` 回退 | 已完成 | ECH-1 | Rust 组 | `image/http.rs` |
| ECH-3 | 图片/检索/详情接入 `HttpClient::send` | 已完成 | ECH-2 | Rust 组 | `image/mod.rs`、`image/http.rs` |
| ECH-4 | `set_ech_enabled` / 代理互斥 / 初始化失败 | 已完成 | ECH-2 | Rust 组 | `lib.rs`、`image/`、`ech/` |
| ECH-5 | 前端启动与设置页接入 | 已完成 | ECH-4 接口确定 | 前端组 | `src/api/ech.ts`、`main.ts`、`SettingsView.vue`、i18n |
| ECH-6 | TTL 缓存、刷新、状态与错误 UX | 已完成 | ECH-3/4/5 | 后续 | `ech/`、`image/http.rs`、前端 |
| ECH-7 | 测试、文档、运行时冒烟 | 进行中 | 全部 | 收尾 | 测试、`AGENTS.md` |

> 状态由 Coordinator 维护：`待开始` → `进行中` → `已完成` / `阻塞`。执行 agent 只在交接消息中报告，不并发编辑本表。

### ECH-0：reqwest 集成 spike（阻塞项）

步骤：

1. 在 `ech/tls.rs` 增加一个不预设 ALPN 的 ECH config 构造函数（可先临时）。
2. 构造一个临时 `CloudflareResolver`，调用 `EchSession::resolve`。
3. 用 reqwest 请求 `https://nhentai.net/`：
   - `use_preconfigured_tls(ech_tls)`
   - `dns_resolver(resolver)`
   - `no_proxy()`
4. 验证：
   - 返回 HTTP 200。
   - 不发生 ALPN panic。
   - 能协商 h2 或 http/1.1。
5. 请求 `https://www.qq.com/`，验证 ECH 失败时的错误类型。
6. 记录结论到本文件 “Phase 0 结论” 小节。

验收：

- 一个临时 ignored 测试或 scratch 示例成功。
- 明确 reqwest 方案是否可行；若不可行，记录备选方案。

### ECH-1：Session 与 Resolver

步骤：

1. 新增 `reqwest_ech_tls()`（空 ALPN）。
2. 新增 `reqwest_plain_tls()`。
3. 实现 `CloudflareResolver`，基于 `EchSession::resolve`。
4. 单元测试 resolver 的端口处理（0/443）。
5. 确保 `EchSession` 可 `Arc` 共享，`Send + Sync`。

验收：

- `cargo test --lib` 通过。
- 新增 resolver 测试。

### ECH-2：HttpClient 双 client 与回退

步骤：

1. 扩展 `ClientState` / `EchRuntime`。
2. 新增 `HttpClient::send(build)`：
   - ECH 未启用 → normal client。
   - ECH 启用 → ech client。
   - 连接/TLS 失败 → plain client 重试一次。
3. 新增错误分类函数 `is_fallback_error(&reqwest::Error) -> bool`，先按 `is_connect() || is_timeout()` 实现。
4. 单元测试错误分类。
5. 注意 `Mutex` 不跨 await 持有。

验收：

- 编译、clippy 通过。
- 不改变现有普通请求行为。

### ECH-3：业务请求接入

步骤：

1. `fetch_image` 使用 `HttpClient::send` 构建 GET。
2. `download_image` / `fetch_document` 改为接受 `&HttpClient` 或回调。
3. 保持平台 headers、referer、缓存键、错误映射不变。
4. 确保 HTTP 请求（非 HTTPS）不走 ECH client。

验收：

- `cargo test --lib` 通过。
- 现有平台测试不回归。
- ECH 关闭时行为与改造前一致。

### ECH-4：后端命令与生命周期

步骤：

1. 新增 `set_ech_enabled`、`get_ech_status`。
2. 初始化时：
   - 校验代理冲突。
   - bootstrap（默认 AliDNS，失败尝试 360）。
   - 构建 `EchRuntime`。
3. 关闭时清理 runtime，恢复 normal client。
4. 初始化失败返回错误，不污染现有 state。
5. `set_http_proxy(Some)` 在 ECH 开启时返回 `invalidInput`。
6. 注册到 `lib.rs` 的 `generate_handler!`。

验收：

- 命令可调用；失败可回滚。
- 代理与 ECH 不能同时生效。
- `cargo test` / `clippy` 通过。

### ECH-5：前端接入

步骤：

1. 新增 `src/api/ech.ts`。
2. `main.ts` 启动时：
   - 如果 `settings.ech` 为 true，先关闭代理。
   - 异步调用 `set_ech_enabled(true)`。
   - 失败时记录/提示，不阻塞挂载。
3. `SettingsView.vue` ECH 开关：
   - 开启前关闭代理。
   - 调用后端；成功写回 store，失败回滚并显示错误。
4. 如新增错误码，更新五处错误契约和两个语言包。
5. 注意 `useAppI18n`/`translate` 约定，不硬编码中文。

验收：

- 设置页切换 ECH 不阻塞 UI。
- 失败有本地化提示。
- 重启后能恢复/重试。

### ECH-6：缓存、刷新与状态

步骤：

1. `EchRuntime` 增加 TTL。
2. Cloudflare DoH A/AAAA 缓存（按 TTL）。
3. 刷新策略：定时/懒刷新，失败保留旧 runtime。
4. 可选 `get_ech_status` 展示。
5. 处理 ECH 配置轮换导致的握手失败。

验收：

- 长时间运行不重复 bootstrap。
- 配置过期后能刷新。
- 刷新失败不导致进程崩溃。

### ECH-7：验证与文档

步骤：

1. 新增/更新 ignored 联网测试。
2. 运行 `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo build`。
3. 运行 `pnpm build`（若前端改动）。
4. `pnpm tauri dev` 冒烟：
   - 开启 ECH，搜索 nhentai，打开详情，加载图片。
   - 搜索 manhuagui/JM，确认 fallback。
   - 关闭 ECH，确认普通路径。
5. 更新 `AGENTS.md` 的 ECH 段落、测试数量、命令说明。
6. 更新本计划状态。

验收：

- 全链路通过，工作区无生成文件/无关改动。
- 提交为独立 commit。

---

## 7. 测试计划

### 7.1 单元

- `CloudflareResolver` 端口映射、cache key 归一化与 60 秒 TTL（`maps_resolved_addresses_to_port_zero`、`normalizes_dns_cache_keys`、`cache_entries_expire_after_the_ttl`）。
- `is_fallback_error` 分类（由 `ech_client_fetches_and_falls_back` 的 qq fallback 覆盖）。
- ECH client / plain client 选择与共享 resolver（由 live runtime 测试覆盖）。
- 代理与 ECH 互斥（`ech_client_fetches_and_falls_back`）。
- 配置刷新与 generation guard（由同一 live 测试强制到期触发）。

### 7.2 联网 `#[ignore]`

- `set_ech_enabled(true)` 成功。
- ECH client 请求 `https://nhentai.net/` 返回 200。
- nhentai 搜索/详情/图片完整链路。
- 非 Cloudflare 站点 fallback 成功。
- 关闭 ECH 后普通请求可用。

### 7.3 手动冒烟

`pnpm tauri dev`：

1. 设置页开启 ECH，确认代理关闭。
2. 搜索 nhentai，打开详情，确认封面/缩略图。
3. 搜索 manhuagui/JM，确认回退可用。
4. 关闭 ECH，确认恢复普通网络/代理设置。
5. 重启，确认持久化 ECH 初始化。

### 7.4 必跑命令

```bash
cd src-tauri
cargo test --lib
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build
```

前端有改动时：

```bash
pnpm build
```

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| reqwest 不接受 ECH preconfigured TLS / ALPN | 阻塞主方案 | ECH-0 spike；备选 hyper + 自定义 connector 或独立 ECH 通道 |
| TLS/ECH 错误分类不可靠 | fallback 不触发 | Phase 0 记录实际错误；扩大安全重试条件；必要时按 host 选择 client |
| ECH 配置轮换 | 请求失败 | TTL 刷新/失败重建 |
| ECH 初始化失败导致离线 | 用户无网络 | 回滚 ECH 开关，恢复普通 client/代理；提示 |
| 非 Cloudflare fallback 泄漏 SNI | 隐私下降 | 明确文档；不把它宣传为完整 ECH |
| Cloudflare DoH 不可达 | 目标 DNS 失败 | 多固定 IP；状态提示；不盲目回退系统 DNS（避免泄漏） |
| Android aws-lc-rs/ECH | 移动端失败 | 先桌面验证；必要时平台开关 |
| 启动延迟 | 首屏慢 | 异步初始化；初始化期间普通 client 或等待 |

---

## 9. 多智能体协作约定

### 9.1 角色

建议：

- **Coordinator**：维护本计划、分配任务、合并/提交、更新状态。
- **Rust Network Agent**：ECH-0 ~ ECH-4、ECH-6。
- **Frontend Agent**：ECH-5。
- **Verification Agent**：ECH-7、跨层检查。

### 9.2 文件所有权

避免并发修改同一文件：

- Rust 网络：`src-tauri/src/ech/**`、`src-tauri/src/image/http.rs`、`src-tauri/src/image/mod.rs`、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`。
- 前端：`src/api/ech.ts`、`src/main.ts`、`src/views/SettingsView.vue`、`src/i18n/messages/*.json`、`src/errors.ts`、`src/composables/useErrorMessage.ts`。
- 文档/测试：`AGENTS.md`、本文件、测试代码。
- 若任务必须跨所有权，先由 Coordinator 串行安排。

### 9.3 执行协议

1. 每个 agent 开始前阅读 `AGENTS.md` 和本文件。
2. 只做分配的任务 ID，不顺手重构无关代码。
3. 开始前确认依赖任务已完成。
4. 完成后：
   - 运行任务指定的检查。
   - 向 Coordinator 报告：任务 ID、改动文件、测试结果、阻塞项。
   - 不直接并发编辑本文件；由 Coordinator 更新状态。
5. 提交使用 Conventional Commits，例如：
   - `feat(ech): resolve request DNS through Cloudflare DoH`
   - `feat(ech): add ECH-aware HTTP client fallback`
   - `feat(settings): initialize ech from persisted settings`
6. 不提交生成文件、`target/`、`dist/`、`scripts/`、临时测试。

### 9.4 交接模板

```text
Task: ECH-2
Status: done
Files: src-tauri/src/image/http.rs
Tests: cargo test --lib; cargo clippy --all-targets -- -D warnings
Notes: fallback classification uses is_connect()/is_timeout(); HTTP status not retried.
Blockers: none
Commit: <hash>
```

---

## 10. 待确认决策

在开始 ECH-5 前需要确认：

1. ECH 是否全局应用到所有 HTTPS 请求，还是只对 nhentai 等 Cloudflare 平台启用？
2. ECH 初始化失败时：
   - A. 回滚开关并提示；
   - B. 保持开关但内部普通网络；
   - C. 应用报错。
   建议 A。
3. ECH 配置刷新周期：固定 10 分钟，还是读取 HTTPS RR TTL？建议先固定 10 分钟。
4. 是否首版就做 `get_ech_status` UI，还是仅失败提示？建议先做失败提示。
5. Android 是否在本次范围内？建议先桌面，Android 单独立项。

---

## 11. 当前 Phase 0 结论

> 由执行 ECH-0 的 agent 填写，2026-10-25。

- reqwest 版本：0.12.28；rustls 0.23.46。
- ECH preconfigured TLS 是否可用：可用。`use_preconfigured_tls(rustls::ClientConfig)` 走 `TlsBackend::BuiltRustls`，同一 rustls 版本下 downcast 成功。
- ALPN 行为：reqwest 不覆盖 preconfigured config 的 `alpn_protocols`。显式设置 `["h2", "http/1.1"]` 后，`https://nhentai.net/` 协商到 HTTP/2.0 并返回 200；空 ALPN 的对照请求协商为 HTTP/1.1（nhentai 返回 403，测试只断言版本）。
- 自定义 resolver 是否可用：可用。`reqwest::dns::Resolve` + `Arc` + 端口 `0` 的 `SocketAddr` 工作正常，目标 A/AAAA 全部走 `EchSession::resolve` 的 Cloudflare DoH。
- nhentai 请求结果：`https://nhentai.net/` 返回 200、HTTP/2.0；现有 `queries_nhentai_via_cloudflare_doh_over_ech` 测试用同一份 `cloudflare-ech.com` 配置返回 `echStatus=accepted`、TLS1.3、`fallback=false`。
- 非 Cloudflare 错误类型：`https://www.qq.com/` 的 ECH 请求失败，`is_connect()=true`、`is_timeout()=false`、`is_request()=true`；source 链是 hyper-util connect error → rustls `InvalidCertificate(NotValidForName)`（期望 `cloudflare-ech.com`，服务端出示 qq CDN 证书）。用普通 TLS + 同一 Cloudflare DoH resolver 重试返回 200（HTTP/1.1）。
- 结论/备选方案：主方案可行，不需要 hyper 自定义 connector。fallback 判断用 `is_connect() || is_timeout()`；不要把 `is_request()` 纳入，否则普通 HTTP 错误也会触发重试。注意 `is_connect()` 也会命中 `kind: Request` 的证书错误（reqwest 会遍历 source），这是 fallback 需要的行为。
- 复现命令：`cargo test --lib -- --ignored reqwest_client_uses_ech_and_cloudflare_dns --nocapture`（约 10–15s 通过，含 4 次联网请求）。

### 11.1 ECH-1 ~ ECH-6 实现结论（2026-10-25）

- ECH-1：`EchSession` 新增 `reqwest_ech_tls()`（h2 + http/1.1）、`reqwest_plain_tls()`、`resolver()`；`CloudflareResolver` 返回端口 `0` 的 `SocketAddr`；`EchSession` 与 resolver 有 `Send + Sync` 测试。
- ECH-2：`HttpClient` 增加 `normal_client` 与 `EchRuntime`；`send(url, build)` 对 HTTPS 先走 ECH client，`is_connect() || is_timeout()` 时用 plain client 重试一次；HTTP URL 始终走普通 client。
- ECH-3：`download_image` / `fetch_document` 改为接收 `&HttpClient` 并调用 `send`；`fetch_image` / `search_comics` / `get_comic` 全部接入。
- ECH-4：新增 `set_ech_enabled` / `get_ech_status`；默认 bootstrap 先 AliDNS、失败再 360；`enable_ech` 清空代理；`set_http_proxy(Some)` 在 ECH 开启时返回 `invalidInput`；命令已注册到 `lib.rs`。
- ECH-5：新增 `src/api/ech.ts`；`main.ts` 启动时异步初始化并失败回滚 `settings.ech`；设置页 ECH 开关改为异步、失败回滚并显示本地化提示；ECH 开启时禁用代理控件；新增 `settings.ech.errors.update`。
- ECH-6：ECH 配置 TTL 10 分钟，到期后的首个 HTTPS 请求懒刷新；`tokio::sync::Mutex` 串行化，刷新成功原子替换 runtime，失败保留旧 runtime 并在 60 秒后重试；`CloudflareResolver` 增加 60 秒 A/AAAA 缓存（上限 256 条），ECH/plain client 共享同一个 resolver；设置页通过 `get_ech_status` 显示后端实际状态；新增 `ech_control_generation`，启动或命令的 bootstrap 晚于用户关闭时会被丢弃。
- 验证：`cargo test --lib` 61 passed / 9 ignored；`ech_client_fetches_and_falls_back` 通过（nhentai 200、qq fallback、TTL 强制刷新、代理互斥）；`enables_ech_with_the_default_bootstrap` 通过；`nhentai_search_detail_and_image_work_with_ech` 通过（搜索/详情/缩略图）；`pnpm build` 通过。
- 未完成：ECH-7（真实 Tauri 窗口 UI 冒烟、设置页开关与启动恢复的手动验证）。

---

## 12. Definition of Done

- [x] ECH 开启时，nhentai 的搜索、详情、图片全部成功（`nhentai_search_detail_and_image_work_with_ech`）；ECH 探针仍为 accepted（`probe_ech_handshake`）。
- [x] 非 Cloudflare 平台请求能 fallback 普通 TLS，不出现不可恢复错误。
- [ ] ECH 关闭时，原有普通请求/代理行为不变（待真实窗口冒烟）。
- [x] ECH 配置 10 分钟 TTL 懒刷新与 60 秒 DoH A/AAAA 缓存已实现并有测试。
- [x] 代理与 ECH 不能同时生效。
- [ ] ECH 配置失败有可理解提示，且可回滚（待真实窗口冒烟）。
- [x] `cargo test --lib`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo build` 通过。
- [x] 前端改动时 `pnpm build` 通过。
- [ ] `pnpm tauri dev` 冒烟通过。
- [x] `AGENTS.md` 已更新，测试数量准确。
- [x] 无生成文件或无关改动。

---

## 13. 快速命令

```bash
# Rust 检查
cd src-tauri
cargo test --lib
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build

# ECH 联网测试
cargo test --lib -- --ignored performs_ech_handshake
cargo test --lib -- --ignored queries_nhentai_via_cloudflare_doh_over_ech
cargo test --lib -- --ignored falls_back_to_plain_tls_for_non_cloudflare_site
cargo test --lib -- --ignored queries_cloudflare_dns_over_ech
cargo test --lib -- --ignored queries_cloudflare_ech_over_doh
cargo test --lib -- --ignored reqwest_client_uses_ech_and_cloudflare_dns
cargo test --lib -- --ignored ech_client_fetches_and_falls_back
cargo test --lib -- --ignored enables_ech_with_the_default_bootstrap
cargo test --lib -- --ignored nhentai_search_detail_and_image_work_with_ech

# 前端
pnpm build
pnpm tauri dev
```
