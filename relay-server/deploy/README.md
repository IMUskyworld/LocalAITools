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

## 健康检查

```bash
curl -fsS http://127.0.0.1:8080/health
curl --cacert /opt/localmind-relay/relay-server/certs/localmind-relay-ca.crt \
  https://39.107.53.230/health
```

## 后续替换为正式域名

购买并完成 ICP 后：

1. 将 DNS A 记录指向 `39.107.53.230`；
2. 将 `deploy/Caddyfile` 的 IP 替换为域名；
3. 删除 `tls internal`，让 Caddy 自动申请正式证书；
4. 更新客户端 Relay URL，并移除 internal CA pin。

不要在未加密的公网端口上运行 Relay。