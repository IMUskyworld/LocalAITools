# LocalAITools — 项目记忆

> 记录当前状态、领域语言与关键决策。**改架构 / 接新功能 / 打包发布前先读本文件**。
> 详细路线图、接口契约、验收标准和施工边界见 `开发计划/开发路线图.md`（当前唯一权威基线）；`开发计划/README.md` 等旧文档仅作历史参考。

## 项目是什么

个人 AI 双端工具（校级项目，≤50 用户）：
- **LocalMind**（Windows 桌面，Tauri 2 + React + Rust + Python agent）：本地 AI 助手
- **LocalFile**（Android，Kotlin + Jetpack Compose）：文件 AI 助手
- **LocalMind Relay**（公网语义中继）：ADR-002 已重新立项 Relay-first；服务代码位于 `relay-server/`，只转发加密通道中的版本化 Envelope，不执行命令、不保存 DeepSeek Key、不保存明文文件内容

## 领域语言（共享词汇）

| 词 | 含义 |
|---|---|
| Agent | LocalMind 的 Python 子进程（Pydantic AI 2.x），负责 规划→工具调用→反思 循环 |
| Agent 工具 | agent 可调用的 7 个工具（见下节） |
| 模式 | 在线（DeepSeek 云端）/ 离线（Ollama 本地模型） |
| make_doc | 文档生成器 exe（PPT/Word/Excel/PDF），agent 的 create_doc 底层调用 |
| AgentServer | `localmind/scripts/agent_server.py`，SSE HTTP 服务，Rust 懒启动 |

## 当前状态（2026-09-11 快照）

- **开发基线**：`开发计划/开发路线图.md`；Phase 0、Phase 1 已完成并验收通过；Phase 2 Relay 服务端已部署并验证，下一步接入 Windows/Android 客户端。
- **Harness v2 已默认为生产路径**：结构化工具错误、单 Turn 重复调用拦截、请求/工具预算、单工具超时、输出截断、Turn 级 Trace。baseline 仅供 A/B 复现。
- **Phase 1 评测**：同一模型 `deepseek-chat` 两轮 12 任务 A/B，baseline 18/24，Harness v2 24/24；详细结果见 `开发计划/Phase1-Harness评测报告.md`。
- **Relay MVP 已完成服务端第一版**：`relay-server/` 提供设备注册、一次性配对码、WSS 转发、命令白名单、`command_id` 幂等、离线队列和状态回传；Rust 单元 + 双端 WSS 集成测试通过。
- **Relay 已上线（2026-09-11）**：https://39.107.53.230/health 已通过本机与公网 HTTPS 验证；Caddy internal CA 固定信任文件为 `relay-server/certs/localmind-relay-ca.crt`。当前无域名/ICP，属校级演示部署。
- **Relay TLS 采用客户端内置 CA（ADR-004，2026-09-11）**：Caddy internal CA 使 WebView2/Android 系统信任链无法校验，账号与设备请求因此改为 Rust `relay_http_request`（rustls + 编译期嵌入 CA）与 Android `RelayTls`（OkHttp 组合信任 + raw 资源）发起；CA 轮换需同步替换 `localmind/src-tauri/certs/` 与 `localfile/app/src/main/res/raw/` 两份副本并重新打包。
- **Relay 账号层已上线（2026-09-12）**：服务器恢复后 Docker 重编译完成，注册/登录/设备授权 API 全部验证通过（`https://39.107.53.230/v1/auth/register` 等）；双端账号页已对接。
- **游客/账号与设备授权方案已冻结（ADR-003）**：游客本地即用；登录同一账号只用于设备归属和设备发现，远程控制必须由 Windows 本机确认后建立设备配对。
- **账号层已进入实现（2026-09-11）**：Relay 已增加注册/登录/刷新/退出、账号设备登记与移除、控制授权请求（Android → Windows 本机确认）与控制配对撤销；密码 Argon2id、access/refresh token 分离且只存哈希。Windows 账号页与 Android 账号页已接入，LocalFile debug/release 均可编译，release 使用调试签名供演示安装。已推送到 GitHub（main / codex/phase-2-relay = 03b6c2c）。
- **GUI 已美化**：蓝紫渐变设计系统、顶栏（模型徽章/在线状态/主题切换）、底部状态栏、欢迎页 + 6 快捷指令卡片、模型/设置页卡片化
- **Agent 工具 7 个**：write_file / read_file / list_dir / move_file / open_app / read_clipboard / create_doc
- **已修复**：剪贴板中文乱码（ctypes 直读 UTF-16）、新对话残留旧流程（切换会话清空 toolCalls/thinkingSteps）、输入框旁重复快捷指令已删
- **已打包（Phase 2.5 完成，2026-09-12 01:30，含跨轮记忆 + session summary + shell 工具）**：`LocalMind.exe`（19.7MB，SHA256 `98903C93C2F66005962114B05035918C9DBA092AE297F48D2C8D421F3D87FA2D`）+ `LocalMindSetup.exe`（50.9MB，SHA256 `9DED6B412B2C1828146A7652650BD18A3E9A2CF152829244F26AF7F2B0566165`）+ `LocalFile.apk`（14.9MB，SHA256 `262F795935915968D5049EEAFECDA895E445FE263E264BF90BC5FBDB7811F95F`）；APK 不含任何内置 key（用户自填模式验证通过）。

