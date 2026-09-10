# ADR-002：采用 Relay-first 跨网络传输方案

- 状态：已接受
- 日期：2026-09-10
- 决策范围：LocalMind（Windows）与 LocalFile（Android）跨网络互联
- 替代关系：替代路线图中“官方 Tailscale 客户端优先”的 Phase 2 默认方案

## 1. 背景

产品硬要求是：

> 任何人下载安装软件后，不安装 Tailscale、不配置路由器端口映射、不购买额外服务器，也能直接使用双端互联。

这意味着默认方案必须满足：

1. Windows 端不暴露公网监听端口；
2. Android 和 Windows 都能只通过出站连接工作；
3. Wi-Fi A 与 Wi-Fi B 位于不同网络、存在 NAT/CGNAT 时仍能通信；
4. 普通用户不需要理解 VPN、Tailnet、端口映射或动态 DNS；
5. 中继服务器只负责受控转发，不能成为任意命令执行器。

## 2. 同类项目调研

调研日期：2026-09-10。使用 GitHub 公开仓库检索“WebSocket relay / pairing / remote relay / self-hosted relay”等关键词，并检查代表性项目的活跃度与职责。

| 项目 | 做得好的地方 | 与本项目的关系 | 不足或不适配 |
|---|---|---|---|
| [Tailscale](https://github.com/tailscale/tailscale) | WireGuard、成熟身份体系、NAT 穿透和稳定性很强 | 可作为高级用户/开发调试通道 | 默认要求用户安装客户端并登录；不满足“下载即用” |
| [Headscale](https://github.com/juanfont/headscale) | 可自托管 Tailscale 控制面 | 说明自建控制面可行 | 仍需 Tailscale 兼容客户端与节点概念，用户路径过重 |
| [ntfy](https://github.com/binwiederhier/ntfy) | 自托管 push/消息中继、部署简单、活跃 | 可借鉴公网出站长连接和轻量运维 | 面向通知，不是语义化设备配对和命令状态机 |
| [Gotify](https://github.com/gotify/server) | WebSocket 实时消息、自托管、活跃 | 可借鉴实时消息分发和鉴权 | 没有本地设备确认、文件权限和远程任务状态 |
| [Syncthing](https://github.com/syncthing/syncthing) | P2P、设备配对、文件同步成熟 | 可借鉴设备身份与配对思路 | 定位是持续文件同步，不是手机控制 Windows Agent |
| [PairDrop](https://github.com/schlagmichdoch/PairDrop) | 浏览器内配对、无需注册、体验好 | 可借鉴一次性配对码和低摩擦交互 | 面向临时文件传输，不提供持久设备控制 |
| [LocalSend](https://github.com/localsend/localsend) | 局域网发现和多平台文件传输体验好 | 可借鉴本地发现和无中心化体验 | 主要依赖同网段，不适合跨 Wi-Fi 的默认远程控制 |

结论：

- Tailscale/Headscale 是优秀的高级通道或开发通道，但不能作为普通用户默认依赖；
- ntfy/Gotify 证明了轻量公网中继可行，但都不解决本项目的设备配对、语义动作、Windows 本地确认与审计；
- 本项目应自建最小 Relay，并复用成熟思路：设备密钥、一次性配对码、幂等命令、离线队列、出站 WSS、服务端不执行命令。

## 3. 方案比较

### A. 用户安装 Tailscale

优点：安全性和 NAT 穿透成熟，工程量最低。

否决原因：违反“下载安装即用”的硬要求，普通用户仍需注册、登录、安装第二个软件。

### B. 应用内嵌 tsnet / WireGuard

优点：用户无感，长期可做到接近 P2P。

否决原因：Windows 需要 Go sidecar，Android 需要 AAR/前台服务/电池与生命周期适配；当前 2 核 2GB Relay 阶段不值得引入该复杂度。

### C. Windows 暴露公网端口 + 手机直连

优点：链路短。

否决原因：家庭宽带 NAT、CGNAT、防火墙和端口映射会让大量用户无法使用，且公网裸服务风险高。

### D. 自建语义 Relay + 双端出站 WSS

优点：

- 两端都只出站，不需要端口映射；
- 可以跨 Wi-Fi、跨 NAT 使用；
- 服务端只转发版本化 Envelope，不执行 Shell；
- 可做离线队列、幂等、状态恢复和审计元数据；
- 后续可平滑增加端到端加密与可选 P2P。

代价：

- 需要域名/TLS/服务器运维；
- 增加一条云端依赖；
- 中继必须严格控制权限，不能保存明文文件内容。

决定采用 D。

## 4. 决策

Phase 2 默认传输路线改为：

```text
Android LocalFile
        │  WSS（仅出站）
        ▼
LocalMind Relay（公网）
        ▲  WSS（仅出站）
        │
Windows LocalMind Remote Service
```

Tailscale 降级为：

- 开发调试通道；
- 高级用户可选 P2P/VPN 通道；
- 未来 P2P 失败时的备用路径。

第一版 Relay 只处理：

- 设备注册与设备凭据；
- 一次性短期配对码；
- Windows/Android 配对关系；
- WebSocket 在线状态与心跳；
- 版本化 Envelope 转发；
- `command_id` 幂等；
- 离线队列；
- 状态回传；
- 最小元数据审计。

Relay 明确禁止：

- 执行命令；
- 保存 DeepSeek Key；
- 保存明文文件内容；
- 绕过 Windows 本地确认；
- 接受 `shell.run`、`powershell.run`、`cmd.run`、`file.delete` 等动作。

## 5. 安全边界

1. TLS 是强制项，不能以公网明文 HTTP 长期提供 Relay。
2. 每台设备使用独立随机凭据；服务端只存 token 哈希。
3. 配对码为一次性、短期有效，不能作为长期凭证。
4. 未配对设备之间不能转发消息。
5. `from_device_id` 必须等于当前认证设备，不能被客户端伪造。
6. 重复 `command_id` 不得重复执行，只能返回原状态。
7. Relay 只按白名单动作类型转发，未知动作拒绝。
8. 文件内容和 AI 对话内容不应由 Relay 持久化；Phase 2 后期升级为客户端端到端加密的 opaque payload。
9. Windows 本机确认、路径边界和审计仍是最终授权边界，Relay 不能代替它们。

## 6. 当前未决项

1. 域名与 ICP：大陆服务器长期公开服务需要合规处理；开发/演示阶段可使用临时 TLS 方案。
2. 端到端加密：第一版 Relay 可先完成认证转发和状态机，但涉及文件内容前必须补齐 E2EE。
3. P2P：在 Relay MVP 稳定后再评估 QUIC/WebRTC/UDP 打洞，不作为首版前置。
4. 多设备：首版只保证一个 Android 与一个 Windows 的配对关系，多设备作为后续扩展。

## 7. 后果

- 用户不再需要安装 Tailscale 才能跨网络使用；
- 服务器成为可用性关键点，必须提供健康检查、重启恢复和备份；
- 协议、设备身份、配对、幂等和审计必须先于 UI 功能开发；
- Relay 的代码和部署将与 LocalMind、LocalFile 分开演进，避免三端同时大规模重构。