# LocalMind + LocalFile 架构审计报告

> 审计日期：2026-09-10
> 审计范围：`localmind/`（Windows）、`localfile/`（Android）、`shared-contract/`
> 代码规模：11,214 行（前端 3,169 / Rust 1,935 / Python 1,108 / Kotlin 4,497 / 契约 505）
> 审计方式：通读核心模块源码 + 调用链追踪 + 死代码扫描（非基于 README 或文件名推断）
> 本轮产出：**仅审计与架构设计，未修改任何代码**

---

## 第一部分：我对整个项目的理解

### 1.1 LocalMind 是什么

按代码实际行为描述，而不是文档描述：

**LocalMind 是一个跑在 Windows 上的 Agent，它的"大脑"是一个独立于主程序的 Python 子进程。**

真正的调用链是这样的：

1. 用户在 React 前端输入一句话
2. 前端 `useStreamChat.ts` 调 `runAgent()`（`@/api/agent.ts`）
3. `runAgent` 先问 Rust 后端要 Agent 服务的连接信息：`tauriInvoke('get_agent_config')`
4. Rust 的 `agent_process.rs::spawn_agent()` 这时才**懒启动** Python 子进程
   （打包版是 `localmind-agent.exe`，开发版是 `python scripts/agent_server.py`）
5. Python 从 stdout 打印 `PORT=<n>`，Rust 读到端口后返回给前端
6. 前端拿到端口 + token，直接 `fetch` Python 服务的 `/agent/stream`（SSE）
7. Python 里的 Pydantic AI 跑 Agent 循环，工具**在 Python 进程内直接执行**
8. 结果通过 SSE 事件流回前端（thinking / tool / delta / done / error）

**这里有个关键事实**：工具执行**完全在 Python 侧**，不经过 Tauri IPC。
`agent_server.py` 的 `build_tools()` 里，`write_file`、`read_file`、`list_dir`、
`move_file`、`open_app`、`read_clipboard`、`create_doc` 全是纯 Python 实现。

### 1.2 LocalFile 是什么

**LocalFile 是一个直接调 DeepSeek API 的 Android 文件助手，它自己手写了一套 Agent 循环。**

调用链：

1. 用户在 `ChatScreen` 输入
2. `ChatViewModel` → `ChatEngine.sendMessage()`
3. `ChatEngine` 走**四段式手写 ReAct**：
   - **规划轮**：命中关键词 → 独立 LLM 调用（不带 tools）→ 输出 `{"plan":[...]}` → 注入 system
   - **执行循环**：`maxRounds = 4`，每轮非流式拿 `tool_calls` → 执行 → 回传结果
   - **反思轮**：工具失败 → 独立 LLM 调用 → 输出 `{"reflection":..., "suggestion":...}` → 注入 system
   - **最终输出**：循环收敛后才流式吐出最终答案
4. 工具只有 **1 个**：`generate_doc`（在手机本地生成 docx/pptx/xlsx/pdf）
5. 网络层 `GatewayClient` 用 OkHttp 直连 `https://api.deepseek.com/v1`

### 1.3 两分别负责什么（按代码事实）

| 能力 | LocalMind (Windows) | LocalFile (Android) |
|---|---|---|
| Agent 循环 | Pydantic AI 托管（成熟实现） | 手写四段式（规划/执行/反思/输出） |
| 工具数量 | 7 个（Python 侧直接执行） | 1 个（generate_doc） |
| 模型 | DeepSeek（在线）+ Ollama（离线） | 仅 DeepSeek（在线） |
| 本地推理 | ❌ 无（llama.cpp 代码存在但未接入） | ❌ 无（LlamaBridge 是 stub） |
| 文件持久化 | ❌ 纯内存，重启丢失 | ✅ Room 数据库（有 DAO/Entity） |
| 会话持久化 | ❌ 纯内存 | ✅ Room 数据库 |
| 双端通信 | ❌ 无 | ❌ 无 |

**一个反直觉的事实**：作为"文件处理助手"的 LocalFile 有数据库持久化，
而作为"核心智能执行端"的 LocalMind **没有持久化**。

### 1.4 当前系统如何工作（核心数据流）

```
Windows 端实际数据流：

  用户输入
    │
    ▼
  ChatPage.tsx
    │  sendMessage(text)
    ▼
  useStreamChat.ts ──── getCurrentMessages() 取全部历史
    │                    （无窗口限制、无压缩）
    ▼
  agent.ts::runAgent()
    │
    ├─① tauriInvoke('get_agent_config')  ──► Rust agent_process.rs
    │                                          └─ 懒启动 Python 子进程
    │                                          └─ 生成随机 24B token
    │                                          └─ 读 stdout "PORT="
    │◄───────────────── { port, token } ────────┘
    │
    └─② fetch http://127.0.0.1:{port}/agent/stream  (SSE, 带 X-LocalMind-Token)
           │
           ▼
       Python agent_server.py
           │  build_system_prompt(desktop)  ← 注入真实桌面路径
           │  build_tools()                 ← 7 个纯 Python 工具
           │  Agent(model, tools, retries=1)
           │
           ├─► OpenAIChatModel(deepseek-chat)  [在线]
           └─► OllamaModel(qwen2.5:7b)         [离线]
           │
           │  run_stream_events()  ── 循环 ──┐
           │       ├─ FunctionToolCallEvent  │  → SSE "thinking"
           │       ├─ 执行工具（Python 直跑）│  → SSE "tool"
           │       ├─ FunctionToolResultEvent│  → 结果回灌上下文
           │       └─ 无 tool_calls → 结束   │
           │                                  ──┘
           │
           ▼
       SSE 事件流 → 前端 → chatStore（toolCalls / thinkingSteps / content）
           │
           ▼
       persistAssistantMessage()  ⚠️ 只存最终 content
                                   ⚠️ tool_calls / tool_results 全部丢弃
```

### 1.5 我对"当前系统"最重要的一句判断

**LocalMind 的 Agent 能力实际只由 `agent_server.py` 一个文件（731 行）承载。**
它写得其实不差——有工具注册、有错误回灌、有小模型文本兜底、有 SSE 流式。

**它"性能差"的主因不在这一层，而在它外围的三处断裂**：

1. **跨轮记忆断裂**：每轮结束后工具调用历史被丢弃
2. **Context 无管理**：历史全量发送，长会话必然劣化
3. **能力面被冻死在 7 个工具**：而这 7 个工具之外，Rust 侧还有 5 个更强也更危险的
   命令（`delete_path` / `run_command` / `append_file` / `rename_path` / `ollama_chat`）
   **全部注册了但没人用**——既没用上，也没卸掉

---

## 第二部分：当前架构图

### 2.1 逻辑架构（User → System）

