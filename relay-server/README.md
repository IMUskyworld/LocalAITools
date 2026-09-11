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

## 账号层

游客模式继续使用设备注册、设备 token、一次性配对码和 WSS 转发，无需账号。

账号模式在设备身份之上增加用户身份、设备归属和控制授权。账号只解决“用户是谁”和“设备属于谁”；同一账号下的手机不能自动控制电脑，必须由 Windows 本机确认建立控制配对。

| 方法 | 路径 | 说明 |
|---|---|---|
| `POST` | `/v1/auth/register` | 邮箱 + 密码注册，返回 access / refresh token |
| `POST` | `/v1/auth/login` | 登录；可选绑定当前设备 |
| `POST` | `/v1/auth/refresh` | 旋转 refresh token，签发新 token 对 |
| `POST` | `/v1/auth/logout` | 撤销当前 refresh token |
| `GET` | `/v1/auth/me` | 查询当前账号 |
| `GET` | `/v1/account/devices` | 查询账号下的设备列表 |
| `POST` | `/v1/account/devices/claim` | 将设备登记到账号（需同时带账号 token 和设备 token） |
| `DELETE` | `/v1/account/devices/{device_id}` | 从账号移除设备，并撤销其控制配对 |
| `GET` | `/v1/control/pairing-requests` | 查询与当前设备相关的控制授权请求 |
| `POST` | `/v1/control/pairing-requests` | Android 设备发起控制授权请求 |
| `POST` | `/v1/control/pairing-requests/{id}/approve` | Windows 目标设备本机确认 |
| `POST` | `/v1/control/pairing-requests/{id}/reject` | Windows 目标设备拒绝 |
| `GET` | `/v1/control/pairings` | 查询当前设备有效的控制配对 |
| `DELETE` | `/v1/control/pairings/{tenant_id}` | 撤销控制配对 |

实现约束：

- 密码使用 Argon2id 加盐哈希，服务端不保存明文密码。
- access token 短期有效（默认 30 分钟），refresh token 可旋转（默认 30 天），服务端只保存哈希。
- 设备 token 与账号 token 分离。
- 控制授权请求默认 10 分钟过期，同一对设备同时最多一个 `pending` 请求。
- 只有目标 Windows 设备可以批准或拒绝请求；批准后写入 `device_pairings` 并带权限列表。
- 已批准的控制配对会限制发送方向（只有 controller 能发命令）和动作白名单。
- 解绑设备会级联撤销其控制配对和当前设备的登录会话。

环境变量：

| 变量 | 默认值 | 含义 |
|---|---|---|
| `RELAY_ACCESS_TOKEN_TTL_SECONDS` | `1800` | access token 有效期 |
| `RELAY_REFRESH_TOKEN_TTL_SECONDS` | `2592000` | refresh token 有效期 |
| `RELAY_PAIRING_REQUEST_TTL_SECONDS` | `600` | 控制授权请求有效期 |

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
