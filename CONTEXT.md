# LocalAITools — 项目记忆

> 记录当前状态、领域语言与关键决策。**改架构 / 接新功能 / 打包发布前先读本文件**。
> 详细路线图、接口契约、验收标准和施工边界见 `开发计划/开发路线图.md`（当前唯一权威基线）；`开发计划/README.md` 等旧文档仅作历史参考。

## 项目是什么

个人 AI 双端工具（校级项目，≤50 用户）：
- **LocalMind**（Windows 桌面，Tauri 2 + React + Rust + Python agent）：本地 AI 助手
- **LocalFile**（Android，Kotlin + Jetpack Compose）：文件 AI 助手
- ~~RelayCloud~~（云端中继）：**已从代码移除**，仅留 shared-contract 契约与历史文档

## 领域语言（共享词汇）

| 词 | 含义 |
|---|---|
| Agent | LocalMind 的 Python 子进程（Pydantic AI 2.x），负责 规划→工具调用→反思 循环 |
| Agent 工具 | agent 可调用的 7 个工具（见下节） |
| 模式 | 在线（DeepSeek 云端）/ 离线（Ollama 本地模型） |
| make_doc | 文档生成器 exe（PPT/Word/Excel/PDF），agent 的 create_doc 底层调用 |
| AgentServer | `localmind/scripts/agent_server.py`，SSE HTTP 服务，Rust 懒启动 |

## 当前状态（2026-09-10 快照）

- **开发基线**：`开发计划/开发路线图.md`；Phase 0、Phase 1 已完成并验收通过；Phase 2 尚未开始。
- **Harness v2 已默认为生产路径**：结构化工具错误、单 Turn 重复调用拦截、请求/工具预算、单工具超时、输出截断、Turn 级 Trace。baseline 仅供 A/B 复现。
- **Phase 1 评测**：同一模型 `deepseek-chat` 两轮 12 任务 A/B，baseline 18/24，Harness v2 24/24；详细结果见 `开发计划/Phase1-Harness评测报告.md`。
- **GUI 已美化**：蓝紫渐变设计系统、顶栏（模型徽章/在线状态/主题切换）、底部状态栏、欢迎页 + 6 快捷指令卡片、模型/设置页卡片化
- **Agent 工具 7 个**：write_file / read_file / list_dir / move_file / open_app / read_clipboard / create_doc
- **已修复**：剪贴板中文乱码（ctypes 直读 UTF-16）、新对话残留旧流程（切换会话清空 toolCalls/thinkingSteps）、输入框旁重复快捷指令已删
- **已打包（Phase 1）**：`LocalMind.exe`（免安装）+ `LocalMindSetup.exe`（NSIS）。Setup SHA256 `3B975891851C08967790DCAD968871114B50D6F6304376256C5066B65963E5D1`；Agent SHA256 `58327F347C48092B868F31F140E8E2D4C18945C4718F2D41EAD4D97BCC7698F1`

## 关键决策（ADR 摘要）

1. **API key 经环境变量注入 + 构建期烘焙**：优先级 = 运行时环境变量 `LOCALMIND_DEEPSEEK_KEY` > 编译期 `option_env!("LOCALMIND_DEEPSEEK_KEY")`（构建时注入，使打包出的 exe 开箱即用）。源码/仓库不存明文 key。⚠️ 烘焙后 key 可从安装包提取，适用于低额度/备用 key，勿用主账号高额度 key。
2. **云端 RelayCloud 方案已移除**：当前不建设云端中继和 new-api 网关；双端远程控制仍是产品目标，但必须走 Tailscale/局域网受控通道，且先完成 Phase 0-1；状态栏不得展示未实现的“远程设备”占位信息。
3. **Agent 工具在 Python 侧实现**（agent_server.py），不依赖 Rust IPC——新增工具 = 改 Python + 重新打包 localmind-agent。
4. **安全红线**：`run_command`、任意 Shell/executable 和远程任意命令永久禁止暴露给 Agent；`delete_path` 在完成 Permission Gateway、备份、审计和 Undo 前不得开放。
5. **Harness 先测量再优化**：固定评测集、结构化 Trace、同模型 A/B 是后续性能判断依据；不通过盲目换模型掩盖 Harness 缺陷。v2 已证明成功率收益，但 Prompt/token 成本仍需后续压缩。

## 打包要点（踩过的坑，环境不可自明）

- 本机工具链（2026-09-10 验证）：VS 2026 Community（`C:\Program Files\Microsoft Visual Studio\18\Community`）+ Rust（`C:\Users\world\.cargo\bin`）+ Node（`C:\APP\node.exe` / `C:\APP\npm.ps1`）+ Python venv（`localmind\scripts\build\agent-venv`）
- **LIBCLANG_PATH** 必须指向 `C:\Users\world\AppData\Roaming\Python\Python311\site-packages\clang\native`（llama-cpp-sys 的 bindgen 需要 libclang.dll）
- **cmake** 不在 PATH，用 CLion 自带：`C:\APP\CLion 2026.2.1\bin\cmake\win\x64\bin` 加进 PATH
- **NSIS**：`C:\Users\world\AppData\Local\tauri\nsis-3.11\Bin\makensis.exe`
- 构建命令：`build-localmind.ps1` → `build-installer.ps1`（脚本默认路径可用 `LOCALMIND_*` 环境变量覆盖）
- Agent 重新打包：`localmind\scripts\build\agent-venv\Scripts\python.exe localmind\scripts\pack_agent_exe.py`（venv 只装 pydantic-ai-slim[openai] + pyinstaller）
- **key 烘焙**：build-localmind.ps1 会从用户环境变量读 LOCALMIND_DEEPSEEK_KEY 并在构建期注入（option_env!），打包出的 LocalMind.exe 开箱即用；源码不含明文 key。build-localmind.ps1 / build-installer.ps1 已改为相对 PSScriptRoot 的路径，NSIS 通过 -DSTAGE_DIR 传暂存目录。

- DeepSeek key 已配置为用户环境变量 `LOCALMIND_DEEPSEEK_KEY`（本地测试 key 勿外泄）
