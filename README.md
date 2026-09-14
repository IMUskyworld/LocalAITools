# LocalMind × LocalFile 双端 AI 智能工具

> 一台电脑 + 一部手机：Windows 上是**能真正动手干活**的 AI Agent，Android 上是可以**远程指挥它**的遥控器。

| 端 | 定位 | 技术栈 |
| --- | --- | --- |
| **LocalMind**（Windows） | 本地 AI Agent：多步工具调用、文档生成、被远程调度 | Tauri 2 + React + Rust + Python (Pydantic AI) |
| **LocalFile**（Android） | 文件 AI 助手 + 远程控制端 | Kotlin + Jetpack Compose |
| **LocalMind Relay** | 双端跨网络通道（自建，可选） | Rust + Axum + SQLite |

许可证：[MIT](LICENSE) · 当前产物：[LocalMindSetup.exe](#-发布产物) / [LocalFile.apk](#-发布产物)

## ✨ 界面预览

| 浅色主题 | 深色主题 |
| --- | --- |
| ![LocalMind 浅色](docs/pic/localmind-light.png) | ![LocalMind 深色](docs/pic/localmind-dark.png) |

---

## ✨ 功能特性

### Windows 端（LocalMind）

- **9 个 Agent 工具**：`write_file` / `read_file` / `list_dir` / `move_file` / `open_app` / `read_clipboard` / `create_doc` / `run_command` / `delete_path`
- **高危操作必须人工确认**：`run_command`（L4）、`delete_path`（L3）执行前弹出确认卡片，展示完整参数；60 秒未确认按拒绝处理
- **审计日志**：每次工具调用记录工具名、参数、结果与风险等级
- **路径白名单（Tool Guard）**：默认只允许 桌面 / 文档 / 下载（含 OneDrive 同名目录）；拒绝 `..`、UNC 路径、device namespace、`C:\Windows`、`Program Files`
- **文档生成**：PPT / Word / Excel / PDF，内置生成器，无需安装 Office 或 Python
- **流式 + 思考轨迹**：SSE 逐字输出，界面实时显示「规划 → 执行 → 完成」
- **在线 / 离线双模**：在线用 DeepSeek（`deepseek-flash`），离线用 Ollama 本地模型
- **长期记忆（记忆文档）**：每轮对话结束自动沉淀跨会话的稳定事实到 `%APPDATA%\LocalMind\memory.md`，每次对话前注入上下文；文件是可编辑的 markdown，超长自动整理、覆盖前自动留 `.bak`

### Android 端（LocalFile）

- **文件 AI 助手**：对话式生成 / 修改 Word、PPT、Excel、PDF
- **手写 Agent 循环**：规划轮 → 工具调用 → 反思轮，工具失败会自动分析原因
- **远程控制端**：登录账号后向电脑发起控制授权申请、下发指令并查看执行结果

### 双端远程控制

```
手机(LocalFile) ──申请远控──▶ 账号 + 设备配对
                                   │
        电脑(LocalMind) ◀──本机点「批准」── 建立控制配对(tenant_id)
                                   │
手机 ──指令──▶ Relay ──转发──▶ 电脑 Agent 执行 ──结果──▶ Relay ──▶ 手机
```

- 同账号只解决「设备归属与发现」，**不等于可以互控**：必须由电脑本机确认后才建立控制配对
- 指令按 `command_id` 幂等去重，支持离线队列与断线重连

### 隐私与凭据

- **不内置任何 API Key**：用户在设置页自行填写
  - Windows：`%APPDATA%\LocalMind\auth.json`，值为 AES-256-GCM 密文（`ENC:` 前缀，密钥由设备 ID 派生）
  - Android：EncryptedSharedPreferences（Android Keystore 保护）
- Relay **不保存 key、不保存明文文件内容、不执行任何命令**，只转发版本化 Envelope
- 卸载 LocalMind 会一并清理本地数据库与配置

---

## 🧩 项目组成

| 组件 | 平台 | 技术栈 | 职责 |
| --- | --- | --- | --- |
| **LocalMind** | Windows | Tauri 2 + React + Rust + Python (Pydantic AI) | 桌面 Agent：工具循环、文档生成、被远程调度 |
| **LocalFile** | Android | Kotlin + Jetpack Compose + OkHttp | 文件处理、文档生成、远程控制端 |
| **LocalMind Relay** | 公网服务 | Rust + Axum + SQLite | 账号、设备登记、控制配对、WSS 转发、幂等、离线队列 |

---

## 🤖 Agent Harness

### LocalMind（Pydantic AI）

Agent 循环下沉到 Python 子进程（`localmind/scripts/agent_server.py`），用 Pydantic AI 2.x 的 `run_stream_events()` 实现多步工具调用：

```
[React 前端] --SSE--> http://127.0.0.1:<port>/agent/stream
     ▲                              │
     │ get_agent_config (IPC)       ▼
[Rust 后端] ---- spawn/kill ---> [Python Agent 服务]
                                  9 个工具 + Tool Guard + 确认机制
```

- **结构化工具错误**：工具失败以结构化结果回灌模型，自动修正后重试
- **预算与超时**：单轮请求数 / 工具调用数上限、单工具超时、输出截断
- **Turn 级 Trace**：记录每轮上下文构建、模型请求、工具调用与耗时
- **交互式确认**：高危工具通过 SSE `confirm` 事件 ↔ 前端确认卡片双向通信

同一模型下的 A/B 评测（baseline vs Harness v2，12 任务 × 2 轮）：`18/24` → `24/24`，详见 [Phase1-Harness评测报告](开发计划/Phase1-Harness评测报告.md)。

### LocalFile（手写循环）

规划 → 工具调用 → 反思 三轮结构，可生成四种文档格式。

---

## 🗂️ 目录结构

```text
LocalAITools/
├── localmind/          # Windows 桌面端
│   ├── src/            # React 前端（TS/TSX）：聊天、账号、远控状态、设置
│   ├── src-tauri/      # Rust 后端：IPC、Agent 进程管理、SQLite、Relay WSS
│   └── scripts/        # Python：agent_server.py、tool_registry/tool_policy、文档生成器
├── localfile/          # Android 端
│   └── app/src/main/java/com/localmind/localfile/
│       ├── ui/         # Compose 界面（聊天 / 远控 / 账号 / 设置）
│       ├── chat/       # 对话引擎与手写 Agent 循环
│       ├── files/      # docx / pptx / xlsx / pdf 生成器
│       ├── common/      # 网络、TLS、Relay 客户端
│       └── storage/     # DataStore + 加密存储
├── relay-server/       # 自建中继（Rust + Axum + SQLite）
├── shared-contract/    # 双端共享契约定义
├── docs/               # 界面截图等资源
└── 开发计划/            # 路线图、ADR、设计文档、评测报告
```

---

## 🚀 快速开始

### 前置要求

| 用途 | 需要 |
| --- | --- |
| Windows 端 | Node.js 20+、Rust、Python 3.11（开发）、Ollama（离线模式，可选） |
| Android 端 | JDK 17、Android SDK |
| Relay（可选） | Docker（部署）、Rust（本地跑测试） |

### 1. Windows 端（LocalMind）

```powershell
# 生产构建：先生成 exe，再生成 NSIS 安装包
.\build-localmind.ps1
.\build-installer.ps1
```

脚本支持用环境变量覆盖本机工具链路径：`LIBCLANG_PATH`、`LOCALMIND_CARGO_BIN`、`LOCALMIND_CMAKE_BIN`、`LOCALMIND_MSVC_BIN`。

开发模式（改 Python 代码即时生效）：

```powershell
cd localmind
npm install
npm run tauri dev
```

### 2. Android 端（LocalFile）

```bash
cd localfile
JAVA_HOME=/path/to/jdk-17 ./gradlew assembleRelease
# 产物：app/build/outputs/apk/release/LocalFile.apk
```

### 3. Relay（可选，自建服务器）

```bash
cargo test --manifest-path relay-server/Cargo.toml
docker compose -f relay-server/docker-compose.yml up -d --build
```

部署细节见 [relay-server/deploy/README.md](relay-server/deploy/README.md)。

### 4. 填写 API Key

两端都**不需要在构建时提供 key**：

1. 到 [platform.deepseek.com](https://platform.deepseek.com) 申请 Key
2. Windows：设置页「DeepSeek API Key」→ 填写 → 保存（可点「测试联通」验证）
3. Android：设置页「DeepSeek API Key」→ 填写 → 保存（可用「DeepSeek 连接」检测）

Key 只保存在本地，仓库与安装包里都不含任何 key。

---

## 📦 发布产物

| 文件 | 平台 | 版本 / 大小 | SHA256 |
| --- | --- | --- | --- |
| `LocalMindSetup.exe` | Windows | v0.3.2 · 53.8 MB | `20C6A54733598EECA98BEABACED273E803B17AFED572BA6C26B16A0E75BB1724` |
| `LocalMind.exe` | Windows | v0.3.2 免安装单文件 · 22.4 MB | `E5B1EABE18FB31ADBA301A14E31D717959B52513DA5D9C7A55F0F9E23FFCF9BE` |
| `LocalFile.apk` | Android | v0.3.2 (versionCode 3) · 16.8 MB | `18A0DF93EA84E4EF118ED9F49846FC2CF3ABDB7BCC96A653B9527C4A00F50EA8` |

> Windows 安装包已内置 Python Agent 与文档生成器，**装完即用，无需另外安装 Python / Office**。
> 免安装版需把 `LocalMind.exe` 与 `LocalMindScripts/` 放在同一目录。

---

## 🧪 测试

```bash
cargo test --manifest-path localmind/src-tauri/Cargo.toml      # 22 项
cargo test --manifest-path relay-server/Cargo.toml              # 3 项
localmind/scripts/build/agent-venv/Scripts/python.exe -m pytest localmind/scripts/tests -q   # 13 项
npm --prefix localmind run build                                # TypeScript 类型检查 + 构建
```

CI 流水线位于 `.github/workflows/`（LocalMind / LocalFile / Relay 三条）。

---

## 📚 文档

- [开发路线图](开发计划/开发路线图.md) —— 当前唯一权威基线：阶段顺序、职责边界、安全红线、验收标准
- [CONTEXT.md](CONTEXT.md) —— 项目当前状态、领域语言与关键决策（改架构前必读）
- [Relay-first ADR](开发计划/ADR-002-relay-first.md) —— 同类项目调研、传输方案与安全边界
- [账号与游客模式 ADR](开发计划/ADR-003-account-and-guest-mode.md) —— 双模式、设备授权与远控边界
- [IP-only TLS 与内置 CA ADR](开发计划/ADR-004-ip-only-tls-and-client-ca-pinning.md) —— 无域名部署下的 TLS 信任方案
- [默认模型 ADR](开发计划/ADR-005-default-model-deepseek-flash.md) —— 在线模型统一为 `deepseek-flash`
- [需求完成度核对](开发计划/需求完成度核对.md) —— 历史需求逐条状态（已完成 / 部分 / 未完成）与剩余排期
- [历史架构设计](开发计划/) —— 早期调研与设计，旧 RelayCloud / 云端网关方案仅供历史参考

---

## 📄 许可证

[MIT](LICENSE) © 2026 Skyworld

---

## ⚠️ 免责声明

本项目为校级学习 / 演示项目（部署规模 ≤ 50 用户）。Relay 演示环境为纯公网 IP（无域名、无 ICP 备案）。文档基于当前代码状态编写，可能随开发演进；`run_command` 等高权限工具会真实修改系统状态，请自行确认后再执行。