## 关键决策（ADR 摘要）

1. **API key 经环境变量注入 + 构建期烘焙**：优先级 = 运行时环境变量 `LOCALMIND_DEEPSEEK_KEY` > 编译期 `option_env!("LOCALMIND_DEEPSEEK_KEY")`（构建时注入，使打包出的 exe 开箱即用）。源码/仓库不存明文 key。⚠️ 烘焙后 key 可从安装包提取，适用于低额度/备用 key，勿用主账号高额度 key。
2. **Relay-first 已重新立项（ADR-002）**：双端默认通过自建 WSS Relay 出站连接，不要求普通用户安装 Tailscale。Relay 只负责设备 token、配对、幂等转发、离线队列和状态；Windows 本机权限、确认和审计仍是最终授权边界。旧 RelayCloud 三组件、new-api 网关和公网裸端口仍不复活。
3. **Agent 工具在 Python 侧实现**（agent_server.py），不依赖 Rust IPC——新增工具 = 改 Python + 重新打包 localmind-agent。
4. **安全红线**：`run_command`、任意 Shell/executable 和远程任意命令永久禁止暴露给 Agent；`delete_path` 在完成 Permission Gateway、备份、审计和 Undo 前不得开放。
5. **Harness 先测量再优化**：固定评测集、结构化 Trace、同模型 A/B 是后续性能判断依据；不通过盲目换模型掩盖 Harness 缺陷。v2 已证明成功率收益，但 Prompt/token 成本仍需后续压缩。
6. **在线内置模型统一为 `deepseek-flash`（ADR-005）**：官方 `/models` 只登记 `deepseek-flash` / `deepseek-v4-pro`；用户口中的 `deepseek-v4-flash` 是 `deepseek-flash` 的别名（服务端归一），代码固定用官方 ID，展示名 DeepSeek V4 Flash，唯一事实来源为 `localmind/src/config/models.ts` + Agent `DEFAULT_ONLINE_MODEL` + Android `DeepSeekConfig.MODEL`。
7. **游客/账号双模式共存（ADR-003）**：游客无需账号即可使用本地能力；账号模式使用同一账号识别设备，但同账号不等于自动互控。远程控制必须经过目标设备显式授权和本机确认。

## 打包要点（踩过的坑，环境不可自明）

- 本机工具链（2026-09-10 验证）：VS 2026 Community（`C:\Program Files\Microsoft Visual Studio\18\Community`）+ Rust（`C:\Users\world\.cargo\bin`）+ Node（`C:\APP\node.exe` / `C:\APP\npm.ps1`）+ Python venv（`localmind\scripts\build\agent-venv`）
- **LIBCLANG_PATH** 必须指向 `C:\Users\world\AppData\Roaming\Python\Python311\site-packages\clang\native`（llama-cpp-sys 的 bindgen 需要 libclang.dll）
- **cmake** 不在 PATH，用 CLion 自带：`C:\APP\CLion 2026.2.1\bin\cmake\win\x64\bin` 加进 PATH
- **NSIS**：`C:\Users\world\AppData\Local\tauri\nsis-3.11\Bin\makensis.exe`
- 构建命令：`build-localmind.ps1` → `build-installer.ps1`（脚本默认路径可用 `LOCALMIND_*` 环境变量覆盖）
- Agent 重新打包：`localmind\scripts\build\agent-venv\Scripts\python.exe localmind\scripts\pack_agent_exe.py`（venv 只装 pydantic-ai-slim[openai] + pyinstaller）
- **key 烘焙**：build-localmind.ps1 会从用户环境变量读 LOCALMIND_DEEPSEEK_KEY 并在构建期注入（option_env!），打包出的 LocalMind.exe 开箱即用；源码不含明文 key。build-localmind.ps1 / build-installer.ps1 已改为相对 PSScriptRoot 的路径，NSIS 通过 -DSTAGE_DIR 传暂存目录。

- **Android 打包必须注入 key**：`localfile` 的 key 取 gradle 属性 `LOCAL_FILE_API_KEY` > 环境变量 `LOCAL_FILE_API_KEY` > 环境变量 `LOCALMIND_DEEPSEEK_KEY`；三者都没有时 BuildConfig 里是中文占位符——**APK 表面正常但在线功能全废**，发布前务必确认（可用 dex 搜 `sk-` 验证）。
- DeepSeek key 已配置为用户环境变量 `LOCALMIND_DEEPSEEK_KEY`（本地测试 key 勿外泄）