```
                        ┌──────────────────┐
                        │      User        │
                        └────────┬─────────┘
                                 │
              ┌──────────────────┴──────────────────┐
              │                                     │
      ┌───────▼────────┐                   ┌────────▼───────┐
      │  LocalMind     │                   │   LocalFile    │
      │  (Windows)     │                   │   (Android)    │
      └───────┬────────┘                   └────────┬───────┘
              │                                     │
              │  React + Tauri                      │  Jetpack Compose
              │  (WebView2)                         │
              │                                     │
      ┌───────▼─────────────────┐          ┌────────▼─────────────┐
      │  useStreamChat          │          │  ChatEngine.kt       │
      │  chatStore (zustand)    │          │  ChatViewModel       │
      └───────┬─────────────────┘          └────────┬─────────────┘
              │                                     │
              │ SSE over 127.0.0.1                  │ OkHttp HTTPS
              │                                     │
      ┌───────▼─────────────────┐          ┌────────▼─────────────┐
      │  Python Agent Service   │          │  GatewayClient       │
      │  (Pydantic AI)          │          │  (直连 DeepSeek)     │
      │  ├─ Model Adapter       │          └────────┬─────────────┘
      │  ├─ Agent Loop          │                   │
      │  ├─ Tool Registry (7)   │          ┌────────▼─────────────┐
      │  └─ Tool Executor       │          │  手写 ReAct 循环      │
      └───────┬─────────────────┘          │  ├─ 规划轮           │
              │                            │  ├─ 执行轮 (max 4)   │
              │                            │  ├─ 反思轮           │
              │                            │  └─ 输出轮           │
              │                            └────────┬─────────────┘
              │                                     │
      ┌───────▼─────────────────┐          ┌────────▼─────────────┐
      │  7 个 Python 工具        │          │  1 个工具            │
      │  write_file  read_file  │          │  generate_doc        │
      │  list_dir    move_file  │          │  (本地生成 4 类文档)  │
      │  open_app    clipboard  │          └──────────────────────┘
      │  create_doc             │
      └───────┬─────────────────┘
              │
      ┌───────▼─────────────────┐
      │  Windows System         │
      │  文件系统 / 进程 / 剪贴板│
      └─────────────────────────┘


  ⚠️ 旁挂（已注册但无人调用，构成攻击面）：
      Rust tools.rs 的 12 个 Tauri Command
        ├── delete_path   ← 递归删除，无确认
        ├── run_command   ← 任意 PowerShell
        ├── append_file / rename_path / ollama_chat
        └── ...（其余 7 个与 Python 工具功能重叠）
```

### 2.2 双端通信架构（现状）

```
   LocalFile                                        LocalMind
   (Android)                                        (Windows)
       │                                                 │
       │         ✂️  完全没有任何通信链路  ✂️              │
       │                                                 │
       │  ┌─────────────────────────────────────────┐    │
       │  │  shared-contract/ （505 行）            │    │
       │  │  ├─ envelope/v1/Envelope.kt   ← 无人 import │
       │  │  ├─ envelope/v1/envelope.ts   ← 无人 import │
       │  │  ├─ envelope/v1/envelope.rs   ← 无人 import │
       │  │  ├─ error-code/ErrorCodes.kt  ← 无人 import │
       │  │  └─ whitelist/actions.json    ← 无人 import │
       │  └─────────────────────────────────────────┘    │
       │                                                 │
       │  localmind/src/api/wssClient.ts (125 行)        │
       │         └── 无人 import（RelayCloud 残留）       │
       │                                                 │
       │  common/mod.rs:  wss_url = "wss://relay.example.com/ws"
       │         └── 硬编码假域名，从未连接               │
```

**结论**：双端互联目前是 **0 基础设施**。shared-contract 是设计稿，
不是可运行代码。所谓"复用旧契约"实际上要**从零实现**。

### 2.3 权限面（现状）

```
  WebView2 (React)
       │  可以调用以下全部命令（无白名单、无确认、无审计）
       ▼
  Tauri invoke_handler
       │
       ├── tools::write_file      ── 任意路径覆盖写
       ├── tools::append_file     ── 任意路径追加
       ├── tools::read_file       ── 任意路径读
       ├── tools::list_dir        ── 任意目录枚举
       ├── tools::delete_path     ── ★ 任意路径递归删除（remove_dir_all）
       ├── tools::rename_path     ── 任意路径移动
       ├── tools::run_command     ── ★★ 任意 PowerShell 命令
       ├── tools::open_app        ── 启动任意程序
       ├── tools::create_doc      ── 生成文档
       └── tools::get_common_paths
```

**关键**：Python Agent 只能用 7 个工具（不含 delete / run_command），
但 **Tauri 命令面比 Agent 工具面更大更危险**，且这层没有任何保护。

---

## 第三部分：问题清单

### P0 — 必须优先解决

| # | 问题 | 证据 | 影响 |
|---|---|---|---|
| **P0-1** | **跨轮记忆断裂**：工具调用历史不落盘，下一轮完全失忆 | `useStreamChat.ts:92` 只 `persistAssistantMessage(content)`；`chat/mod.rs::save_message` 只存 content | Agent 无法做长任务；重复调用工具；无法回答"你刚才做了什么" |
| **P0-2** | **应用重启数据全丢**：StorageManager 是纯内存 Vec | `storage/mod.rs:11-15` 注释自述"MVP 使用内存存储，正式版切换至 SQLite" | 会话无法留存；Memory 功能零基础设施 |
| **P0-3** | **`run_command` 已注册且无防护** | `main.rs:78` 注册；`tools.rs:158` 直接跑 PowerShell | WebView 内任意 JS 可执行任意系统命令 |
| **P0-4** | **`delete_path` 已注册且无防护** | `main.rs:76`；`tools.rs:129` `remove_dir_all` | 任意路径递归删除，无确认、无备份、无回收站 |
| **P0-5** | **文件操作无路径边界** | 全部 `tools.rs` 函数直接 `PathBuf::from(path)` | 可写/删系统目录、用户任意文件 |
| **P0-6** | **Context 无窗口管理** | `useStreamChat.ts:69` `getCurrentMessages()` 全量取历史 | 长会话 token 线性膨胀，必然超限或劣化 |
| **P0-7** | **工具定义三处重复且不一致** | Python 7 个 / `tools.ts` 3 个（死代码）/ `tools.rs` 12 个 | 行为不一致；改一处漏两处；"Agent 有哪些能力"无法回答 |

### P1 — 重要问题

