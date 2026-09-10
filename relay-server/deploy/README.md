# Relay 部署

当前部署目标为阿里云北京 ECS `39.107.53.230`。由于域名和 ICP 尚未处理，部署使用 Caddy internal CA 在 443 上提供 TLS，客户端后续通过仓库中的 `certs/localmind-relay-ca.crt` 固定信任该 CA。

## 服务器依赖

- Docker Engine
- Docker Compose v2
- Caddy 2
- 至少 2GB Swap

## 首次部署

```bash
git clone https://github.com/IMUskyworld/LocalAITools.git /opt/localmind-relay
cd /opt/localmind-relay
sudo bash relay-server/deploy/install.sh
```

若服务器访问 GitHub 较慢，可改用 `ghproxy.net` 按文件下载，或使用部署机的 `scp` 上传 `relay-server/` 与 `shared-contract/`。

Docker 构建默认不执行 `apt-get`：官方 Rust slim 镜像已包含编译 `rusqlite bundled` 所需的 GCC 和 libc 头文件。Cargo 使用 `rsproxy.cn` 稀疏索引，避免国内服务器访问 `crates.io` 长时间卡住。

## 健康检查

```bash
curl -fsS http://127.0.0.1:8080/health
curl --cacert /opt/localmind-relay/relay-server/certs/localmind-relay-ca.crt \
  https://39.107.53.230/health
```

Windows 上使用系统 Schannel 的 `curl.exe` 做临时检查时，如果提示 revocation status is unknown，可加 `--ssl-no-revoke`。这个参数只用于本地诊断；LocalMind 客户端应使用 rustls 并显式加入仓库内的 internal CA，不依赖系统吊销检查。

## IP-only TLS 说明

`deploy/Caddyfile` 中的 `default_sni 39.107.53.230` 是必需的。部分客户端在直接连接 IP 时不会发送 TLS SNI；没有它时，Caddy 会在握手阶段返回 `internal error`。

## 后续替换为正式域名

购买并完成 ICP 后：

1. 将 DNS A 记录指向 `39.107.53.230`；
2. 将 `deploy/Caddyfile` 的 IP 替换为域名；
3. 删除 `tls internal` 和 `default_sni`，让 Caddy 自动申请正式证书；
4. 更新客户端 Relay URL，并移除 internal CA pin。

不要在未加密的公网端口上运行 Relay。
