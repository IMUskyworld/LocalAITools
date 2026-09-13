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
| Agent 工具 | agent 可调用的 9 个工具（见下节） |
| 模式 | 在线（DeepSeek 云端）/ 离线（Ollama 本地模型） |
| make_doc | 文档生成器 exe（PPT/Word/Excel/PDF），agent 的 create_doc 底层调用 |
| AgentServer | `localmind/scripts/agent_server.py`，SSE HTTP 服务，Rust 懒启动 |

## 当前状态（2026-09-13 快照）

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
- **Agent 工具 9 个**：write_file / read_file / list_dir / move_file / open_app / read_clipboard / create_doc / run_command / delete_path
- **已修复**：剪贴板中文乱码（ctypes 直读 UTF-16）、新对话残留旧流程（切换会话清空 toolCalls/thinkingSteps）、输入框旁重复快捷指令已删
- **已打包（v0.3.0 Phase 4，2026-09-12 04:00，含确认UI+审计日志+记忆管理+工具卡片+AI摘要+远程控制）**：`LocalMind.exe`（19.9MB，SHA256 `0C8DFC7046F96D00CBC0278A26D628440957BB4C23DAA544993866D21308C6B3`）+ `LocalMindSetup.exe`（50.9MB）+ `LocalFile.apk`（14.9MB）；APK 不含任何内置 key。
- **双端远控端到端打通（2026-09-13 深夜）**：修掉叠加的 6 处 bug —— ①`build_tools` 把确认回调当成 `emit_thinking` 传，前端收不到 `confirm` SSE → 弹不出确认框 → 工具 60s 超时（现改为真正的 `confirm` 事件 + 聊天区确认卡片）；②`ipc.ts` 的 `initAttempted` 布尔竞态让 `loadSessions`/`createSession` 同 tick 相撞 → 从不建会话、错误被吞（改 Promise memo 化）；③`commands.tenant_id` 外键指向遗留 `pairings` 表（Relay `migrate_v3` 重建表去 FK）；④Android 命令信封漏 `tenant_id` → 路由 403；⑤Relay 权限白名单缺 `chat_task`；⑥Windows 状态回传 `tenant_id: None` → 结果回不来。记忆管理改为读 `session_summaries`（`memory_entries` 从未写入）。
- **已打包（v0.3.1 APK，2026-09-13 23:50）**：`LocalFile.apk`（17.3MB，versionCode 2，SHA256 `489DF3783B893C2BCB2C45B282E91DF02DD25DC8C04728D6831146F86949F19A`），修复「命令执行成功后 Android 主动关连接，OkHttp 补发 `onFailure` 把正确结果覆盖成 `WebSocket error`」——`RelayWssClient` 加终态标志、`RemoteControlViewModel` 改用 `AtomicBoolean` 并在 `onError` 里判终态。模拟器 E2E 复测：手机发指令 → 模拟 PC 收到 → 回传 done → 结果卡片正确显示。
- **全仓库 bug 审计 + 修复（2026-09-14）**：① `cargo test` 的 key 回归测试直接写真实 `%APPDATA%\LocalMind\auth.json`，**会把用户已配置的 key 覆盖成测试值**（本次审计时确实冲掉了用户 key，已还原）→ StorageManager 增加可注入的 `auth_path`（`with_paths`），测试改用临时文件；② 测试仍断言「run_command 永久禁止」（旧红线）与现行高危+确认策略冲突 → 改为断言必须声明且 confirmation_required=True；③ 双端 access token 仅 30 分钟且除启动路径外不刷新，App 长开后远控/账号操作必报错 → Android/Windows 均加 403005 自动刷新 + 重试（`withFreshAccessToken`）；④ Agent 子进程崩溃/被杀后只返回死端口、不再拉起 → `get_or_spawn` 增加 `try_wait()` 存活检查并自动重启；⑤ 审计日志风险等级写死 `log.success ? 'L2' : 'L2'` → 按工具名映射 L0~L4；⑥ 会话摘要喂的是用户消息而非助手回复，且 fallback 分支永远不触发 → 改用助手正文并真接上 fallback；⑦ 停止生成时待确认的高危操作不结掉，卡片残留且事后点确认仍会真执行 → stopGeneration 先 resolveConfirm(false)；⑧ API key 加密存储曾被 aaf1459 整体回退（Android `EncryptedPrefs` 未被引用、Windows auth.json 明文） → 两端恢复加密：Windows 写 `ENC:` + AES-256-GCM（device_id 派生密钥）、Android EncryptedSharedPreferences，均兼容历史明文并自动迁移；⑨ 「关于」页版本号用 `BuildConfig.VERSION_NAME`（编译期常量会被内联，改版本后增量编译实测残留为 0.3.0）→ 改为运行时从 PackageManager 读取，预置固定这类“构建缓存导致版本错误”的坑。