| # | 问题 | 证据 | 影响 |
|---|---|---|---|
| **P1-1** | `executeTool` 无工具名白名单 | `tools.ts:98` `tauriInvoke(name, args)` 直接透传 | 若模型输出 `run_command`，会真的执行 |
| **P1-2** | llama.cpp 推理引擎完全孤立 | `inference/mod.rs` 全部引用仅在本文件内（含 5 个测试） | 编译体积 + 构建时间白白付出；误导后来者 |
| **P1-3** | 多处 mock / 空壳实现 | `chat/mod.rs:144` 回显 mock；`model/mod.rs:121-146` 全 `Ok(true)`；`storage/mod.rs:123` `save_config` 空壳；`model/mod.rs:201` `check_device` 硬编码 i7-12700H | UI 显示的能力与实际不符 |
| **P1-4** | `AppConfig::load()` 硬编码假配置 | `common/mod.rs:71` `wss://relay.example.com/ws` | 无真实配置系统 |
| **P1-5** | `get_app_info` 每次生成新 device_id | `storage/mod.rs:133` `Uuid::new_v4()` | 设备身份不稳定，双端无法配对 |
| **P1-6** | `chat::*` 与 `chat_api::*` 两套会话 API | `main.rs:43-57` | 职责重复，维护双份 |
| **P1-7** | 附件内容不进历史 | `ChatPage.tsx:148` 发送后 `clearAttachments()`；`withAttachments` 仅当轮注入 | 多轮无法引用附件 |
| **P1-8** | 手机端 API key 编进 APK | `DeepSeekConfig.kt:11` `BuildConfig.DEEPSEEK_API_KEY` | 反编译可提取 |
| **P1-9** | 无审计日志 / 无备份 / 无 undo | 全项目无相关实现 | 出事无法追溯、无法回滚 |
| **P1-10** | shared-contract 与 wssClient.ts 全死代码 | grep 无任何 import | 双端互联零基础 |

### P2 — 可以后续解决

| # | 问题 | 说明 |
|---|---|---|
| P2-1 | 模型显示名硬编码 "Deepseek-V4-Pro" | 实际用 `deepseek-chat`，UI 与事实不符（建议统一为真实模型名） |
| P2-2 | 手机端 `maxRounds = 4` 硬编码 | 长任务不够用，应可配置 |
| P2-3 | 手机端无取消机制 | `stopGeneration()` 是空实现（`ChatEngine.kt:326`） |
| P2-4 | 无超时控制 | Python 端 `create_doc` 有 120s，其余工具无超时 |
| P2-5 | 错误处理风格不统一 | 有的返回 `ToolResult::err`，有的返回 `Err`，有的吞异常 |
| P2-6 | 无测试 | 除 `inference/mod.rs` 内 5 个（依赖真实模型、大概率跑不了）外，全项目无测试 |

---

## 第四部分：根因分析

### 4.1 直接回答："LocalMind 的 Agent 性能差，哪些来自模型，哪些来自 Harness？"

我用**证据**而非感觉来拆：

#### 结论先行

| 归因 | 占比（我的判断） | 依据 |
|---|---|---|
| **Harness / Runtime** | **约 60%** | 跨轮记忆断裂、Context 无管理、工具能力面被冻死 |
| **Model** | **约 25%** | 离线 7B 模型确实弱，但已有文本兜底缓解 |
| **Architecture** | **约 15%** | 三套工具定义、两套会话 API、无持久化 |

#### 逐条论证

**① Harness 问题（最主要）**

- **跨轮记忆断裂是头号杀手**
  `useStreamChat.ts:92` 只保存最终文本。Agent 跑完一轮后，
  "我调用了哪些工具、返回了什么" **全部蒸发**。
  下一轮 Agent 只知道"我上一轮说了句'已整理好'"。

  后果不是"性能差"，而是**能力缺失**：
  - 用户问"你刚才移动了哪些文件？" → 答不上来（信息根本不在上下文里）
  - 需要 5 步以上的任务 → 第二轮开始就在瞎猜
  - **这不是模型笨，是模型没数据**

- **Context 无窗口管理**
  `getCurrentMessages()` 全量返回。会话越长，每轮 token 越多。
  没有 summary、没有滑动窗口、没有裁剪策略。
  长会话必然：要么超限报错，要么模型被海量历史稀释注意力。

- **工具能力面被人为冻死**
  这是最讽刺的一条：`tools.ts:3-4` 的注释写着
  > 工具数量经过验证——本地小模型（qwen2.5:7b）在工具过多时不稳定，
  > 精简为 3 个核心工具保证可靠调用

  为了迁就 7B 模型，把工具砍到 3 个。但**在线用的是 DeepSeek**，
  完全不需要为 7B 让路。**这是在在线模式下也自我阉割**。

  而 Python 端注册了 7 个，Rust 端还躺着 12 个（含更强的 `run_command`）。
  结果是：能力有，但没接上；接上的，又被砍了。

**② Model 问题（真实存在，但是次要）**

- 离线 qwen2.5:7b 确实弱：不输出标准 `tool_calls`、复杂指令理解差
- 但项目**已经做了正确的缓解**：`parse_tool_call_from_text` 文本兜底
  （`agent_server.py:374`），这个设计是对的
- 所以模型问题是"上限低"，不是"设计错"

**③ Architecture 问题**

- 三套工具定义 → 没有人能回答"Agent 到底能干什么"
- 无持久化 → Memory 无从谈起
- Mock 实现混杂 → 无法判断哪些能力是真的

### 4.2 一个必须点破的判断

**"Agent 性能差"这个说法本身可能 masking 了真正的问题。**

从代码看，你遇到的很多"Agent 笨"的瞬间，很可能是：
- 它**不记得**上一轮做了什么（P0-1）
- 它**看不到**工具执行结果（P0-1）
- 它**没有**完成这个任务所需的工具（工具被砍到 7 个 / 3 个）

这三种情况，**换更强的模型也解决不了**。

所以我建议：**在换模型之前，先做 Phase 1（见第十部分），
把记忆和 Context 修好**。很可能修完后，同一个模型的表现会明显不同。

---

## 第五部分：目标架构

### 5.1 设计原则

1. **单一事实来源**：工具只有一处定义，其他端从它生成
2. **Harness 与 Model 解耦**：换模型不改循环（这点现在做得不错，保持）
3. **能力面显式化**：Agent 能做什么，由 Tool Registry + Permission 共同决定
4. **默认安全，按需提权**：不是"什么都要确认"，而是"低风险自动、高风险拦截"
5. **渐进式**：不重写，先修断裂点

### 5.2 目标分层

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 6  客户端层                                            │
│  LocalMind (Tauri/React)  ·  LocalFile (Compose)  ·  未来 Web │
├─────────────────────────────────────────────────────────────┤
│  Layer 5  通信层（新增）                                       │
│  Command / Event 协议 · 配对鉴权 · 文件传输 · 状态同步          │
│  传输：Tailscale(tsnet) 优先，局域网直连降级                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 4  Agent Runtime（LocalMind 核心）                     │
│  ┌──────────┬──────────┬──────────┬──────────┬───────────┐  │
│  │ Session  │ Context  │  Agent   │  Tool    │ Permission│  │
│  │ Manager  │ Manager  │  Loop    │ Registry │  Gateway  │  │
│  └──────────┴──────────┴──────────┴──────────┴───────────┘  │
├─────────────────────────────────────────────────────────────┤
│  Layer 3  Model Adapter                                      │
│  OpenAIChatModel · OllamaModel · （未来: Anthropic / 本地）    │
│  统一走 OpenAI 兼容协议 ← 现有设计已正确                        │
├─────────────────────────────────────────────────────────────┤
│  Layer 2  Memory 系统（新增）                                  │
│  Working Memory（当前任务） / Session Memory / Long-term      │
├─────────────────────────────────────────────────────────────┤
│  Layer 1  持久化层（新增）                                     │
│  SQLite：sessions / messages / tool_calls / memory / audit    │
├─────────────────────────────────────────────────────────────┤
│  Layer 0  系统层                                              │
│  文件系统 · 进程 · 剪贴板（Windows）  ·  文件 URI（Android）    │
└─────────────────────────────────────────────────────────────┘
```

### 5.3 关键设计决策

**决策 1：工具注册中心化**

```
                    ┌─────────────────────┐
                    │  tool-manifest.yaml │  ← 唯一事实来源
                    │  （或 Python 装饰器  │
                    │     自动导出）       │
                    └──────────┬──────────┘
                               │ 生成
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
      Python 工具实现    前端展示元数据    权限等级声明
      （执行体）         （UI 显示）      （Permission Gateway 消费）
