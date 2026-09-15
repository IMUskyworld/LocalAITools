# LocalAITools 项目长期记忆

## 关键路径

| 用途 | 路径 | 备注 |
|---|---|---|
| 演示 PPT 存放目录 | `C:\暑期开发ppt` | 用户于 2026-09-07 指定，存放暑期开发演示用 PPT |

## 演示相关事实（务必以此为准，勿照开发计划文档讲）

- `开发计划/README.md` 与 `开发计划/交付汇总说明.md` 描述的是**从未落地的三组件方案**
  （RelayCloud 云端中继、手机远程控制电脑、扫码配对、P2P 通道、new-api 网关）。
  **这些功能在代码中已删除，不可演示，PPT 中不要出现。**
- 实际可演示能力清单与注意事项见 `.workbuddy/memory/2026-09-07.md`。

## ⭐ 评审反馈待办（2026-09-09 汇报后，已与王文涛 2026-09-10 对齐）

详细对话与 5 点改进清单见 `.workbuddy/memory/2026-09-10.md`（待写）。
**当前共识（5 → 3）**：

| # | 原始要求 | 当前状态 | 备注 |
|---|---|---|---|
| ~~①~~ | ~~Ollama + qwen2.5:7b 内置到 LocalMind，离线点开即用~~ | ❌ **已取消** | 工程难度高（GGUF 4~5GB 体积 + GPU 后端），且用户已经手动装得不错。王文涛：「这俩其实无所谓」 |
| ~~②~~ | ~~同上（塞 Qwen）~~ | ❌ **同上取消** | — |
| ③ | PPT 加 Agent 权限模型（为何高权限 / 如何拥有 / 如何防误删） | ⚠️ **论据已推翻，需重写** | ❌ **不要用「7 个工具没有 delete」这个说法**——那只是 Python Agent 的工具面；Rust `tools.rs` 另有 `delete_path` + `run_command` 且已注册进 invoke_handler（`main.rs:76,78`）。详见下方「权限论据纠正」 |
| ④ | **双端互联**（手机 ↔ 电脑） | ⭐ **最高优先级**，新功能开发 | 公网方案难搞，**王文涛推荐 Tailscale**：Go 的 `tailscale.com/tsnet` 让两端以 mesh VPN 节点互通，WireGuard 加密、不暴露公网、不用自建中继。**注**：旧三组件方案（RelayCloud 云端中继/P2P/扫码配对）代码已删，**重新立项**——新方案本质是把"中继"换成"Tailscale 协调服务器"，两端业务代码同 LAN 一样写 |
| ⑤ | **新增：自动更新记忆文档** | 待办，新出现 | 用户口述需求，细节待定（是给 Agent 的 session summary 落盘？还是项目文档随代码自动更新？） |
| ＋ | **LocalMind Agent 能力加强** | 待办，用户补充 | 用户原话：「我的 LocaMind 的 Agent 能力太弱了，需要加强」。具体方向待问（工具集扩展？长任务规划？持久记忆？） |

### ⚠️ 权限论据纠正（2026-09-10 审计后推翻旧结论）

**错误论据（勿再用）**：「7 个工具里没有 delete，所以 Agent 不会乱删文件」
- 这只对 Python `agent_server.py` 注册的 7 个工具成立
- Rust `tools.rs` 有 **12 个 Tauri command**，含 `delete_path`（remove_dir_all）
  和 `run_command`（任意 PowerShell），**全部注册进 `main.rs` invoke_handler**，无防护
- 若评委追问"那 Tauri 命令面呢"，此论据会被当场击穿

**正确表述**：
> Agent 能力边界由 Python Tool Registry 决定，不含删除；
> 但应用的 Tauri 命令面比 Agent 更大，这层目前缺少保护——这正是本轮要修的。

**完整审计见**：`docs/LocalMind-LocalFile架构审计报告.md`（12 部分，含 P0/P1/P2 清单、
根因分析、目标架构、Permission Model、Memory 生命周期、Benchmark 方案、Phase 0-5 路线）

### 当前真实工作量
- ③ 是 PPT 一次性补充，10 分钟能搞定
- ④ 是大功能（手机远控电脑 + 局域网方案选型 + 双端鉴权 + UI），量级最大
- ⑤ 细节不明，需先问清目标
- ＋ LocalMind Agent 能力加强：需先与用户对齐方向

### 待确认的待问问题
- ⑤「自动更新记忆文档」是给 Agent 生成对话记忆，还是更新项目技术文档？
- ＋LocalMind Agent 能力要往哪个方向加强（工具集 / 长任务 / 持久记忆 / 多 Agent 协作）
- ④ 是否走 Tailscale 方案（局域网 P2P mesh），还是仍然坚持自建中继
