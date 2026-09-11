/**
 * 在线模式（DeepSeek 云端）内置模型 —— 双端唯一事实来源。
 *
 * 决策与依据见 `开发计划/ADR-005-default-model-deepseek-flash.md`：
 * - 官方模型 ID 是 `deepseek-flash`（`https://api.deepseek.com/models` 只返回
 *   `deepseek-flash` 与 `deepseek-v4-pro`）；
 * - `deepseek-v4-flash` 目前是它仍可用的别名，但 API 返回的 model 字段同样是
 *   `deepseek-flash`，因此代码固定用官方 ID，避免别名下线后安装包突然不可用；
 * - 切换模型只需改这里（Windows）+ `localmind/scripts/agent_server.py`
 *   （Agent 侧默认值）+ Android `DeepSeekConfig.MODEL`。
 */
export const ONLINE_MODEL_ID = 'deepseek-flash';
export const ONLINE_MODEL_LABEL = 'DeepSeek V4 Flash';
export const OFFLINE_MODEL_LABEL = '本地模型';