```

现在的问题就是没有这个中心，导致 Python / TS / Rust 各写一套。

**决策 2：Context 分层**

```
  ┌────────────────────────────────────────────┐
  │  System Prompt（固定 + 工具 Schema）         │  ← 常驻
  ├────────────────────────────────────────────┤
  │  Long-term Memory 摘要（压缩后）             │  ← 按需注入
  ├────────────────────────────────────────────┤
  │  历史摘要（超过 N 轮后压缩）                 │  ← 自动压缩
  ├────────────────────────────────────────────┤
  │  最近 K 轮完整消息（含 tool_calls/results）  │  ← 滑动窗口
  ├────────────────────────────────────────────┤
  │  当前用户输入 + 附件                         │
  └────────────────────────────────────────────┘
```

**这是解决 P0-1 + P0-6 的核心。滑动窗口保留完整 tool 交互，
超出部分压成摘要。**

---

## 第六部分：双端架构

### 6.1 技术选型对比

| 方案 | 局域网 | 互联网 | NAT 穿透 | 实现成本 | 安全性 | 结论 |
|---|---|---|---|---|---|---|
| **HTTP 轮询** | ✅ | ❌ | ❌ | 低 | 中 | ❌ 不适合实时 |
| **TCP 裸Socket** | ✅ | ❌ | ❌ | 中 | 需自建 | ❌ 要自己处理重连/加密 |
| **WebSocket（自建中继）** | ✅ | ✅ | 需中继服务器 | 高 | 需自建 | ⚠️ 要维护服务器 |
| **Tailscale / tsnet** | ✅ | ✅ | ✅ 自动 | **低** | **WireGuard** | ✅ **推荐** |
| **WebRTC** | ✅ | ✅ | 需 STUN/TURN | 很高 | DTLS | ❌ 过度设计 |

### 6.2 推荐：Tailscale (tsnet) 优先 + 局域网直连降级

**为什么选 Tailscale**：

1. **`tailscale.com/tsnet` 把 Tailscale 嵌入 Go 程序**——但你是 Rust/Kotlin 栈，
   所以实际做法是：**两端各自运行 tailscaled（或用系统 Tailscale 客户端）**，
   应用只连本机 tailscaled 的 LocalAPI / 直接用 tailnet IP
2. **NAT 穿透自动化**：不用自建中继、不用配端口转发
3. **WireGuard 加密**：传输安全是内置的，不用自己实现
4. **ACL 做鉴权**：设备级的访问控制，比自建 token 体系可靠
5. **公网/局域网统一**：同一个 tailnet IP 在两种网络下都能通

**风险与缓解**：

| 风险 | 缓解 |
|---|---|
| 需要 Tailscale 账号（免费 100 设备） | 演示账号提前准备；生产可用 Headscale 自建协调服务 |
| 用户需装 Tailscale 客户端 | 打包 tailscaled 随应用分发；或降级到局域网直连 |
| 首次配对稍复杂 | 提供扫码配对（复用当年设计的思路） |

### 6.3 通信架构设计

```
  LocalFile (Android)                          LocalMind (Windows)
  ┌────────────────────┐                      ┌────────────────────┐
  │  RemoteControl UI  │                      │  Command Handler   │
  └─────────┬──────────┘                      └─────────┬──────────┘
            │                                            │
  ┌─────────▼──────────┐                      ┌──────────▼─────────┐
  │  Command Client    │                      │  Command Server    │
  │  ├─ send(cmd)      │                      │  ├─ 校验签名        │
  │  ├─ subscribe(evt) │                      │  ├─ 权限网关        │
  │  └─ upload/download│                      │  └─ 转发给 Agent    │
  └─────────┬──────────┘                      └──────────┬─────────┘
            │                                            │
  ┌─────────▼────────────────────────────────────────────▼─────────┐
  │                    传输层（二选一，自动降级）                     │
  │  ① Tailscale: 100.x.y.z:port (WireGuard 加密，优先)             │
  │  ② 局域网直连: 192.168.x.x:port (mDNS 发现 + PSK 加密，降级)      │
  └─────────────────────────────────────────────────────────────────┘

  协议：WebSocket（在 Tailscale/局域网之上跑，不用自己做加密和重连）
  消息：JSON，复用 CommandEnvelope 思路（但要重写，旧的无法直接用）
```

### 6.4 Command / Event 模型

```jsonc
// Command: LocalFile → LocalMind
{
  "v": 1,
  "id": "uuid",              // 幂等键，用于去重与重试
  "type": "command",
  "action": "agent.run",     // agent.run | agent.cancel | fs.list | fs.read |
                             // fs.download | sys.status | task.pause
  "payload": { "text": "帮我整理桌面" },
  "from": "device-uuid-phone",
  "ts": 1757500000000,
  "sig": "hmac-sha256(...)"   // 用配对时协商的密钥签名
}

// Event: LocalMind → LocalFile
{
  "v": 1,
  "type": "event",
  "ref": "uuid",             // 对应 command.id
  "seq": 12,                 // 单调递增，用于断线重连补发
  "kind": "thinking",        // thinking | tool_call | tool_result |
                             // delta | done | error | confirm_required
  "payload": { "phase": "executing", "label": "正在执行：list_dir" },
  "ts": 1757500001000
}

