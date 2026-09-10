# LocalMind Relay

LocalMind / LocalFile 的跨网络语义中继服务。目标是让用户不需要安装 Tailscale、不需要配置路由器端口映射，也能让已配对手机安全地向 Windows 发送受控任务。

## 职责

- 设备注册和独立 token
- 一次性短期配对码
- 设备关系校验
- WSS Envelope 转发
- `command_id` 幂等
- 离线消息队列和重连补发
- 命令状态更新
- 最小服务日志

## 明确不做

- 不执行 Shell、PowerShell、CMD 或任意程序
- 不保存 DeepSeek Key
- 不保存明文文件内容
- 不绕过 Windows 本机确认、Tool Policy 和审计
- 不转发 `shell.run`、`file.delete` 等危险动作

## HTTP / WSS API

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/health` | 健康检查 |
| `POST` | `/v1/devices/register` | 首次注册设备，返回 `device_id` 和一次性展示的 `device_token` |
| `POST` | `/v1/pairing-codes` | 已认证设备签发 6 位一次性配对码 |
| `POST` | `/v1/pairings/claim` | 已认证设备使用配对码建立关系 |
| `GET` | `/v1/pairings` | 查询当前设备的配对 |
| `DELETE` | `/v1/pairings/{tenant_id}` | 解除配对 |
| `GET` | `/ws` | WebSocket 升级；需要 `X-Device-Id` 和 `Authorization: Bearer <token>` |

## 规划中的账号层

当前服务端实现的是设备注册、设备 token、一次性配对码和 WSS 转发。游客模式继续使用这条路径即可。

账号模式的设计见 `开发计划/ADR-003-account-and-guest-mode.md`。后续将在不破坏现有设备 API 的前提下增加：

- `/v1/auth/register`
- `/v1/auth/login`
- `/v1/auth/refresh`
- `/v1/auth/logout`
- `/v1/auth/me`
- `/v1/account/devices`
- 账号级设备授权和撤销接口

账号只解决“用户是谁”和“设备属于谁”；同一账号下的手机不能自动控制电脑，必须由 Windows 本机确认建立控制配对。

## 本地运行

```bash
cargo test --manifest-path relay-server/Cargo.toml
RELAY_BIND=127.0.0.1:8080 RELAY_DB_PATH=data/relay.db cargo run --manifest-path relay-server/Cargo.toml
curl http://127.0.0.1:8080/health
```

## Docker

```bash
docker compose -f relay-server/docker-compose.yml up -d --build
```

## 安全说明

第一版已经完成设备 token、配对关系、TLS 部署入口、动作白名单、幂等和离线队列。文件内容真正通过 Relay 之前，客户端仍需增加端到端加密，使 Relay 只看到 opaque payload。当前服务不是“任意命令远程执行器”。
