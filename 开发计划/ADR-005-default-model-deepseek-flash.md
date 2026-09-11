# ADR-005：双端内置在线模型统一为 DeepSeek V4 Flash（官方 ID `deepseek-flash`）

- 状态：已接受
- 日期：2026-09-11
- 决策范围：LocalMind（前端 + Rust + Python Agent）、LocalFile（Android）
- 依赖决策：ADR-002 Relay-first（不受影响）

## 1. 背景

用户要求：双端"内置在线模型"统一为 V4 Flash 档，并且不再出现界面名字与实际调用不一致的情况。

2026-09-11 用项目内置 key 实测官方 API（`https://api.deepseek.com`）：

| 请求的 model | 结果 |
|---|---|
| `GET /models` | 只返回两个 ID：`deepseek-flash`、`deepseek-v4-pro` |
| `deepseek-flash` | 200，响应 `model` = `deepseek-flash` |
| `deepseek-v4-flash` | 200，但响应 `model` 被归一为 `deepseek-flash`（别名） |
| `deepseek-chat` / `deepseek-reasoner` | 200，响应 `model` 同样被归一为 `deepseek-flash`（历史别名） |
| 不存在的名字（如 `totally-bogus-model-xyz`） | 400 `invalid_request_error`，报错文本明确列出 supported names 只有 `deepseek-flash`、`deepseek-v4-pro` |

结论：用户口中的 "deepseek-v4-flash" 就是官方 `deepseek-flash` 的别名，两者调用的是同一个模型。

## 2. 决策

1. 双端在线模式统一使用**官方 ID `deepseek-flash`**；用户可见展示名统一为 **DeepSeek V4 Flash**。
2. 代码里不写 `deepseek-v4-flash` 别名：它与官方 ID 等价，但要额外承担"别名哪天被摘掉、安装包突然不可用"的风险。
3. 唯一事实来源收敛到三处，改模型只改这三处：
   - Windows 前端：`localmind/src/config/models.ts`（`ONLINE_MODEL_ID` / `ONLINE_MODEL_LABEL`）
   - Windows Agent：`localmind/scripts/agent_server.py` 的 `DEFAULT_ONLINE_MODEL`
   - Android：`localfile/.../common/DeepSeekConfig.MODEL` 与 `assets/config.json` 的 `default_model` / `model_display_name`
4. 清理散落在 Rust `chat` 模块与 UI 文案里的硬编码显示名（原为 `Deepseek-V4-Pro`），避免"界面写 Pro、实际调 Flash"的口径不一致。
5. 前端在线请求显式携带 `ONLINE_MODEL_ID`，不再依赖 Agent 端默认值兜底。

## 3. 后果

- 评审时界面显示的模型名与实际调用完全一致，不会再出现名字对不上的问题。
- 离线模式（Ollama）与本决策无关，仍由用户选择本地模型。
- 若 DeepSeek 后续改名或下线该 ID，只需改上述常量并重新打包双端。
- 旧版客户端仍可运行：历史别名（`deepseek-chat` 等）会被服务端归一，不影响已发出的安装包。