// 需要用户确认时
{
  "type": "event",
  "kind": "confirm_required",
  "payload": {
    "confirm_id": "uuid",
    "risk": "high",
    "action": "delete_path",
    "reason": "即将删除 C:/Users/xxx/Desktop/old（共 23 个文件）",
    "expires_at": 1757500030000
  }
}
```

**关键设计点**：

- **`seq` 单调递增** → 断线重连后按 seq 补发，解决"断线期间发生了什么"
- **`confirm_required`** → 高风险操作不阻塞等待，而是发事件让手机端弹确认
- **`id` 幂等** → 网络重试不会重复执行

### 6.5 职责边界（回答"谁该干什么"）

| 能力 | 归属 | 理由 |
|---|---|---|
| Agent 循环 / 推理 | **LocalMind** | 需要工具、需要算力、需要访问文件系统 |
| 文件**生成**（docx/pptx/xlsx/pdf） | **两端各自本地完成** | 手机端已实现且很好，不需要过 Windows |
| 文件**查看/摘要/问答** | **两端各自本地完成** | 隐私 + 离线可用 |
| 操作 Windows 文件系统 | **仅 LocalMind** | 手机碰不到 Windows 磁盘 |
| 远程下发指令 | LocalFile → LocalMind | — |
| 长任务状态/日志 | LocalMind 产生，LocalFile 订阅 | — |
| 会话数据主副本 | **LocalMind**（SQLite） | 手机是视图，不是数据源 |
| 手机端本地文件历史 | **LocalFile**（Room） | 已有，保持 |

**数据归属原则**：
- **只存在 Windows**：Agent 执行日志、工具调用记录、系统路径信息
- **可同步到 Android**：会话列表摘要、任务状态、生成的文件（按需下载）
- **绝不上传**：文件内容本身（除非用户明确点"发到手机"）

---

## 第七部分：安全架构

### 7.1 权限模型（回答评委第 2 点）

#### 先纠正一个事实

我上一轮给你的 PPT 论据——**"7 个工具里没有 delete，所以不会乱删"**——
**这个论据不成立，请不要用。**

事实是：
- Python Agent 注册的 7 个工具确实不含 delete ✅（这部分没错）
- **但 Rust 侧 `tools.rs` 有 `delete_path` 和 `run_command`，
  且都注册进了 invoke_handler**（`main.rs:76, 78`）

如果评委追问"那 Tauri 命令面呢"，这个论据会被当场击穿。

#### 正确的权限表述

**"Agent 的能力边界由 Python Tool Registry 决定，不含删除。
但应用的 Tauri 命令面比 Agent 更大，这层目前缺少保护——
这正是我们这轮要修的。"**

这样讲既诚实，又显示你清楚自己的系统。

#### 最小权限模型设计

```
  风险分级                处理方式                       需要确认？
  ─────────────────────────────────────────────────────────────
  L0 只读              自动执行                         否
     read_file / list_dir / get_common_paths /
     read_clipboard / check_ollama

  L1 受限写入           自动执行（限 workspace）          否
     write_file / create_doc / append_file
     （限：Desktop / Documents / Downloads / 用户指定目录）

  L2 移动/重命名        自动执行（限 workspace）          否
     move_file / rename_path
     （自动记录操作日志，可 undo）

  L3 执行程序           自动执行 + 白名单                 首次确认
     open_app（限白名单程序）
     run_command（★ 默认禁用，需显式开启 + 每条确认）

  L4 破坏性操作         拦截 + 显式确认                   是（每次）
     delete_path（移入回收站，不真删）
     批量操作（> N 个文件）

  L5 禁止               直接拒绝                         —
     系统目录（C:/Windows, C:/Program Files）
     其他用户目录
     磁盘根目录
     网络/UNC 路径
```

**关键原则**：

1. **默认自动，例外确认**——不是"所有都确认"
   - 只有 L3（首次）、L4 需要确认，其余全自动
   - 这样 Agent 的自主性基本不受影响

2. **删除 = 移入回收站，不真删**
   - Windows 有 `SHFileOperation` 的回收站 API
   - 用户可自行恢复，比"备份"轻量且符合直觉

3. **`run_command` 默认关闭**
   - 它是 P0 风险，但也是能力天花板
   - 设计成：设置里显式开启 → 每条命令弹确认 → 日志记录

### 7.2 文件安全

```
  任意路径输入
       │
       ▼
  ┌─────────────────────────────┐
  │ ① 规范化                     │  canonicalize：消解 ../ 和符号链接
  └─────────────┬───────────────┘
                ▼
  ┌─────────────────────────────┐
  │ ② 拒绝清单（L5）             │  C:/Windows, C:/Program Files,
  │                             │  其他用户目录, 磁盘根, UNC
  │    → 命中即拒绝              │
  └─────────────┬───────────────┘
                ▼
  ┌─────────────────────────────┐
  │ ③ Workspace 边界             │  默认 Desktop/Documents/Downloads
  │    → 越界则需用户授权         │  用户可添加"受信任目录"
  └─────────────┬───────────────┘
                ▼
  ┌─────────────────────────────┐
  │ ④ 批量保护                   │  单次影响 > 10 个文件 → 升级为 L4
  └─────────────┬───────────────┘
                ▼
  ┌─────────────────────────────┐
  │ ⑤ 执行前快照（写/删/移）      │  记录原路径 + 大小 + mtime + hash(小文件)
  └─────────────┬───────────────┘
                ▼
  ┌─────────────────────────────┐
  │ ⑥ 审计日志（SQLite）          │  ts / action / path / result / session_id
  └─────────────┬───────────────┘
                ▼
              执行
```

**关于 Undo/Rollback 的务实建议**：

不要做完整 rollback 系统（过度设计）。做**两级**：

- **廉价版（推荐先做）**：删除走回收站；写/移操作记录快照，
  提供"撤销上一步"（只支持最近 1 步，或最近 10 步的列表）
- **昂贵版（暂缓）**：完整快照 + 任意时间点回滚 → 这会变成备份软件，不做

**是否需要沙箱？** → **不需要**。
沙箱会杀死 Agent 的核心价值（操作真实文件系统）。
正确做法是上面的"边界 + 日志 + 回收站"组合。

### 7.3 用户确认的触发原则

**反模式**：每个文件操作都弹窗 → Agent 失去自主性，用户疲劳后无脑点确认

**正确模式**：

| 场景 | 行为 |
|---|---|
| 在 workspace 内写文件 | 静默执行，UI 显示"已写入 xxx" |
| 在 workspace 内移动文件 | 静默执行，UI 提示"已移动 3 个文件 · 撤销" |
| 删除文件 | 移入回收站，UI 提示"已删除 xxx · 撤销" |
| 批量 > 10 个 | 暂停，请求确认（一次确认，不是逐个） |
| 越出 workspace | 请求确认，可"始终允许此目录" |
| run_command | 每条确认（若已开启） |

---

## 第八部分：Memory 架构

### 8.1 现状：Memory = 零

先说清楚起点：

- `StorageManager` 是内存 Vec（P0-2）→ **连会话都存不下**
- 没有任何长期记忆机制
- 工具调用历史不落盘（P0-1）→ **连短期记忆都是断的**

所以 Memory 系统要**从地基开始建**，不是"加个自动更新文档"那么简单。

### 8.2 三层 Memory 模型

```
  ┌──────────────────────────────────────────────────────┐
  │  Working Memory（工作记忆）                            │
  │  · 当前任务的 tool_calls / tool_results                │
  │  · 存活期：单次 Agent 运行                             │
  │  · 存储：Python 进程内 + 落盘到 tool_calls 表           │
  │  · 作用：让 Agent 在同一任务内连贯思考                  │
  ├──────────────────────────────────────────────────────┤
  │  Session Memory（会话记忆）                            │
  │  · 一个会话的完整消息 + 工具历史                        │
  │  · 存活期：会话生命周期（跨重启，因为落 SQLite）         │
  │  · 存储：messages / tool_calls 表                      │
  │  · 作用：解决 P0-1，让多轮对话连贯                      │
  ├──────────────────────────────────────────────────────┤
  │  Long-term Memory（长期记忆）                          │
  │  · 跨会话的用户偏好、环境事实、项目约定                  │
  │  · 存活期：永久                                        │
  │  · 存储：memory 表（key / value / confidence / source） │
  │  · 作用：让 Agent 记住"用户是内大大二学生"这类事实       │
  └──────────────────────────────────────────────────────┘
