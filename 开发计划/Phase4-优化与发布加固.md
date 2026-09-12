# Phase 4：优化与发布加固

> 日期：2026-09-12
> 范围：P0 全部 + P1 全部 + P2（#10 #12 #13 #14）+ P3（版本号）
> 原则：只优化，不改架构；每项完成后验证编译再进入下一项

---

## 任务清单

### P0：必须做

#### #1 高危操作确认 UI
- **现状**：`run_command` / `delete_path` 直接执行，无确认
- **目标**：L3/L4 工具执行前，前端弹窗显示完整命令文本，用户点"确认执行"或"拒绝"
- **实现**：
  - Python `agent_server.py`：`run_command` 和 `delete_path` 的工具包装器在执行前，通过 SSE 发送 `{"type":"confirm","tool":"run_command","args":{...},"id":"xxx"}` 事件
  - 前端 `useStreamChat.ts`：收到 `confirm` 事件 → 阻塞等待 → 用户点击后通过 fetch POST `/agent/confirm` 发回确认/拒绝
  - Python `agent_server.py`：新增 `/agent/confirm` 端点，用 `threading.Event` 等待确认结果
  - 确认 UI：在聊天区渲染一个可交互的确认卡片（显示工具名 + 参数 + 确认/拒绝按钮）
- **验收**：Agent 调用 `run_command` 时暂停，用户确认后才执行，拒绝则返回错误给 Agent

#### #2 审计日志写入
- **现状**：`audit_log` 表已建但从未写入
- **目标**：L2/L3/L4 工具每次执行都写入审计记录
- **实现**：
  - Python `build_tools` 中的 `_wrap` 函数：在工具执行后，将 (tool_name, args, success, output, timestamp) 通过 SSE 发送给前端
  - 前端收到后调用 Tauri `save_audit_log` 命令写入 SQLite
  - Rust `storage/mod.rs`：新增 `save_audit_log` 方法
  - Rust `chat_api.rs`：新增 `save_audit_log` Tauri 命令
- **验收**：每次文件操作/命令执行后，`audit_log` 表新增一条记录

#### #3 CONTEXT.md 修正
- 修正"Agent 工具 7 个" → 9 个
- 删除 ADR-004 安全红线中"run_command 永久禁止"的矛盾描述
- 更新 Agent 工具列表

#### #4 死代码清理
- 删除 `localmind/src/api/wssClient.ts`（已被 Rust relay_wss.rs 替代）
- 删除 `localmind/src/api/gateway.ts`（已被 Agent 代理替代）
- 确认无 import 引用后删除

---

### P1：应该做

#### #5 工具调用 UI 美化
- **现状**：工具调用以 `[调用工具:name] {json}` 纯文本嵌入消息
- **目标**：前端解析工具标记 → 折叠式卡片展示
- **实现**：
  - 新增 `ToolCallCard` 组件：工具名 + 参数摘要 + 成功/失败图标 + 可展开完整输出
  - `ChatPage.tsx` 渲染消息时，检测 `[调用工具:...]` 标记 → 替换为 `ToolCallCard`
  - 工具结果中的 JSON 参数格式化为可读表格
- **验收**：消息中的工具调用以卡片形式展示，不再显示原始标记文本

#### #6 用户记忆管理界面
- **目标**：设置页新增"记忆管理"区域
- **实现**：
  - SettingsPage 新增 section：调用 `get_memories` 展示列表
  - 每条记忆显示：category 标签 + content + 置信度 + 删除按钮
  - 搜索框：调用 `search_memories` 按关键词过滤
- **验收**：用户可以查看、搜索、删除长期记忆

#### #7 远程控制 Windows 端通知
- **目标**：远程命令到达时弹 Windows 系统通知
- **实现**：
  - `relayManager.ts`：收到 `relay-command` 时调用 `tauri-plugin-notification` 的 `requestPermission` + `sendNotification`
  - 通知内容："收到远程命令：{intent_text}"，点击打开应用
- **验收**：远程命令到达时 Windows 右下角弹通知气泡

#### #8 远程控制结果 Android 端展示优化
- **目标**：结果区域支持滚动和长文本展示
- **实现**：
  - `RemoteControlScreen.kt`：结果 Card 内用 `SelectionContainer` + `VerticalScrollbar` 包裹
  - 长结果截断 + "展开全部"按钮
  - 复制按钮（复制到剪贴板）
