# ADR-004：无域名阶段用 Caddy internal CA + 客户端内置 CA 固定信任

- 状态：已接受
- 日期：2026-09-11
- 决策范围：LocalMind Relay（服务端 TLS）、LocalMind（Windows）、LocalFile（Android）
- 依赖决策：ADR-002 Relay-first、ADR-003 游客/账号模式

## 1. 背景

Relay 部署在阿里云北京 ECS 的公网 IP `39.107.53.230` 上，项目尚未购买域名、未完成 ICP 备案。
大陆节点上未备案域名无法正常通过 80/443 对外提供服务，而公开 CA 也不会给纯 IP 签发证书。

可选方案：

| 方案 | 结果 |
|---|---|
| A. 明文 HTTP | 令牌、设备凭据在公网明文传输，且与安全设计冲突，否决 |
| B. 把 internal CA 装进操作系统/Android 系统信任库 | 需要管理员权限、污染用户系统，安装即用体验差，否决 |
| C. 客户端内置该 CA，用自己的通道发请求 | 采纳 |
| D. 购买域名 + ICP + 正式证书 | 目标状态，现阶段做不到，作为后续替换路径 |

## 2. 决策

1. 服务端继续用 Caddy `tls internal`，证书来自 Caddy 本地 CA，随仓库分发 `relay-server/certs/localmind-relay-ca.crt`。
2. 客户端不再依赖操作系统信任链，而是显式信任这一份 CA：
   - **Windows**：Relay HTTP 请求全部由 Rust 侧 `relay_http_request` 发起（reqwest + rustls + `add_root_certificate`），CA 以 `include_bytes!` 编译进可执行文件；前端 WebView2 不再直接 `fetch` Relay。
   - **Android**：`RelayTls` 用 OkHttp 组合信任管理器，在系统信任之外加入打包进 APK 的 `res/raw/localmind_relay_ca.crt`。
3. 两端都不提供“跳过证书校验”的开关。

## 3. 后果

- 任何人下载安装后即可使用账号/协同能力，无需手动导入证书或改系统设置。
- 证书一旦轮换，必须同步更新服务端 `certs/`、Windows 内置副本与 Android raw 资源，并重新打包。
- 内置 CA 属于项目私有信任锚，不能用于其他用途；客户端仍需保持 `https`，不允许回落明文。

## 4. 后续替换为正式证书的收尾清单

1. 域名完成 ICP，DNS A 记录指向该 ECS；
2. `relay-server/deploy/Caddyfile` 去掉 `tls internal` 与 `default_sni`；
3. 删除两端内置 CA 与 `add_root_certificate` / 组合信任逻辑，回到系统信任链；
4. 更新默认 Relay 地址与文档。