```

### 8.3 Memory 生命周期

```
  Capture（捕获）
      │  从哪捕获？
      │  ├─ 用户显式说："记住我喜欢..."
      │  ├─ Agent 执行结果（成功/失败的模式）
      │  └─ 环境事实（桌面路径、已装软件、模型列表）
      ▼
  Filter（过滤）
      │  谁判断值得存？
      │  ├─ 规则过滤：长度、是否含敏感词、是否重复
      │  └─ 模型判断：一次廉价的 LLM 调用，输出
      │     {"worth": true/false, "category": "...", "text": "..."}
      │     ⚠️ 只在会话结束时做一次，不每条消息都做
      ▼
  Store（存储）
      │  memory 表：
      │  id / category / content / confidence / source_session_id
      │  / created_at / updated_at / access_count / embedding(可选)
      ▼
  Retrieve（检索）
      │  什么时候注入？
      │  ├─ 关键词匹配（简单、无依赖、先做这个）
      │  └─ 语义检索（可选，需要 embedding，暂缓）
      │  注入多少？→ 按 token 预算截断，只注入 top-K
      ▼
  Update（更新）
      │  冲突怎么办？
      │  └─ 同 category + 高相似度 → 覆盖，保留 source 与更新时间
      ▼
  Compress（压缩）
      │  什么时候压？
      │  └─ 会话轮次 > N → 调用 LLM 生成摘要，替换早期消息
      ▼
  Cleanup（清理）
       └─ 过期（> 90 天未访问）/ 低置信度 / 用户手动删除
```

### 8.4 防污染的四条硬规则

| 风险 | 防护 |
|---|---|
| 错误信息写入 Memory | 只从**成功的工具执行**和**用户显式陈述**中提取；失败记录只进日志不进 Memory |
| 垃圾信息填满 | Filter 阶段用规则 + 模型双重过滤；单条 Memory 有长度上限 |
| 无限膨胀 | 每个 category 有条目上限（如 50 条）；定期 Cleanup |
| 反过来污染 Context | 注入时按 token 预算截断；只在系统提示末尾追加，不插入中间 |

### 8.5 关于"自动更新记忆文档"

你提到的第 5 点改进是"新增自动更新记忆文档功能"。我的建议：

**不要自动往 Markdown 追加内容。**

原因：
- Markdown 无法去重、无法检索、无法过期、无法按 token 预算截断
- 追加式文档必然膨胀成垃圾堆

**正确做法**：结构化存 SQLite（见上），然后**按需导出**为 Markdown 给用户看。
即：Memory 的权威存储是数据库，Markdown 只是**只读视图**。

---

## 第九部分：Agent Harness 改造方案

### 9.1 现有 Harness 的 18 个模块体检

| # | 模块 | 现状 | 评级 |
|---|---|---|---|
| 1 | Model Adapter | `OpenAIChatModel` / `OllamaModel` 走统一协议 | ✅ **好，保持** |
| 2 | Prompt Assembly | `build_system_prompt` 注入桌面路径、工具约束 | ✅ 尚可，但工具说明硬编码在字符串里 |
| 3 | Context Management | **全量发送，无窗口** | ❌ **P0** |
| 4 | Agent Loop | Pydantic AI `run_stream_events`，无显式 max_iterations | ⚠️ 缺上限保护 |
| 5 | Tool Registry | **三处定义** | ❌ **P0** |
| 6 | Tool Router | 无独立路由，Pydantic AI 内建 | ⚠️ 可接受 |
| 7 | Tool Executor | Python 直接执行，有 `_wrap` 异常包装 | ✅ 尚可 |
| 8 | State Management | 无显式状态机，靠 `run_stream_events` 隐式流转 | ⚠️ 不可观测 |
| 9 | Task Management | 无 | ❌ 缺 |
| 10 | Error Handling | `_wrap` + `retries=1` + 错误回灌 | ✅ **设计正确** |
| 11 | Retry | `retries=1`（框架）+ 文本兜底（自研） | ✅ 尚可 |
| 12 | Cancellation | `AbortController` → fetch abort → BrokenPipe 静默 | ✅ **正确** |
| 13 | Timeout | 仅 `create_doc` 有 120s，其余无 | ⚠️ 缺 |
| 14 | Streaming | SSE + `tool_call_id` 配对 | ✅ **好** |
| 15 | Memory | **无** | ❌ **P0** |
| 16 | Logging | `%TEMP%/localmind-agent.log` 简易日志 | ⚠️ 无结构化、无审计 |
| 17 | Permission | **无** | ❌ **P0** |
| 18 | Security | 有 token 鉴权 + 只绑 127.0.0.1 ✅；但 Tauri 命令面无保护 ❌ | ⚠️ 半好半坏 |

### 9.2 改造清单（按优先级）

**① Context Manager（新增模块，解决 P0-6 + P0-1）**

```
  build_context(system, history, current_input, attachments, budget)
      │
      ├─ 固定部分：system prompt + 工具 schema  → 计算 tokens
      ├─ 当前输入：user message + attachments   → 计算 tokens
      ├─ 剩余预算 = 模型上限 × 0.7 - 固定 - 当前
      │
      ├─ 从最近往回取历史消息
      │     └─ 每条消息完整保留其 tool_calls + tool_results（原子性）
      │
      ├─ 超出预算的早期消息 → 调 LLM 生成摘要
      │     └─ 摘要缓存（避免每轮重复生成）
      │
      └─ 返回 [system, summary?, recent_messages..., current]
```

**关键点**：tool_call 和它的 tool_result **必须成对保留或成对丢弃**。
拆开会让模型看到"调用了工具但没有结果"，是幻觉的常见诱因。

**② Tool Registry 中心化（解决 P0-7）**

```
  在 Python 侧定义工具时，同时声明：
      {
        "name": "write_file",
        "risk_level": "L1",
        "workspace_only": true,
        "needs_confirm": false,
        "description": "...",      ← 给模型看
        "display_name": "写文件",   ← 给 UI 看
      }
  用脚本导出为 JSON，供前端/手机端/权限网关消费。
```

**③ Permission Gateway（新增，解决 P0-3/4/5）**

```
  execute_tool(name, args)
      │
      ├─ 1. 查 Tool Registry 拿 risk_level
      ├─ 2. 路径规范化 + 拒绝清单 + workspace 边界
      ├─ 3. 批量检测（影响文件数 > N → 升级风险）
      ├─ 4. L4/L3(首次) → 发 confirm_required 事件，挂起等待
      ├─ 5. 执行前快照 → 审计日志
      ├─ 6. 执行
      └─ 7. 记录结果 → 审计日志