- **已打包（v0.3.2，2026-09-14）**：`LocalMind.exe`（22.4MB，SHA256 `1F2834C7964C31CACF4B84C7B266DC1DB3DC9EBCDEC4BFC5300268A53C5BFB49`）+ `LocalMindSetup.exe`（53.8MB，SHA256 `C2B0CBBB54F297ECBBA6D0FC16FF4B0F6CF91E064F56DC83E450F833A60F9D35`）+ `LocalFile.apk`（17.3MB，versionCode 3，SHA256 `C4309A86726D7AAF30F8E2A51BD7CAD608015AE8EAF7784FE9A5076DCF096A5B`）。

## 关键决策（ADR 摘要）

1. **API key 用户自填（v0.3.0 起）**：安装包不内置任何 key；用户在设置页填写后写入 `%APPDATA%\LocalMind\auth.json`，值以设备绑定的 AES-256-GCM 密文存储（`ENC:` 前缀，密钥由 device_id 派生；历史明文自动迁移）。优先级 = 前端传入的 `body.token` > 环境变量 `LOCALMIND_DEEPSEEK_KEY`（仅供开发调试）。安装/卸载会自动清理历史遗留的 `LOCALMIND_DEEPSEEK_KEY` 环境变量。⚠️ 历史坑：旧安装包曾把 key 硬编码进 installer.nsi 并写入 HKCU\Environment，导致换 key 后仍用旧 key。
2. **Relay-first 已重新立项（ADR-002）**：双端默认通过自建 WSS Relay 出站连接，不要求普通用户安装 Tailscale。Relay 只负责设备 token、配对、幂等转发、离线队列和状态；Windows 本机权限、确认和审计仍是最终授权边界。旧 RelayCloud 三组件、new-api 网关和公网裸端口仍不复活。
3. **Agent 工具在 Python 侧实现**（agent_server.py），不依赖 Rust IPC——新增工具 = 改 Python + 重新打包 localmind-agent。
4. **高危工具策略（2026-09-13 修订，推翻旧红线）**：`run_command`（L4）/ `delete_path`（L3）不再"永久禁止"，改为**显式声明 + 强制用户确认 + 审计**：Python 侧 `request_confirmation` 发 `confirm` SSE 并阻塞等待，60s 超时视为拒绝；前端确认卡片展示完整参数后用户点「确认执行」才真正运行；每次工具调用写 `audit_log`（风险等级按工具名映射，不再恒为 L2）。仍然禁止：未确认的静默执行、把任意 shell 暴露给远程指令、绕过 Tool Registry 注册工具。
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
- **key 存储**：`auth.json` 位于 `%APPDATA%\LocalMind\auth.json`（明文 JSON，字段 `deepseek_api_key`）。Rust `storage::get_api_key/set_api_key` 读写此文件；前端 `runAgent` 把它放进 `body.token` 传给 Python agent。**构建脚本不需要任何 key**。

- **Android 也是用户自填 key**：`localfile` 的 `BuildConfig.DEEPSEEK_API_KEY` 固定为空字符串，用户在设置页填写后存 DataStore 并写入 `DeepSeekConfig.API_KEY`。APK 内不应出现任何 `sk-` 字符串（可用 dex 搜 `sk-` 验证）。
- **卸载会清理数据**：`installer.nsi` 的 Uninstall 段会删除 `%APPDATA%\LocalMind`（auth.json / localmind.db / traces），即『卸载软件自动删除记忆文档』。