- **验收**：长结果可滚动查看、可复制

#### #9 API Key 加密存储
- **目标**：API Key 不再明文存储
- **实现**：
  - Windows：用 Windows DPAPI（`CryptProtectData` / `CryptUnprotectData`）加密 key 后存 SQLite
  - Android：用 `EncryptedSharedPreferences` 替代普通 DataStore 存储 gatewayToken
  - 读取时自动解密，对上层透明
- **验收**：直接查看 SQLite 文件 / DataStore 文件看不到明文 key

---

### P2：锦上添花

#### #10 Agent 自动确认机制
- **目标**：低风险工具（read_file、list_dir、read_clipboard）不需要确认直接执行；中风险（write_file、move_file、create_doc、open_app）显示操作意图但不暂停；高风险（run_command、delete_path）必须确认
- **实现**：
  - `ToolSpec` 增加 `confirmation_required: bool` 字段
  - Python `_wrap` 中根据 `confirmation_required` 决定是否发送 `confirm` 事件
  - L0/L1 工具：直接执行
  - L2 工具：发送 `info` 事件（前端显示为灰色提示）但不阻塞
  - L3/L4 工具：发送 `confirm` 事件并阻塞等待
- **验收**：read_file 等无感知执行；run_command 弹确认；write_file 显示提示但继续

#### #12 上下文窗口优化
- **目标**：token 估算更精确
- **实现**：
  - Python `agent_server.py`：用 `tiktoken`（cl100k_base 编码器）精确计算 token
  - fallback：tiktoken 不可用时使用字符估算（当前逻辑）
  - `CONTEXT_BUDGET_TOKENS` 改为从模型配置读取（DeepSeek flash = 65536）
- **验收**：长对话不再因 token 估算误差导致过早截断或溢出

#### #13 Summary 生成改为 AI 辅助
- **目标**：每轮结束后用模型生成结构化摘要
- **实现**：
  - 前端：Turn 完成后，额外发送一个轻量请求（非流式）给 Agent："请用一句话总结本轮对话的关键事实和结果"
  - Agent 返回的摘要存入 `session_summaries`
  - 非阻塞：摘要请求与 UI 更新并行，不阻塞用户继续输入
  - 如果摘要请求失败，fallback 到截取前200字（当前逻辑）
- **验收**：摘要质量明显优于截取，且不影响响应速度

#### #14 Relay WSS 断线重连后拉取未处理命令
- **目标**：重连后自动拉取离线期间的命令
- **实现**：
  - Rust `relay_wss.rs`：重连成功后发送一个 `state` 信封请求未处理命令
  - 或者：前端在 `relay-wss-connected` 事件中调用一个新的 Tauri 命令 `fetch_pending_relay_commands`
  - Relay 侧：已有 `enqueue_message` 机制，确保离线消息不丢
- **验收**：断网后恢复，之前发的远程命令仍能被接收和执行

---

### P3：版本号

#### #15 版本号升级
- `localmind/Cargo.toml`：`0.1.0` → `0.3.0`
- `localmind/package.json`：`0.1.0` → `0.3.0`
- `localfile/app/build.gradle.kts`：`versionName "1.0.0"` → `"0.3.0"`
- `CONTEXT.md`：更新版本号
- `开发计划/开发路线图.md`：更新 Phase 表格

---

## 执行顺序

1. #3 CONTEXT.md 修正（5 分钟）
2. #4 死代码清理（5 分钟）
3. #1 高危操作确认 UI（2 小时）
4. #2 审计日志写入（1 小时）
5. #10 Agent 自动确认机制（2 小时，与 #1 合并实施）
6. #5 工具调用 UI 美化（3 小时）
7. #7 Windows 通知（30 分钟）
8. #6 记忆管理界面（2 小时）
9. #8 Android 结果展示优化（1 小时）
10. #9 API Key 加密存储（2 小时）
11. #12 上下文窗口优化（1 小时）
12. #13 AI 辅助摘要（1 小时）
13. #14 WSS 断线重连拉取（1 小时）
14. #15 版本号升级 + 重新打包（30 分钟）

预估总工时：约 16 小时