```

**④ Session/Memory 持久化（解决 P0-1 + P0-2）**

```
  SQLite 表设计：
    sessions(id, title, mode, created_at, updated_at)
    messages(id, session_id, role, content, model_label, created_at, token_count)
    tool_calls(id, message_id, tool_call_id, name, arguments,
               output, success, created_at)      ← 关键：补上这一张表
    memory(id, category, content, confidence, source_session_id,
           created_at, updated_at, access_count)
    audit_log(id, ts, session_id, action, path, risk_level,
              result, undo_data)                  ← 支持撤销
```

**`tool_calls` 表是 P0-1 的解药。** 有了它，下一轮才能把
"我调用过什么、返回了什么"重新拼回上下文。

**⑤ 状态机显式化（解决"不可观测"）**

```
  IDLE → PLANNING → EXECUTING → OBSERVING → (回到 PLANNING | DONE)
            │            │
            └────────────┴──→ FAILED → (RETRY | ABORTED)
                              │
                              └→ WAITING_CONFIRM → EXECUTING
```

好处：每步可观测、可取消、可超时、可测试。
现在靠 `run_stream_events` 隐式流转，出问题只能看日志猜。

---

## 第十部分：实施路线图

### Phase 0 — 止血（安全与数据）

| 项目 | 内容 |
|---|---|
| **做什么** | ① 从 `invoke_handler` 移除 `run_command`、`delete_path`、`append_file`、`rename_path`、`ollama_chat`<br>② `StorageManager` 换 SQLite<br>③ `get_app_info` 的 device_id 持久化 |
| **为什么先做** | 这是**风险**不是功能。删掉 5 个高危死命令，攻击面立刻下降；持久化是后面所有功能的地基 |
| **依赖** | 无 |
| **解决什么** | P0-2、P0-3、P0-4、P0-5、P1-5 |
| **风险** | 移除命令可能导致某处 UI 报错 → 已确认这些命令零调用点，风险低 |
| **如何测试** | ① 全局 grep 确认无调用 ② 编译运行，走一遍完整对话 ③ 重启应用确认会话还在 |

### Phase 1 — 修记忆（Agent 性能的主战场）

| 项目 | 内容 |
|---|---|
| **做什么** | ① 新增 `tool_calls` 表，落盘每次工具调用<br>② 重建上下文时把 tool_calls/results 拼回 message history<br>③ Context 窗口管理（滑动窗口 + 摘要压缩） |
| **为什么先做** | **这是"Agent 性能差"的最大单一成因**。修完很可能不换模型就有明显改善 |
| **依赖** | Phase 0（需要 SQLite） |
| **解决什么** | P0-1、P0-6、P1-7 |
| **风险** | 摘要压缩会增加一次 LLM 调用 + 延迟 → 用缓存缓解，且只在超阈值时触发 |
| **如何测试** | 见第十一部分 A/B 测试 |

### Phase 2 — 立权限（能力解锁的前提）

| 项目 | 内容 |
|---|---|
| **做什么** | ① Tool Registry 中心化（含 risk_level 声明）<br>② Permission Gateway（路径边界 + 拒绝清单 + 批量保护）<br>③ 删除走回收站 + audit_log + 单步 undo<br>④ `run_command` 改为"默认关闭 + 显式开启 + 逐条确认"后**重新引入** |
| **为什么先做** | 只有建立权限体系，才敢把 `run_command` 这种高价值能力开放给 Agent。**先修锁，再开锁** |
| **依赖** | Phase 0、Phase 1 |
| **解决什么** | P0-3、P0-4、P0-5、P0-7、P1-1、P1-9 |
| **风险** | 权限过严会削弱 Agent → 用"默认自动、例外确认"原则控制 |
| **如何测试** | 构造恶意路径（`../../Windows/System32`）、系统目录、批量删除；验证拦截率 |

### Phase 3 — Memory 系统

| 项目 | 内容 |
|---|---|
| **做什么** | ① memory 表 + Capture/Filter/Store/Retrieve 链路<br>② 会话结束时的摘要与记忆提取（一次 LLM 调用）<br>③ 按需注入（关键词匹配 + token 预算）<br>④ 导出 Markdown 只读视图 |
| **为什么现在做** | 需要 Phase 1 的持久化基础；且此时 Agent 已有连贯记忆，加长期记忆才有意义 |
| **依赖** | Phase 1 |
| **解决什么** | 提示词第六部分 |
| **风险** | 记忆污染 → 用 8.4 的四条硬规则防护 |
| **如何测试** | 新开会话，验证 Agent 能引用 3 个会话前的事实；人工检查 memory 表无垃圾 |

### Phase 4 — 双端互联

| 项目 | 内容 |
|---|---|
| **做什么** | ① Tailscale/局域网传输层<br>② Command/Event 协议（含 seq、幂等、confirm_required）<br>③ 配对鉴权<br>④ LocalFile 的 RemoteControl UI（任务状态/日志/取消/确认/文件下载） |
| **为什么最后做** | 依赖前面全部：需要权限网关（否则远程执行很危险）、需要状态机（否则无法同步任务状态）、需要持久化（否则日志无处存） |
| **依赖** | Phase 0-3 |
| **解决什么** | 提示词第四、五部分 |
| **风险** | Tailscale 账号依赖；配对流程复杂度 → 提供局域网降级方案 |
| **如何测试** | 手机下发"整理桌面"→ 观察 Windows 执行 + 手机收到实时事件 + 断网重连补发 |

### Phase 5 — 清理与扩展

| 项目 | 内容 |
|---|---|
| **做什么** | ① 删除 llama.cpp（Windows + Android）死代码<br>② 删除 shared-contract 与 wssClient.ts 旧契约<br>③ 统一 mock 实现（要么实现，要么删 UI）<br>④ 模型显示名统一为真实模型名 |
| **为什么最后** | 不影响功能，但能显著降低认知负担和构建时间 |
| **依赖** | 无（可与前面并行做部分） |
| **解决什么** | P1-2、P1-3、P1-4、P1-6、P1-10、P2-1 |

---

## 第十一部分：Benchmark / 验证方案

### 11.1 核心目标：拆分 Model 与 Harness

**核心方法**：控制变量。

```
                    Harness v1 (当前)    Harness v2 (改造后)
  Model A (DeepSeek)      A1                   A2
  Model B (qwen2.5:7b)    B1                   B2

  · A2 - A1 = Harness 改进带来的增益（模型不变）
  · B2 - B1 = Harness 改进带来的增益（弱模型下）
  · A1 - B1 = 模型带来的差异（Harness 不变）
```

**如果 A2 明显优于 A1，但 A1 - B1 不大** →
说明问题主要在 Harness，不在模型。**这是我基于代码判断的预期结果。**

### 11.2 测试任务集（建议 12 条，覆盖能力维度）

| # | 任务 | 考察点 | 成功标准 |
|---|---|---|---|
| T1 | "帮我整理桌面" | 多步规划 + 批量操作 | 文件按类型归类，无丢失，无越界 |
| T2 | "写一个贪吃蛇 HTML 游戏并打开" | 工具链串联 + 验证闭环 | 生成文件 + 浏览器打开 + 游戏可玩 |
| T3 | "读一下 D:/report.txt 并总结" | 文件读取 | 总结准确 |
| T4 | "做一个内大介绍的 PPT" | create_doc | 文件真实生成且可打开 |
| T5 | （接 T1）"刚才移动了哪些文件？" | **跨轮记忆** ⭐ | 能列出具体文件名 ← **当前必挂** |
| T6 | （接 T3）"第 2 段说了什么？" | **跨轮记忆** ⭐ | 准确引用 ← **当前必挂** |
| T7 | "把桌面所有 txt 合并成一个文件" | 多文件批处理 | 正确合并 |
| T8 | "删掉桌面的临时文件" | **删除安全** | 移入回收站 + 请求确认 |
| T9 | 长会话（20 轮后问第 1 轮的事） | **Context 管理** ⭐ | 仍能正确回答 ← **当前会超限或遗忘** |
| T10 | 故意给错路径 | 错误处理 | 优雅报错 + 自我修正，不崩溃 |
| T11 | "打开计算器" | open_app | 程序启动 |
| T12 | 上传大文件后追问细节 | 附件记忆 | 能引用附件内容 ← **当前必挂** |

**⭐ 标记的 T5/T6/T9/T12 是"当前必挂"的**——它们直接对应 P0-1 和 P0-6。
这四条在改造前后的对比，就是最有说服力的证据。

### 11.3 量化指标

| 指标 | 定义 | 采集方式 |
|---|---|---|
| **任务成功率** | 12 条任务中成功的比例（人工判定） | 人工评分表 |
| **平均轮次** | 完成任务平均需要几轮 | tool_calls 表统计 |
| **重复工具调用率** | 相同 (name, args) 的调用次数 / 总调用次数 | tool_calls 表 SQL 查询 ⭐ |
| **Token 消耗** | 每轮平均 input tokens | API 返回 usage |
| **首字延迟** | 用户发送到首个 token 的时间 | 前端埋点 |
| **错误恢复率** | 工具报错后最终成功的比例 | tool_calls 表 success 字段 |
| **跨轮正确率** | T5/T6/T9/T12 的正确率 | 人工评分 |

**"重复工具调用率"是最能反映 P0-1 的指标**——
Agent 忘记自己做过什么，就会反复调同一个工具。

### 11.4 回归测试（安全）

```
  安全回归套件（Phase 2 后必须全绿）：
    □ 写入 C:/Windows/test.txt        → 拒绝
    □ 删除 C:/Users/xxx/Desktop       → 拦截 + 确认
    □ 路径穿越 ../../Windows          → 拒绝
    □ 批量删除 20 个文件               → 升级确认
    □ 删除文件                        → 进入回收站（可恢复）
    □ 任何操作                         → audit_log 有记录
    □ 撤销上一步                       → 文件恢复
```

---

## 第十二部分：明确建议暂缓的事情

### 12.1 现在不要做

| 项 | 为什么暂缓 |
|---|---|
| **微服务架构** | 单机桌面应用，进程间通信用 HTTP on localhost 已经够。引入微服务只会增加调试难度 |
| **消息队列（Kafka/RabbitMQ）** | 通信量是"每秒几条指令"。SQLite + WebSocket 足够。MQ 是重量级误用 |
| **向量数据库** | Memory 起步阶段用关键词匹配 + SQLite 就够。等 memory 条目超过 1000 条再考虑 embedding |
| **多 Agent 编排** | 现在单 Agent 的基础问题（记忆、Context、权限）都没解决。加多 Agent 只会让问题指数级复杂 |
| **完整快照/任意时间点回滚** | 会演变成备份软件。用"回收站 + 单步 undo"覆盖 95% 场景 |
| **沙箱** | 会杀死 Agent 核心价值。用"边界 + 日志 + 回收站"替代 |
| **Web / Linux 第三端** | 先把双端打通、把 Harness 修好。第三端是 Phase 5 之后的事 |
| **llama.cpp 本地推理** | 已确认是死代码。Ollama 方案工作良好，不要重复投入（且本轮 Ollama/Qwen 不在范围内） |
| **复杂权限系统（RBAC/ABAC）** | 4 级风险分级（L0-L5）足够。上 RBAC 是过度设计 |
| **插件系统 / 动态工具加载** | 先让 7-12 个工具跑稳 |

### 12.2 一个特别提醒：不要重写

代码里确实有大量问题——mock、死代码、重复实现。但：

**`agent_server.py`（731 行）是整个项目最有价值的资产，它写得是对的。**

工具注册、错误回灌、文本兜底、SSE 流式、凭据隔离——这些设计都是正确的。
**要修的是它外围的断裂，不是它本身。**

我建议的方式是：
- Phase 0-3 全部是**加法 + 减法**（加持久化/权限/Context 管理，减死代码/危险命令）
- 唯一需要"改写"的是 `useStreamChat.ts` 的上下文组装逻辑和 `storage/mod.rs`
- `agent_server.py` 主体保持不动

---

## 附：无法从代码确定的事项

以下我在代码中找不到答案，**需要你确认**，我不会假设：

1. **"DeepSeek V4 Pro" 的真实来源**
   代码里 UI 到处显示 `Deepseek-V4-Pro`（`chat/mod.rs` ×3、`chatStore.ts` ×3、
   `useStreamChat.ts:60`、`common/mod.rs:73`），
   但实际 API 调用用的 `MODEL = "deepseek-chat"`（`DeepSeekConfig.kt:13`、
   `agent_server.py:588`）。
   **"V4 Pro" 是真实产品线名称，还是纯 UI 包装？** 这影响 PPT 措辞和答辩应答。

2. **Ollama 之外的离线方案是否曾有规划**
   `inference/mod.rs`（llama.cpp）是死代码，但它是"曾经想做本地推理但放弃"，
   还是"未来要接"？这决定 Phase 5 是删掉还是保留。

3. **RelayCloud 的教训**
   当年三组件方案被砍的**具体原因**是什么？
   （技术难度？维护成本？需求变化？）
   这决定双端互联时要避开哪些坑。

4. **LocalFile 的 Room 数据库现状**
   我看到有 `AppDatabase.kt` / 4 个 DAO / `Entities.kt`，
   但没逐一读表结构。如果已有会话持久化，**手机端的 Memory 起点比 Windows 端高**，
   值得先摸清。

5. **目标用户与使用场景**
   是自用 + 答辩演示，还是要分发给真实用户？
   这决定权限模型的严格程度（自用可以宽松，分发必须严格）。

---

*报告完 · 本轮未修改任何代码*
