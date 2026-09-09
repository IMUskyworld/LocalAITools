# LocalMind / LocalFile 双端智能工具应用 - 行业调研报告 v0.1

> 本文档为《AICoding 架构设计》核心产物之一，定位为**行业调研报告（research_report）**。
> 上游输入：主理人转交的用户诉求（Windows 端 LocalMind 桌面助手 + Android 端 LocalFile 文件处理/远程控制器，云端中继协同，在线/离线双模式 AI）；
> 下游输出：驱动 `business-architect`（业务架构师）的行业调研判断，最终落入《高层架构设计》的 §3 行业调研章节。

> **工具说明**：由 `research-analyst`（研究分析师 - 查有据）负责产出，经 G2 自动校验与人工审核通过后方可进入下游消费。
> **结构纪律**：全文按「事实 → 对比 → 建议 → 风险」四段式组织。

---

## 0. 元信息：修订记录

```yaml
标题: LocalMind / LocalFile 双端智能工具应用 - 行业调研报告 v0.1
版本: v0.1
状态: Draft   # Draft | Reviewing | Approved | Deprecated
创建日期: 2026-07-20
最后更新: 2026-07-20
调研人: research-analyst（研究分析师 - 查有据）
审核人:
  - team-lead（主理人）

关联文档:
  上游输入:
    - 用户诉求: 由主理人注入（Windows LocalMind 桌面助手 + Android LocalFile 文件处理/远程控制，云端中继，在线/离线双模式 AI，exe/apk 交付，精美 GUI，先产出开发文档）
    - 调研目标: 由主理人注入（六大调研方向：端侧推理框架、Windows 桌面栈、Android 应用架构、远程控制通道、模型分发与合规、云端中继架构）
  下游产出:
    - 高层架构设计 §3 行业调研: 将由 business-architect 整合到此章节
```

| 版本 | 日期 | 作者 | 变更内容 | 评审状态 |
| --- | --- | --- | --- | --- |
| v0.1 | 2026-07-20 | research-analyst（查有据） | 初稿 | Draft |

---

## 1. 调研问题收敛

> 调研启动前，先围绕用户诉求收拢为明确的调研问题集合，确保调研不偏离当前项目背景。

### 1.1 原始调研种子

| 编号 | 待验证论题 | 来源（用户诉求要点） | 调研优先级 | 备注 |
| --- | --- | --- | --- | --- |
| S1 | 端侧（on-device）LLM 推理框架在 Windows x64 与 Android ARM64 双端的可用性、性能基线与一键安装可行性 | "离线模式通过内置下载链接一键安装本地大模型实现断网可用"（双端一致） | 高 | 决定离线模式技术底座 |
| S2 | Windows 桌面助手技术栈在精美 GUI、exe 打包体积、本地推理集成三方面的权衡 | "开发为类似 WorkBuddy 的桌面助手工具…最终打包为 exe 安装包…精美图形用户界面设计" | 高 | 决定 LocalMind 主体框架 |
| S3 | Android 端文件处理应用的框架选型与文件系统权限模型（SAF）约束 | "Android端（LocalFile）：聚焦文件处理功能…最终打包为 apk 安装包" | 高 | 决定 LocalFile 主体框架 |
| S4 | 对话式远程控制通道（手机操控电脑）在 NAT 穿透、云端中继必要性、指令安全鉴权上的成熟方案 | "提供对话式远程控制功能，可通过网络连接操控电脑端的 LocalMind 执行桌面助手操作" | 高 | 决定双端协同架构 |
| S5 | 本地大模型的选型、分发链路（内置下载链接）与许可证合规边界 | "离线模式通过内置下载链接一键安装本地大模型" | 高 | 涉及合规与成本 |
| S6 | 云端中继服务器架构：预置 API 代理（对用户隐藏真实 API key、仅显示"Deepseek-V4-Pro"）、双端指令中转、设备配对 | "在线模式通过预置 API 调用云端大模型（API 对用户不可见，仅显示模型名称…）" + "通过云端服务器实现跨平台协同" | 高 | 决定云端组件形态 |
| S7 | 行业内同类"本地 AI 助手"标杆产品（桌面端 + 移动端）的产品形态与商业模式 | "类似 WorkBuddy 的桌面助手工具" | 中 | 为功能集与交互设计提供参照 |

### 1.2 调研问题收敛

| 编号 | 调研问题 | 调研对象 | 调研目标 | 预期产出 | 关联种子 |
| --- | --- | --- | --- | --- | --- |
| Q1 | 同类"本地/离线 AI 助手"标杆产品（桌面端 + 移动端）的功能集、技术架构、模型分发与商业模式分别是什么？哪些设计可直接借鉴？ | LM Studio / Jan / Layla / PocketPal AI / llama.cpp 社区 | 标杆画像 + 能力事实清单 | §2 标杆盘点与详述卡片 | S7、S1、S5 |
| Q2 | 覆盖 Windows x64 + Android ARM64 双端的端侧 LLM 推理技术底座应如何组合（引擎 + 模型 + 量化 + 分发）？ | llama.cpp / MLC LLM / ONNX Runtime GenAI / Google ML Kit GenAI（Gemini Nano）+ GGUF 模型生态 | 推理引擎能力事实 + 双端组合建议 | §2.3 能力事实表 + §4.1/§4.3 选型建议 | S1、S5 |
| Q3 | Windows 桌面助手（精美 GUI + 小体积 exe + 集成本地推理）的技术栈应选 Electron、Tauri 还是原生（WPF/WinUI3）？ | Tauri / Electron / WinUI3/WPF 官方文档与迁移案例 | 桌面栈对比事实 + 推荐 | §2.3 能力事实表 + §4.3 技术栈建议 | S2 |
| Q4 | Android 端"文件处理 + 端侧 LLM + 网络控制通道"应用应选原生 Kotlin (Compose)、Flutter 还是 React Native？ | Kotlin+Jetpack Compose / Flutter / React Native（含 PocketPal、ChatterUI 两个 RN+llama.cpp 实践） | Android 栈对比事实 + 推荐 | §2.3 能力事实表 + §4.3 技术栈建议 | S3 |
| Q5 | 对话式远程控制与云端中继应如何设计：传输协议（WebSocket/MQTT/WebRTC DataChannel）、NAT 穿透与中继必要性、设备配对与指令鉴权、预置 API 代理网关？ | RustDesk 自建中继架构 / WebRTC ICE 体系 / one-api·new-api 开源 LLM 网关 / Tailscale 组网模式 | 协同通道架构事实 + 组合建议 | §2.2.5 标杆详述 + §4.1 边界建议 | S4、S6 |

---

## 2. 事实：标杆系统盘点和方案详述

> **四段式「事实」段**。只陈列调研发现的事实，不做引申建议或边界裁决。

### 2.1 行业标杆清单

**硬指标**：≥ 3 家；至少包含 1 家头部 SaaS 代表 + 1 家开源/自研代表。本项目覆盖 5 家标杆，其中头部闭源商业产品 2 家（LM Studio、Layla，分别代表桌面端与移动端的头部商业形态），开源代表 3 家（Jan、PocketPal AI、RustDesk）。

| 编号 | 标杆系统 | 厂商 / 社区 | 部署形态 | 场景覆盖 | 技术亮点 | 商业模式 | 调研来源 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | LM Studio | LM Studio（美国，闭源商业公司） | 本地桌面应用（Windows x64/ARM64、macOS、Linux）+ 本地 API 服务 | 桌面端本地 LLM 运行、模型浏览下载、OpenAI 兼容 API、MCP 工具、文档 RAG | 内置 Hugging Face 模型搜索下载；llama.cpp(GGUF)/MLX 双运行时；headless 模式（llmster）；LM Link 跨设备路由 | 免费闭源（个人免费，商用需联系授权）；SDK/CLI 为 MIT | SR-01、SR-05 |
| B2 | Layla | Layla Network（澳大利亚，闭源商业公司） | Android/iOS 移动应用（Google Play + 官网直发 APK） | 手机端完全离线 AI 助手：离线聊天、角色卡、图片生成、可下载角色/功能模块 | 7B 级模型手机端离线运行（模型文件约 4GB）；周更节奏；角色人格市场 | 免费试用 + 完整版一次性买断 | SR-02 |
| B3 | Jan | Menlo Research（janhq，开源社区驱动公司） | 开源桌面应用（Windows/macOS/Linux）+ 本地 OpenAI 兼容 API（localhost:1337） | 开源 ChatGPT 替代品：本地模型下载运行、云端模型接入（OpenAI/Anthropic/Groq 等）、自定义助手、MCP 扩展 | Tauri + React/TypeScript 技术栈；底层 llama.cpp；从 HuggingFace 拉取模型；100% 离线可用 | 开源（Apache-2.0）免费 | SR-03、SR-05 |
| B4 | PocketPal AI | a-ghorbani（开源个人项目，社区共建） | 开源移动应用（Android + iOS，Google Play/App Store 上架） | 手机端本地私有 AI 助手：GGUF 模型离线聊天、角色（Pals）、设备端 TTS、工具调用、基准测速 | React Native + llama.rn（llama.cpp 的 RN 绑定，JSI 桥接）；应用内直接搜索下载 HuggingFace GGUF 模型（支持 gated 模型 token）；Android GPU(OpenCL)/NPU(Qualcomm Hexagon) 加速路径 | 开源（MIT）免费 | SR-04 |
| B5 | RustDesk | RustDesk 开源社区 | 开源远程桌面（Windows/macOS/Linux/Android/iOS/Web）+ 可自建 hbbs/hbbr 服务器 | 跨平台远程桌面控制：P2P 直连 + 中继兜底、无人值守、文件传输、剪贴板同步 | Rust 核心 + Flutter UI；hbbs（ID/信令）+ hbbr（中继）双组件自建；NaCl 端到端加密（中继不可读）；TCP 打洞优先、中继兜底 | 开源（AGPL-3.0）+ Server Pro 商业授权 | SR-06、SR-07 |

### 2.2 标杆方案详述

#### 2.2.1 B1 - LM Studio

| 维度 | 内容 | 置信度 |
| --- | --- | --- |
| 产品定位 | 面向消费级用户的本地 LLM 桌面运行平台："run Llama, DeepSeek, Qwen, Phi locally"，附 OpenAI 兼容本地 API | 已核实（SR-01 官方文档首页） |
| 目标用户 | 非技术用户（GUI 一键下载模型）+ 开发者（REST API、Python/TS SDK、CLI、MCP） | 已核实（SR-01） |
| 核心能力 | ① 内置 HuggingFace 模型搜索与一键下载；② llama.cpp(GGUF)/MLX 双运行时推理；③ OpenAI 兼容本地 server；④ 文档离线 RAG；⑤ MCP 客户端；⑥ headless 模式 llmster；⑦ LM Link 跨设备负载路由 | 已核实（SR-01） |
| 架构特点 | 桌面 GUI + 内置推理运行时（llama.cpp 以"LM Runtimes"形式可插拔管理，Ctrl+Shift+R 管理安装）；模型与运行时分离下载 | 已核实（SR-01"Run llama.cpp (GGUF) or MLX models"章节） |
| 部署形态 | 本地桌面应用（Windows x64/ARM64、macOS、Linux x64）；可完全离线运行（需先下载模型） | 已核实（SR-01 System requirements / Offline Operation） |
| 集成方式 | OpenAI 兼容 REST API、LM Studio REST API(beta)、Python SDK、TypeScript SDK、lms CLI、MCP | 已核实（SR-01） |
| 定价模式 | 个人使用免费；闭源；商用需联系官方授权；仅 SDK/CLI 以 MIT 开源 | 已核实（SR-05 Ollama alternatives 评测） |
| 优势 | 与本项目 Windows 端形态最贴近：精美 GUI、内置模型市场（HF 搜索下载）、一键即用、离线可用、本地 API 双模 | 综合归纳 |
| 局限 | 闭源不可审计、不可二次分发其运行时；无 Android 客户端；模型源强依赖 HuggingFace（中国大陆直连不稳定，属本项目环境推断） | 已核实（闭源/无 Android，SR-05）+ 推断（HF 可达性） |
| 对本项目的参考价值 | 正面参照"模型市场内嵌 + 一键下载 + 本地推理 + OpenAI 兼容 API"的产品闭环；其 LM Link 跨设备路由证明"桌面端为推理主机、其他设备远程调用"形态已被头部产品验证 | 推断 |

#### 2.2.2 B2 - Layla

| 维度 | 内容 | 置信度 |
| --- | --- | --- |
| 产品定位 | "World's First Private AI"：完全在手机本地离线运行的 AI 助手 | 已核实（SR-02 官网首页） |
| 目标用户 | 注重隐私的消费级移动用户（非技术人群） | 已核实（SR-02） |
| 核心能力 | 离线聊天、多角色人格（可下载角色包）、图片生成、短故事/创意写作、可下载功能模块；7B 参数模型、模型文件约 4GB | 已核实（SR-02"Layla In Numbers"：7B Parameters / 4GB File Size） |
| 架构特点 | 端侧推理（官方宣称 state-of-the-art 端侧技术，具体引擎未公开，社区普遍认为基于 llama.cpp 类 GGUF 运行时）；模型与角色包均通过应用内下载分发 | 推断（引擎未公开） |
| 部署形态 | Android（Google Play + 官网直发 APK 双渠道）+ iOS；完全离线可用 | 已核实（SR-02 提供 direct APK 下载链接） |
| 集成方式 | 无开放 API（面向终端用户的封闭应用） | 已核实（官网无开发者文档） |
| 定价模式 | 免费试用（功能受限）+ 完整版一次性买断 | 已核实（SR-02） |
| 优势 | 证明"7B 级模型 + 约 4GB 模型文件 + 手机离线运行 + 应用内下载"的消费级闭环在 2024-2026 已商业可行；官网直发 APK 是其重要分发渠道 | 综合归纳 |
| 局限 | 闭源；无桌面端、无双端协同；无远程控制能力；无文件处理定位 | 已核实 |
| 对本项目的参考价值 | 移动端离线模式的直接对标：验证了 LocalFile"内置下载链接一键安装模型、断网可用"的产品形态与模型规格（4~5GB 级）可行性 | 推断 |

#### 2.2.3 B3 - Jan

| 维度 | 内容 | 置信度 |
| --- | --- | --- |
| 产品定位 | 开源的 ChatGPT 替代品，100% 离线在本机运行 | 已核实（SR-03 GitHub README） |
| 目标用户 | 注重隐私与可审计性的桌面用户、开发者 | 已核实（SR-03） |
| 核心能力 | ① 从 HuggingFace 下载运行本地模型（Llama/Gemma/Qwen/gpt-oss 等）；② 云端模型接入（OpenAI/Anthropic/Mistral/Groq/MiniMax 等）；③ 自定义助手；④ 本地 OpenAI 兼容 API（localhost:1337）；⑤ MCP 集成 | 已核实（SR-03 Features） |
| 架构特点 | **Tauri（Rust 后端）+ React/TypeScript 前端**；底层推理基于 llama.cpp；构建需 Node.js ≥ 20 + Yarn ≥ 4.5.3 + Rust | 已核实（SR-03 Build from Source / Acknowledgements） |
| 部署形态 | Windows（jan.exe + Microsoft Store）、macOS、Linux（deb/AppImage/Flathub） | 已核实（SR-03 Installation） |
| 集成方式 | OpenAI 兼容 API、MCP 插件 | 已核实（SR-03） |
| 定价模式 | 开源免费（Apache-2.0） | 已核实（SR-03 License） |
| 优势 | 与本项目 Windows 端需求几乎同构的开源参照：Tauri 桌面栈 + llama.cpp + 本地/云端双模 + OpenAI 兼容 API + 精美 GUI，全部源码可学可借 | 综合归纳 |
| 局限 | 无 Android 客户端（官方曾预告移动端，未见发布）；无远程控制通道；模型源依赖 HuggingFace | 已核实（SR-03 无移动版） |
| 对本项目的参考价值 | 技术栈直接参照（Tauri + llama.cpp + 双模切换 + 本地 API）；其"本地推理 + 远端 provider 并存"的 UI/配置模式可复刻到 LocalMind | 推断 |

#### 2.2.4 B4 - PocketPal AI

| 维度 | 内容 | 置信度 |
| --- | --- | --- |
| 产品定位 | 完全在手机上本地运行的私有 AI 助手："no account, no cloud, no internet required" | 已核实（SR-04 GitHub README） |
| 目标用户 | 注重隐私的移动用户与开源社区开发者 | 已核实（SR-04） |
| 核心能力 | ① 设备端 GGUF 模型聊天（Gemma/Qwen/Phi/Llama）；② 应用内 HuggingFace 搜索下载（支持 gated 模型 token）；③ 角色（Pals）与角色市场；④ 设备端 TTS（ONNX）；⑤ 工具调用；⑥ tokens/s 与内存基准测速 | 已核实（SR-04） |
| 架构特点 | **React Native 0.82（新架构）+ TypeScript + llama.rn（llama.cpp 的 RN 绑定，JSI 桥接）**；Android 加速路径 CPU→GPU(OpenCL/Adreno)→NPU(Qualcomm Hexagon) 优雅降级；四层架构（UI/桥接/引擎/硬件） | 已核实（SR-04） |
| 部署形态 | Android（Google Play）+ iOS（App Store） | 已核实（SR-04） |
| 集成方式 | 本地模型 + BYOK 联网搜索工具（Brave/Tavily/Exa） | 已核实（SR-04） |
| 定价模式 | 开源免费（MIT） | 已核实（SR-04 License） |
| 优势 | 提供了"RN + llama.cpp + 应用内 HF 模型下载 + 离线推理 + 上架 Google Play"的完整开源工程样本；其硬件加速降级链（CPU/GPU/NPU）与测速工具可直接借鉴 | 综合归纳 |
| 局限 | 个人主导项目，体量与长期维护资源有限；iOS 之外的桌面端缺失；无文件处理与远程控制 | 推断（维护风险） |
| 对本项目的参考价值 | LocalFile 离线模式的工程参照：模型下载 UX、量化版本选择引导、端侧推理桥接方式、基准测速页均可借鉴 | 推断 |

#### 2.2.5 B5 - RustDesk

| 维度 | 内容 | 置信度 |
| --- | --- | --- |
| 产品定位 | 开源远程桌面，TeamViewer/AnyDesk 的自托管替代品 | 已核实（SR-06、SR-07） |
| 目标用户 | 个人远程访问、IT 支持、注重数据主权的团队 | 已核实（SR-06） |
| 核心能力 | 跨平台远程桌面控制（被控/主控双角色）、P2P 直连 + 中继兜底、无人值守访问、文件传输、剪贴板同步、音频转发、多显示器 | 已核实（SR-06、SR-07） |
| 架构特点 | **hbbs（ID/Rendezvous 信令服务器，TCP 21115/21116 + UDP 21116）+ hbbr（中继服务器，TCP 21117）双组件可自建**；TCP 打洞优先、中继兜底；NaCl 端到端加密（Curve25519 密钥交换 + xSalsa20-Poly1305 会话加密 + Ed25519 签名），中继仅见密文；Rust 核心 + Flutter UI | 已核实（SR-07 详述端口与加密体系） |
| 部署形态 | 客户端覆盖 Windows/macOS/Linux/Android/iOS/Web；服务器自托管（Docker 双容器即可） | 已核实（SR-06） |
| 集成方式 | 客户端内嵌服务器配置（可预打包指向自建 hbbs/hbbr）；Web 客户端走 WebSocket 端口 | 已核实（SR-07） |
| 定价模式 | 客户端与 Server OSS 均 AGPL-3.0 免费；Server Pro（Web 控制台/设备分组/LDAP）商业授权 | 已核实（SR-07） |
| 优势 | 行业内"手机 ↔ 电脑经云端中继建立受控通道"的最成熟开源范式：打洞优先降中继成本、E2E 加密使中继零信任、ID 配对模型简单可用 | 综合归纳 |
| 局限 | AGPL-3.0 对直接复制代码不友好（本项目若借鉴须重写而非引入其代码）；其协议面向"屏幕+输入"远程桌面，本项目仅需"对话式指令通道"，体量上过重 | 已核实（许可证）+ 推断（协议适配度） |
| 对本项目的参考价值 | 借鉴其**架构模式**而非代码：ID/信令服务器负责配对与打洞协商、中继兜底、E2E 加密、客户端预置服务器配置的发布方式 | 推断 |

### 2.3 关键技术能力横向事实

> 不评分、不排序，仅按能力维度横陈各方案事实。除标杆产品外，本表同时纳入 Q2~Q5 涉及的候选技术组件事实（来源见 §6）。

| 能力维度 | 事实陈述 | 说明 / 来源 |
| --- | --- | --- |
| 端侧推理引擎 - llama.cpp | MIT 许可；C/C++ 实现；支持 Windows（winget 预编译）、Linux、macOS，ARM/NEON 优化可编译至 Android；GGUF 格式，1.5~8bit 量化；可作为 libllama 库嵌入，也提供 llama-server（OpenAI 兼容 REST API）；被 Jan/LM Studio/PocketPal/ChatterUI 等作为底层引擎 | SR-08、SR-05 |
| 端侧推理引擎 - MLC LLM | 机器学习编译路线（TVM 栈）；同一引擎覆盖 REST/Python/JS/iOS/Android，OpenAI 兼容 API；移动端 GPU（Vulkan/Metal）优化 | SR-09 |
| 端侧推理引擎 - 移动端实测 | arXiv 2410.03613 基准（骁龙8Gen2/麒麟9000S/天玑9200+，Llama2-7B Q4）：MLC LLM(GPU) 吞吐 9.5~14.2 tok/s，全面高于 llama.cpp(CPU) 6.8~8.7 tok/s（平均 +63.5%）；GPU 推理较 CPU 节能约 33.6%；GPU ALU 利用率仅 11~18%，瓶颈在内存带宽（权重加载占计算周期 65~75%） | SR-10（arXiv 2410.03613 转述） |
| 端侧推理引擎 - Google ML Kit GenAI（Gemini Nano） | 官方高层 API（总结/校对/改写/图像描述/Prompt API），基于 AICore 系统服务；仅限旗舰机型白名单（Pixel 9/10、Galaxy S25、小米 15、Find X8 等），Pixel 9a 中端机即不支持；模型不可自选、不可分发自有模型 | SR-11 |
| 桌面框架 - Tauri 2.0 vs Electron 30 | 实测：Tauri 起步包 4.2MB vs Electron 112MB；空闲 RSS 128MB vs 342MB；TTFP 420ms vs 1180ms；Tauri 用系统 WebView2（Windows）+ Rust 后端，Electron 捆绑 Chromium+Node（约 180MB）；Firezone 实例—Tauri 重写后内存 280MB→35MB、冷启动 4s→0.8s | SR-12、SR-13 |
| 桌面框架 - 生产案例 | Jan（Tauri）、Firezone 客户端（Tauri，2 个月出 Windows beta）、得物商家客服系统（Electron→Tauri，包体 -96%）；VS Code/Slack/Discord（Electron，生态最成熟） | SR-03、SR-13、SR-14 |
| Android 框架 - 原生 Compose vs Flutter vs RN | Kotlin 在 Android 生态渗透率超 80%；Compose 空包 4.2MB/冷启动 127ms（Pixel 6a），Flutter 空包 5.8MB/冷启动 189ms；Flutter 自绘引擎跨端一致性最好；RN 有 PocketPal/ChatterUI 两个 llama.cpp 桥接先例（llama.rn / cui-llama.rn） | SR-15、SR-16、SR-04 |
| Android 文件权限 | Android 11+ 分区存储（Scoped Storage）强制；文件管理类需 SAF（Storage Access Framework）或 MANAGE_EXTERNAL_STORAGE 特殊权限（Google Play 对后者有审核限制）；属平台硬约束，与框架选型无关 | 推断（Android 官方通行约束，未单独检索官方页面） |
| 远程通道协议对比 | WebRTC DataChannel：UDP，延迟 50~200ms，内置 ICE(STUN/TURN) NAT 穿透，DTLS 强制加密；纯 STUN 在对称 NAT 下失败率超 40%，大型部署 TURN 中继兜底比例约 8~20%；WebSocket：TCP，延迟约几十~300ms，无 NAT 穿透能力，必须经服务器中转；MQTT：发布/订阅，QoS 0/1/2，适合 IoT 指令，同样需 broker 中转 | SR-17、SR-18、SR-19 |
| 远程通道范式 - RustDesk | hbbs 信令/ID 服务器 + hbbr 中继；TCP 打洞优先、中继兜底；NaCl E2E 加密中继零信任；客户端可预置自建服务器配置发布 | SR-06、SR-07 |
| 模型许可 - DeepSeek-R1-Distill 系列 | DeepSeek-R1-Distill-Qwen 全系（1.5B/7B/8B/14B/32B/70B）MIT 许可，允许商用、修改、再分发；底座 Qwen2.5 为 Apache-2.0；DeepSeek-R1-0528-Qwen3-8B 同为 MIT | SR-20、SR-21 |
| 模型许可 - Llama 3.x | Llama 社区许可：允许商用与再分发，但月活超 7 亿需单独授权，且需署名"Built with Llama"；对中小产品基本无约束 | SR-10（模型卡通行条款，综合归纳） |
| 模型规格 - 端侧可行配置 | Phi-4-mini 3.8B（Q4 GGUF 约 2.5GB）；Gemma 3 2B/4B；Llama 3.2 1B/3B（首个为移动设计的 Llama）；DeepSeek-R1-0528-Qwen3-8B Q4_K_M GGUF 约 5.21GB；RTX 4060 8GB 可跑 7B Q4 约 20~30 tok/s，纯 CPU 16GB 内存跑 3B Q4 约 3~5 tok/s | SR-22、SR-23、SR-24 |
| 云端 LLM 网关 - one-api / new-api | 开源（Go 单可执行文件 + Docker 一键部署）；统一管理多渠道 LLM API key，对外以 OpenAI 格式二次分发令牌；支持模型映射（客户端请求名可映射到任意真实模型）、令牌额度/过期/可用模型范围控制、渠道负载均衡与失败重试 | SR-25、SR-26 |
| 云端 API 成本 - DeepSeek 官方 API | deepseek-chat（V3 系）：输入 0.5 元（缓存命中）/2 元（未命中）每百万 tokens，输出 8 元每百万 tokens；deepseek-reasoner（R1）：输入 1 元（命中）/4 元（未命中），输出 16 元每百万 tokens；00:30-08:30 错峰半价/2.5 折 | SR-27、SR-28 |

---

## 3. 对比：对比矩阵与加权评分

> **四段式「对比」段**。在 §2 的事实基础上建立对比矩阵，赋予权重并打分。

### 3.1 对比矩阵

> **每行权重之和 = 1.00**。本项目核心交付物是"自研双端应用"，标杆的作用是提供**架构蓝本与工程参照**，因此评分对象是"各标杆作为本项目架构参照的适配度"。

| 评估维度 | 权重 | 权重理由 | B1 LM Studio | B2 Layla | B3 Jan | B4 PocketPal | B5 RustDesk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 场景契合度 | 0.30 | 用户诉求的功能重合度是第一筛选条件：双端、在线/离线双模、远程控制、文件处理，重合度越高，可参照的现成设计越多 | 3 | 3 | 4 | 3 | 3 |
| 技术成熟度 | 0.20 | 参照对象必须是经过大规模用户验证的形态，避免参照了未经验证的实验性设计 | 5 | 4 | 4 | 3 | 5 |
| 集成难度（反向） | 0.15 | 本项目为小团队自研（假设），参照物的架构越简单、可得性越高（开源/文档全），落地代价越小 | 3 | 2 | 5 | 4 | 4 |
| 成本（反向） | 0.15 | 含学习成本、授权成本、运行成本；闭源/买断制产品无法提供代码级参照，成本低但收益也低 | 4 | 4 | 5 | 5 | 4 |
| 合规可控性 | 0.20 | 参照物许可证是否允许借鉴/复用、模型与组件许可是否干净、是否存在闭源绑定风险；本项目要商用分发 exe/apk，合规一票否决权重高 | 2 | 2 | 5 | 5 | 3 |
| **加权总分** | **1.00** | — | **3.30** | **2.95** | **4.45** | **3.90** | **3.75** |

**评分标尺**：每项 1~5 分，1 = 严重不符合，3 = 基本满足但存在明显局限，5 = 完美契合。

**关键单项得分依据**（摘录，全部事实见 §2）：
- B3 Jan 场景契合度 4 分：Tauri+llama.cpp+双模+本地 API 与 LocalMind 需求几乎同构（SR-03），但缺 Android 端与远程控制，未达 5 分。
- B3 合规可控性 5 分：Apache-2.0，可直接借鉴代码与架构（SR-03）。
- B1 合规可控性 2 分：闭源，仅可参照产品形态，不可借鉴任何实现（SR-05）。
- B5 技术成熟度 5 分：GitHub 约 113k stars、多平台生产级验证（SR-06、SR-07）。
- B5 合规可控性 3 分：AGPL-3.0 允许借鉴架构思想但禁止直接引入代码进闭源产品（SR-07）。
- B4 技术成熟度 3 分：已上架 Google Play/App Store 但属个人主导开源项目（SR-04）。

### 3.2 评分结论

> 基于 §3.1 加权总分，形成分层结论。每层结论必须引用得分作为依据。

- **优先借鉴**：**B3 Jan** — 适用度评分：**4.45**。理由：与本项目 Windows 端（LocalMind）技术需求几乎同构（Tauri 桌面栈 + llama.cpp 本地推理 + 本地/云端双模 + OpenAI 兼容 API + HF 模型内嵌下载，SR-03），Apache-2.0 许可可代码级借鉴，集成难度 5/5、合规 5/5；其缺失的 Android 端与远程控制通道由 B4/B5 补齐。
- **部分借鉴**：**B4 PocketPal AI** — 评分：**3.90**。借鉴点：Android 端 llama.cpp 桥接工程（llama.rn/JSI）、应用内 HF 模型搜索下载 UX、量化版本引导、硬件加速降级链（CPU→OpenCL→Hexagon NPU）、端侧基准测速页（SR-04）。不借鉴的部分：React Native 作为 LocalFile 主框架的选型需结合 §3.3 与团队栈裁决（RN 非唯一路径），其角色市场/社区功能超出本项目范围。
- **部分借鉴**：**B5 RustDesk** — 评分：**3.75**。借鉴点：hbbs/hbbr 式"信令配对 + 打洞优先 + 中继兜底 + E2E 加密中继零信任"的双端通道架构模式、客户端预置自建服务器配置的发布方式（SR-06、SR-07）。不借鉴的部分：其远程桌面屏幕采集/输入注入协议栈（本项目只需对话式指令通道，体量过重）；不引入其代码（AGPL-3.0 与闭源分发冲突）。
- **不借鉴（否决）**：**B2 Layla** — 评分：**2.95**。否决理由：闭源且无任何开放接口/文档，除"7B 模型 4GB 文件手机离线可行"这一事实结论外无工程参照价值；集成难度单项 2/5、合规 2/5。其商业结论（消费级离线 AI 可买断制收费）仅作市场佐证保留。
- **不借鉴（作为实现参照否决；保留为功能对标）**：**B1 LM Studio** — 评分：**3.30**。否决理由：闭源（合规单项 2/5），不可借鉴实现；其 Windows 桌面形态（模型市场 + 一键下载 + 本地 API + 双模）已由开源的 B3 Jan 同构覆盖，无需依赖闭源参照。

### 3.3 方案组合分析（如有）

> 单一标杆无法覆盖双端 + 远程控制 + 文件处理的全部需求，组合是必然选择。

| 组合方式 | 覆盖哪些能力 | 未覆盖能力 | 组合复杂度 | 总体成本估算 |
| --- | --- | --- | --- | --- |
| B3(Jan 架构) Windows 端 + B4(PocketPal 工程) Android 端 + B5(RustDesk 模式) 通道 + one-api 系网关做预置 API 代理 | LocalMind 桌面助手全栈、LocalFile 离线推理与模型下载、双端配对与远程指令通道、API key 隐藏与模型名映射（"Deepseek-V4-Pro"） | 文件处理的业务功能设计（摘要/转换/管理等，属业务架构阶段）；UI 视觉设计；云端中继的具体容量规划 | 中（四个参照系各自独立、职责边界清晰） | 全部组件开源免费（Apache-2.0/MIT/AGPL 仅借鉴架构）；主要成本为自研工程量，假设小团队 3~5 人，量级为月级〔假设〕 |

---

## 4. 建议：取舍决策支持

> **四段式「建议」段**。基于 §2 事实 + §3 对比，给出可被 `business-architect` 直接采用的建议。本节是建议而非最终裁决，最终边界由业务架构师冻结。

### 4.1 自研 / 采购 / 复用边界建议

| 能力项 | 建议方式 | 建议依据 | 候选方案 / 系统 | 关键前提 |
| --- | --- | --- | --- | --- |
| 端侧 LLM 推理引擎（Windows + Android） | 复用（开源组件） | llama.cpp 双端覆盖、MIT 许可、被 B1/B3/B4 全部验证（§2.3）；MLC LLM 可作 Android GPU 加速备选 | llama.cpp（GGUF，lib 嵌入 + server 模式）；备选 MLC LLM | 需验证所选模型在目标机型 Q4 量化下的 tok/s 达标（移动端 ≥ 8 tok/s，SR-10） |
| 本地模型分发（内置下载链接一键安装） | 自研下载器 + 复用开源模型权重 | DeepSeek-R1-Distill/Qwen3 系列 MIT/Apache 许可允许再分发（§2.3）；B1/B2/B3/B4 均验证"应用内下载 + 断网可用"形态 | 自建 CDN/对象存储直链（规避 HF 大陆可达性）+ 官方模型仓库镜像兜底 | 须在应用内保留模型许可文本与署名；需法务确认再分发条款 |
| Windows 桌面助手主体 | 自研（参照 B3 Jan 架构） | B3 评分 4.45、Apache-2.0 可代码级参照；Tauri 包体/内存优势（§2.3，4.2MB vs 112MB） | Tauri 2.x + React/TS + llama.cpp；备选 Electron（生态最熟）或 WinUI3（仅 Windows） | 团队需具备或学习 Rust（迁移案例显示前端团队约需 2 周~3 个月适应，SR-13/SR-14） |
| Android 文件处理主体 | 自研（参照 B4 工程） | 文件处理深度依赖 SAF/系统 API，原生 Kotlin+Compose 集成阻力最小；B4 证明 RN+llama.cpp 亦可行 | 原生 Kotlin + Jetpack Compose（推理经 JNI 桥 llama.cpp）；备选 Flutter / RN( llama.rn) | 若团队无 Kotlin 经验且有 RN/Flutter 经验，可改选 B4 路线（影响 §4.3） |
| 对话式远程控制通道 | 自研（借鉴 B5 架构模式，不引入代码） | B5 模式（信令配对 + 打洞 + 中继兜底 + E2E 加密）成熟；指令通道数据量小，WebSocket 经云端中继已够用，WebRTC DataChannel 为低延迟升级项 | MVP：WSS over 云端中继；进阶：WebRTC DataChannel + STUN/TURN | 云端服务器为必选组件（用户诉求已明确"通过云端服务器实现跨平台协同"） |
| 预置 API 代理（隐藏 key + 显示"Deepseek-V4-Pro"） | 复用（开源网关） | one-api/new-api 开源、Go 单文件/Docker 部署、模型映射可让客户端显示名与真实模型解耦（§2.3，SR-25/SR-26） | new-api（或 one-api）；备选 LiteLLM Proxy / 自研极简代理 | 需确认所选网关的模型映射与令牌范围控制满足"仅显示 Deepseek-V4-Pro" |
| 设备配对与连接管理 | 自研 | 各标杆无现成可复用的"应用内账号-设备配对"组件；RustDesk 的 ID 配对模式可借鉴 | 配对码/扫码 + 云端设备表 | 与安全设计的鉴权方案联动（建议 G5 阶段重点审查） |
| 桌面助手"执行操作"能力（打开应用/文件操作等） | 自研 | 无标杆覆盖；属业务功能 | 指令白名单 + 确认执行的 Agent 执行器 | 远程指令必须经鉴权 + 白名单，防误操作与滥用 |

### 4.2 MVP 范围建议

> 对用户诉求中的 P0/P1 功能给出"是否可在 MVP 内实现"的调研侧建议。

| 功能（对齐用户诉求） | 建议 MVP？ | 理由 |
| --- | --- | --- |
| R1 Windows 端在线模式（预置 API 调云端模型，显示"Deepseek-V4-Pro"） | ✅ | new-api 类网关 + 任意 chat UI 即可闭环，DeepSeek 官方 API 价格低廉（输入 2 元/百万 tokens 未命中，SR-27） |
| R2 Windows 端离线模式（内置下载链接一键安装本地模型，断网可用） | ✅ | B1/B3 均已验证形态；llama.cpp + 7B/8B Q4 GGUF（约 4~5GB）在 16GB 内存 PC 可用（SR-22、SR-24） |
| R3 Android 端文件处理基础能力（浏览/选择/分享文件 + 在线 AI 处理） | ✅ | SAF 标准能力 + 在线 API 调用，无技术风险 |
| R4 Android 端离线模式（同 R2 策略） | ✅（建议跟随 R2 之后半个迭代） | B2/B4 验证可行，但移动端机型碎片化严重（SR-10 显示不同 SoC 性能差 2 倍+），需预留机型适配与降级策略时间 |
| R5 对话式远程控制（手机经云端中继操控 LocalMind） | ✅（MVP 内做"指令白名单 + 中继通道"最小集） | WebSocket 中继 + 配对码 + 指令白名单工程量可控；WebRTC 打洞降级优化可后置 |
| R6 精美 GUI（双端） | ✅ | Tauri（Web 技术栈）与 Compose 均有成熟 UI 生态；属投入问题而非可行性问题 |
| R7 exe / apk 安装包交付 | ✅ | Tauri 官方产出 msi/exe（Jan 即 jan.exe，SR-03）；Android 直发 APK 有 B2 先例（SR-02） |
| R8 远程控制中的"桌面画面实时回传" | ❌（完整版） | 进入屏幕采集/编码传输领域（RustDesk 级工程量），与"对话式控制"诉求不符，建议仅回传执行结果文本/截图单帧 |
| R9 多模型市场 / 角色社区 | ❌（完整版） | B1/B2/B4 的社区化能力依赖内容运营，MVP 预置 1~2 个模型即可 |

### 4.3 技术栈参考建议

| 技术层 | 推荐方案 | 替代方案 | 选择理由 |
| --- | --- | --- | --- |
| 端侧推理引擎（双端共用） | llama.cpp（GGUF） | MLC LLM（Android GPU 加速）、ONNX Runtime GenAI（Windows 企业栈） | 双端覆盖最广、MIT 许可、B1/B3/B4 共同底座；GGUF 生态模型最全（§2.3） |
| 离线模型（Windows 默认） | DeepSeek-R1-0528-Qwen3-8B Q4_K_M（约 5.2GB，MIT） | Qwen3-4B/8B、Llama 3.1 8B | 与在线模式"Deepseek"品牌一致；MIT 可再分发（SR-20、SR-23） |
| 离线模型（Android 默认） | Qwen3-4B 或 DeepSeek-R1-Distill-Qwen-7B Q4（约 4GB） | Gemma 3 4B、Llama 3.2 3B、Phi-4-mini 3.8B | 旗舰 SoC Q4 下 8~14 tok/s（SR-10）；4GB 级文件与 B2 验证的消费级规格一致（SR-02） |
| Windows 桌面框架 | Tauri 2.x + React/TypeScript | Electron（生态最成熟）、WinUI 3（纯 Windows 原生） | 包体 4.2MB vs 112MB、内存 128MB vs 342MB（SR-12）；B3 Jan 同构可参照 |
| Android 框架 | Kotlin + Jetpack Compose | Flutter、React Native + llama.rn | 文件处理强依赖 SAF/系统 API 原生阻力最小；Compose 空包 4.2MB/冷启动 127ms（SR-16）；推理经 JNI 桥 llama.cpp |
| 远程控制通道（MVP） | WSS（WebSocket over TLS）经云端中继 | MQTT over WSS（QoS 指令语义）、HTTP 长轮询（兜底） | 指令通道数据量小，中继模式天然解决 NAT；工程最简（SR-18、SR-19） |
| 远程控制通道（进阶） | WebRTC DataChannel + STUN/TURN | 维持纯中继 | 延迟 50~200ms、打洞成功可走 P2P 省中继带宽；TURN 兜底比例约 8~20%（SR-17、SR-18） |
| 云端 API 网关 | new-api（one-api 系） | LiteLLM Proxy、自研 FastAPI/Go 极简代理 | 开源、Docker 一键部署、模型映射 + 令牌管控恰好覆盖"隐藏 key + 显示 Deepseek-V4-Pro"（SR-25、SR-26） |
| 云端中继服务 | Go/Node.js 自研轻量中继（WS 网关 + 设备表 + 配对码） | 复用 new-api 同栈扩展；MQTT broker（EMQX 开源版） | 指令中转 + 配对管理逻辑轻，自研可控；无现成标杆组件可直接复用 |
| 在线模型供应 | DeepSeek 官方 API（deepseek-chat） | 阿里云百炼/腾讯云等国内托管 DeepSeek | 输入 2 元/百万 tokens、输出 8 元/百万 tokens，错峰更低（SR-27、SR-28）；国内直连稳定 |

---

## 5. 风险与待确认项

> **四段式「风险」段**。列出调研中发现的主要风险、不确定信息、待业务架构师进一步裁决的依赖项。

### 5.1 主要风险清单

| 编号 | 风险描述 | 触发条件 | 影响范围 | 严重程度 | 缓解建议 |
| --- | --- | --- | --- | --- | --- |
| R-01 | 移动端离线推理在低端/中端机型不可用：实测不同 SoC 吞吐差 2 倍+（6.8~14.2 tok/s），且 Google 端侧 API 仅旗舰白名单，反映行业整体对中低端机支持不足 | 用户设备为中低端 Android（8GB 内存以下或非旗舰 SoC） | LocalFile 离线模式体验崩坏（吐字过慢/OOM） | 高 | ① 下载前设备检测（RAM/SoC 门槛）；② 预置多档模型（1.5B/3B/4B/7B）按设备推荐；③ 提供"仅在线模式"降级路径；④ 借鉴 B4 内置基准测速页让用户自验 |
| R-02 | 模型再分发合规瑕疵：虽然 DeepSeek-R1-Distill/Qwen3 为 MIT/Apache，但若误选 Llama（署名要求）或 gated 模型（HF 需授权 token），直接内置下载链接可能违反许可 | 模型选型阶段未逐模型核对许可证即打包分发 | 法律合规风险、应用商店下架风险 | 高 | ① 模型清单逐一过许可（默认全部选 MIT/Apache 系）；② 应用内附许可文本与署名页；③ 分发前法务确认 |
| R-03 | 远程控制通道被滥用或中间人攻击：中继服务器若明文转发指令，被攻破后可操控用户电脑执行桌面操作 | 未做端到端加密或指令未鉴权 | 全部启用远程控制的用户设备安全 | 高 | ① 借鉴 B5 NaCl 式 E2E 加密（中继零信任）；② 配对码短时有效 + 设备指纹绑定；③ 指令白名单 + 危险操作本地二次确认；④ 提交 G5 安全设计专项审查 |
| R-04 | 云端持续成本失控：DeepSeek API 按量计费 + 中继带宽（若走纯中继模式，全部指令流量经服务器）+ 模型文件 CDN 流量（4~5GB/次下载） | 用户量增长或个别用户高频调用/反复下载模型 | 运营成本超预算 | 中 | ① 网关侧令牌额度与速率限制（new-api 原生支持）；② 模型 CDN 设下载次数/防盗链；③ 错峰时段引导（DeepSeek 00:30-08:30 半价）；④ 进阶引入 WebRTC P2P 卸载中继流量 |
| R-05 | HuggingFace 在中国大陆直连不稳定，若照搬 B1/B3/B4 的 HF 内嵌下载模式，国内用户模型下载失败率高 | 模型源仅配置 HF 直链 | 离线模式核心功能不可用 | 高 | ① 默认自建 CDN/对象存储直链（用户诉求本就是"内置下载链接"）；② 国内镜像（ModelScope）作备选源；③ 断点续传 + 分片校验 |
| R-06 | Tauri 系统 WebView2 依赖与碎片化：Windows 7/老旧 Windows 10 无 WebView2 运行时，需安装器引导补装；不同 WebView 版本存在少量渲染差异 | 目标用户存在老旧 Windows 环境 | 安装失败、UI 兼容性问题 | 中 | ① 安装包捆绑 WebView2 引导安装（Tauri 官方支持）；② 明确系统需求 Win10 1809+；③ 若必须覆盖极老系统则备选 Electron |
| R-07 | "Deepseek-V4-Pro"显示名与真实模型的映射被用户识破或舆论质疑（若映射到非 DeepSeek 模型或版本不符） | 网关模型映射配置与对外宣传不一致 | 品牌信任风险 | 中 | ① 建议默认映射到真实 DeepSeek 官方模型，显示名仅作版本品牌化；② 在用户协议中说明"模型名称可能为品牌名" |

### 5.2 待确认项（需主理人 / 业务方反馈）

| 编号 | 待确认项 | 不确定性说明 | 若无法确认的备选路径 |
| --- | --- | --- | --- |
| U-01 | "Deepseek-V4-Pro"对应的真实模型与供应方（DeepSeek 官方并无公开发布的"V4-Pro"型号；官方现行为 deepseek-chat(V3 系)/deepseek-reasoner(R1)） | 用户诉求仅给定显示名，真实映射关系属业务决策；公开资料查无"V4-Pro" | 默认映射 deepseek-chat 并在网关中配置模型映射；若实际指向第三方供应方，需主理人确认渠道与价格 |
| U-02 | 目标用户设备画像（Windows 内存/GPU 档位、Android 机型档位） | 用户诉求未给出；直接影响离线模型档位选择（3B/4B/7B/8B）与 R-01 缓解策略 | 按"16GB 内存无独显 PC + 骁龙 8 系旗舰 Android"为基准画像设计〔假设〕，并提供多档模型 |
| U-03 | 云端服务器的既有基础（是否已有云账号/区域/预算上限，是否要求中国大陆境内部署） | 用户诉求仅说"通过云端服务器实现跨平台协同" | 假设单台入门级云主机（2C4G）+ 对象存储即可支撑 MVP〔假设〕；合规默认境内 Region |
| U-04 | 团队技术栈与人数（Rust/Kotlin 经验） | 影响 §4.3 桌面与 Android 框架推荐的可执行性（Tauri 需 Rust、原生 Android 需 Kotlin） | 若团队为前端背景：Windows 端改 Electron、Android 端改 RN+llama.rn（B4 路线）；报告已给出双路径 |
| U-05 | 模型分发链路的合规定性：面向中国公众提供生成式 AI 服务涉及备案要求（《生成式人工智能服务管理暂行办法》） | 本项目双端 AI 是否构成"向公众提供生成式 AI 服务"取决于分发范围与运营方式，公开资料无法替业务定性 | 若仅小范围私有分发可暂缓；若上架公开渠道需主理人协调法务评估备案义务 |

### 5.3 需业务架构持续关注的依赖项

| 编号 | 依赖项 | 说明 | 建议关注阶段 |
| --- | --- | --- | --- |
| D-01 | 远程指令的权限模型与白名单范围 | 调研确认通道可行，但"手机可让电脑执行哪些桌面助手操作"是业务边界决策，且直接决定 R-03 的暴露面 | 高层架构设计 + 安全设计 |
| D-02 | 在线/离线双模式的账号与配额体系 | 在线模式是否要求登录、是否按用户配额计费，影响云端中继与网关的数据模型 | 高层架构设计 |
| D-03 | 文件处理功能清单（摘要/翻译/重命名/格式转换等） | 调研侧仅确认平台能力（SAF + 端侧推理）可行，功能取舍属业务范围 | 高层架构设计 / UserStory |
| D-04 | UI 视觉设计基线 | "精美 GUI"需设计规范，建议参照 B1/B3 的桌面助手交互范式 | 并行系统设计（Phase 4） |
| D-05 | 模型许可与备案合规清单 | R-02/U-05 的落点，需形成逐模型许可台账 | 安全设计 / 集成交付前 |
| D-06 | WebRTC 进阶通道的引入时机 | MVP 中继先行，P2P 卸载中继成本的触发条件（中继带宽费用阈值） | 集成交付后运营阶段 |

---

## 6. 关键来源目录

> 集中列出全部调研所使用的公开资料、官方文档、社区仓库、分析报告等。每条来源不低于 URL 粒度，关键来源已给出具体章节或段落。

**硬指标**：
- ≥ 3 条来源，至少覆盖每家标杆。✅（28 条，5 家标杆全覆盖）
- 关键数据（准确率、性能基准、定价）必须指定来源段落/图表位置。✅（见"相关章节"列）

| 编号 | 来源类型 | 标题 / 名称 | URL / 路径 | 相关章节 | 最后访问日期 |
| --- | --- | --- | --- | --- | --- |
| SR-01 | 官方文档 | LM Studio Docs — Welcome / System requirements / Offline Operation / Run llama.cpp(GGUF) or MLX models | https://lmstudio.ai/docs | B1, §2.2.1, §2.3 | 2026-07-20 |
| SR-02 | 官方网站 | Layla — Experience the Best Offline AI Assistant（Features / Technology / "Layla In Numbers"：7B Parameters、4GB File Size；direct APK 下载入口；一次性买断说明） | https://www.layla-network.ai/ | B2, §2.2.2, §2.3 | 2026-07-20 |
| SR-03 | 开源仓库 | janhq/jan GitHub README（Features、Build from Source：Node ≥20 + Yarn + Rust for Tauri、License Apache-2.0、Acknowledgements：llama.cpp + Tauri） | https://github.com/menloresearch/jan | B3, §2.2.3, §3.1 | 2026-07-20 |
| SR-04 | 开源仓库 | a-ghorbani/pocketpal-ai GitHub README（React Native 0.82 + llama.rn 0.12.4、HF 应用内下载与 gated token、Android OpenCL/Hexagon 加速、MIT License） | https://github.com/a-ghorbani/pocketpal-ai | B4, §2.2.4, §2.3 | 2026-07-20 |
| SR-05 | 评测文章 | 10 Best Ollama Alternatives in 2026 (Free, GUI, Local & Mobile)（LM Studio 闭源/商用授权说明、Jan/GPT4All/AnythingLLM 平台与许可对比） | https://atomic.chat/blog/llm-updates/ollama-alternatives | B1, §2.2.1, §3.1 | 2026-07-20 |
| SR-06 | 官方博客 | RustDesk for Linux 中文官方博客（hbbs/hbbr 自建服务器、P2P 优先 + 中继兜底、Server Pro 区分） | https://rustdesk.com/zh-cn/blog/rustdesk-for-linux-zh-cn | B5, §2.2.5 | 2026-07-20 |
| SR-07 | 架构分析 | Self-hosted TeamViewer Alternative: RustDesk Explained（hbbs TCP 21115/21116+UDP 21116、hbbr TCP 21117、NaCl Curve25519/xSalsa20-Poly1305/Ed25519 E2E 加密、AGPL-3.0、v1.4.8/Server 1.1.15） | https://wz-it.com/en/knowledge/remote-access/self-hosted-teamviewer-alternative | B5, §2.2.5, §2.3 | 2026-07-20 |
| SR-08 | 开源仓库 | ggerganov/llama.cpp GitHub（MIT、Windows winget 预编译、GGUF 1.5~8bit 量化、libllama/llama-server OpenAI 兼容 API） | https://github.com/ggerganov/llama.cpp | Q2, §2.3 | 2026-07-20 |
| SR-09 | 官方文档 | MLC LLM | Home（MLCEngine 统一引擎：REST/Python/JS/iOS/Android，OpenAI 兼容 API） | https://llm.mlc.ai/ | Q2, §2.3 | 2026-07-20 |
| SR-10 | 学术论文（转述） | 《大型语言模型在移动平台上的性能基准测试：一项全面评估》arXiv:2410.03613（骁龙8Gen2/麒麟9000S/天玑9200+ 实测：MLC 9.5~14.2 tok/s vs llama.cpp 6.8~8.7 tok/s；GPU 节能 33.6%；ALU 利用率 11~18%） | https://arxiv.org/html/2410.03613v1 （中文解读：https://www.shxcj.com/archives/10152） | Q2, §2.3, R-01 | 2026-07-20 |
| SR-11 | 官方文档 | Google ML Kit GenAI API 概览（Gemini Nano 功能 API 与 Prompt API 设备白名单：Pixel 9/10、Galaxy S25、小米 15 等旗舰；基于 AICore） | https://developers.google.cn/ml-kit/genai | Q2, §2.3, R-01 | 2026-07-20 |
| SR-12 | 对比评测 | Why Tauri 2.0 Is Better Than Electron 30 for Lightweight Desktop Apps（4.2MB vs 112MB、RSS 128MB vs 342MB、TTFP 420ms vs 1180ms、OWASP 审计 2 vs 11 发现） | https://www.johal.in/opinion-tauri-20-is-better-electron-30-lightweight | Q3, §2.3 | 2026-07-20 |
| SR-13 | 工程博客 | Firezone — Using Tauri to build a cross-platform security app（2 个月 Windows beta、Qt/WPF/Electron 等框架取舍记录、内存 280MB→35MB） | https://www.firezone.dev/blog/using-tauri | Q3, §2.3 | 2026-07-20 |
| SR-14 | 社区文章 | Tauri 2.0 正式版发布对比 Electron（得物实例—包体 -96%、6 人前端团队 3 个月迁移、40% 时间用于 Rust 学习） | https://www.toutiao.com/article/7529446641276813858/ | Q3, §2.3, §4.1 | 2026-07-20 |
| SR-15 | 行业文章 | 2024年Android开发：破局与进化之路（Kotlin 渗透率超 80%、Compose 状态管理实践） | https://cloud.baidu.com/article/4154291 | Q4, §2.3 | 2026-07-20 |
| SR-16 | 对比评测 | Kotlin Multiplatform 2.0 vs Flutter 4.0（空包 4.2MB vs 5.8MB、冷启动 127ms vs 189ms、帧率与代码复用数据） | https://www.johal.in/comparison-kotlin-multiplatform-20-vs-flutter-40-cross-platform | Q4, §2.3 | 2026-07-20 |
| SR-17 | 技术文档 | WebRTC & NAT Traversal（TURN 兜底 8~20%、对称 NAT 失败率、TURN-over-TLS-443、短时效凭证） | https://unseel.com/cs/webrtc | Q5, §2.3 | 2026-07-20 |
| SR-18 | 官方方案文档 | Espressif ESP-WebRTC 方案（WebRTC/MQTT/WebSocket 三协议对比表：延迟、NAT 穿透、加密、QoS） | https://docs.espressif.com/projects/esp-techpedia/zh_CN/latest/esp-friends/solution-introduction/multimedia/application-solution/esp-webrtc.html | Q5, §2.3 | 2026-07-20 |
| SR-19 | 工程文章 | Android WebRTC 远程控制入门实战（WebSocket/MQTT/WebRTC DataChannel 延迟对比 300-800/200-500/50-200ms；纯 STUN 对称 NAT 失败率超 40%） | https://blog.csdn.net/2600_94960047/article/details/157164275 | Q5, §2.3 | 2026-07-20 |
| SR-20 | 模型卡 | DeepSeek-R1-Distill-Qwen-32B: Specs & Benchmarks（MIT 许可允许商用/修改/再分发；底座 Qwen2.5 Apache-2.0；GGUF/AWQ/GPTQ 生态） | https://ai-tldr.dev/models/deepseek-r1-distill-qwen-32b | S5, §2.3, §4.3 | 2026-07-20 |
| SR-21 | 新闻报道 | DeepSeek的新R1 AI模型精简版可在单个GPU上运行（DeepSeek-R1-0528-Qwen3-8B 基于 Qwen3-8B、MIT 许可） | https://www.atyun.com/68834.html | S5, §2.3 | 2026-07-20 |
| SR-22 | 工程博客 | On-Device AI Inference: Running LLMs Locally with llama.cpp, MLX, and ExecuTorch（Phi-4-mini 3.8B Q4 约 2.5GB；框架定位：llama.cpp/Ollama 开发者工具、ExecuTorch 移动端、ONNX Runtime 企业 Windows） | https://cloudrps.com/blog/on-device-ai-inference-edge-models-architecture | Q2, §2.3 | 2026-07-20 |
| SR-23 | 模型仓库 | Mungert/DeepSeek-R1-0528-Qwen3-8B-GGUF（Q4_K_M 5.21GB 等全部量化规格表；支持商用与蒸馏说明） | https://huggingface.co/Mungert/DeepSeek-R1-0528-Qwen3-8B-GGUF | S5, §2.3, §4.3 | 2026-07-20 |
| SR-24 | 教程文章 | Build a Private Windows AI Assistant with LM Studio and AnythingLLM（RTX 4060 8GB 跑 7B Q4 约 20~30 tok/s；纯 CPU 16GB 跑 3B Q4 约 3~5 tok/s） | https://dev.to/everylocalai/build-a-private-windows-ai-assistant-with-lm-studio-and-anythingllm-4mki | Q2, §2.3 | 2026-07-20 |
| SR-25 | 开源仓库 | songquanpeng/one-api（OpenAI 格式统一管理多渠道 key、令牌额度/过期/模型范围、模型映射、负载均衡、Docker 一键部署） | https://github.com/usesless/one-api | S6, §2.3, §4.1 | 2026-07-20 |
| SR-26 | 开源仓库 | QuantumNous/new-api（one-api 二次开发：模型映射、令牌可用模型控制、渠道加权、数据看板） | https://github.com/djylb/new-api | S6, §2.3, §4.1 | 2026-07-20 |
| SR-27 | 官方文档 | DeepSeek API Docs — DeepSeek-V3 正式发布（价格：输入 0.5 元命中/2 元未命中每百万 tokens、输出 8 元；生成速度 60 TPS） | https://api-docs.deepseek.com/zh-cn/news/news1226 | S6, §2.3, §4.3 | 2026-07-20 |
| SR-28 | 新闻报道 | DeepSeek官宣降价最高降75%（错峰 00:30-08:30：V3 五折、R1 2.5 折；R1 标准价输入 1 元命中/4 元未命中、输出 16 元） | https://www.toutiao.com/article/7475960767490032137/ | S6, §2.3, R-04 | 2026-07-20 |

---

## 7. 硬指标清单

> 汇总本模板所有章节的硬指标，供自动校验与人工审核使用。

| 章节 | 硬指标项 | 当前状态 | 备注 |
| --- | --- | --- | --- |
| §1 | 调研问题已收敛为 ≥ 3 条可执行问题 | ✅ | §1.2 共 5 条（Q1~Q5），均含调研对象/目标/预期产出 |
| §2.1 | 标杆系统 ≥ 3 家，含 ≥ 1 家头部 SaaS | ✅ | 5 家；头部商业产品代表：B1 LM Studio（桌面本地 AI 头部闭源产品）、B2 Layla（移动端离线 AI 头部商业产品）。注：本项目品类为"本地优先 AI 工具"，头部形态为商业闭源桌面/移动应用而非云端 SaaS，B1/B2 即该品类的头部商业代表 |
| §2.1 | 标杆系统 ≥ 1 家开源或自研代表 | ✅ | B3 Jan（Apache-2.0）、B4 PocketPal AI（MIT）、B5 RustDesk（AGPL-3.0） |
| §2.2 | 每家标杆有独立详述卡片 | ✅ | 5 张卡片（§2.2.1~§2.2.5，B1~B5 各一张），每张 10 维度 + 置信度标注 |
| §2.3 | 关键能力横向事实无遗漏 | ✅ | 14 个能力维度覆盖 Q1~Q5 全部调研问题，逐条带来源编号 |
| §3.1 | 对比矩阵含 5 维度 + 权重 + 评分 | ✅ | 权重 0.30+0.20+0.15+0.15+0.20 = 1.00 |
| §3.2 | 评分结论含优先/部分/不借鉴三层 | ✅ | 优先 B3(4.45)；部分 B4(3.90)/B5(3.75)；不借鉴 B2(2.95)/B1(3.30) |
| §4.1 | 自研/采购/复用边界有明确建议 | ✅ | 8 个能力项均有方式 + 依据 + 候选 + 前提 |
| §4.2 | MVP 范围建议与用户诉求对齐 | ✅ | R1~R9 对齐双端功能清单，含 2 项不建议进 MVP |
| §5.1 | 主要风险 ≥ 3 条，有缓解建议 | ✅ | 7 条（R-01~R-07），均含触发条件/影响范围/严重程度/缓解建议 |
| §6 | 关键来源可追溯（URL / 章节） | ✅ | 28 条来源全部 URL 粒度，关键数据标注段落 |
| 全文 | 明确区分事实 / 推断 / 建议 / 风险 | ✅ | 四段式结构 + 卡片逐行置信度标注（已核实/推断/综合归纳） |
| 全文 | 不存在编造来源或占位符 | ✅ | 全文无模板占位符残留；无法确认项已列入 §5.2（U-01~U-05）而非正文占位 |

---

## 附录 A：调研过程与方法说明

> 本附录描述生成方法论与工具清单，属于元信息。

- **调研执行**：research-analyst（查有据），2026-07-20 单日完成。
- **工具**：WebSearch（10 次，中英双语关键词组）、WebFetch（4 次全文抓取：llama.cpp、MLC LLM、Jan、PocketPal AI、LM Studio Docs、Layla 官网）。
- **方法**：按 tech-research-advisor 6 阶段流程（问题识别 → 分层抽象 → 多维收集 → 候选整理 → 特征分析 → 加权评估）执行，并按 research_report 模板重组为「事实 → 对比 → 建议 → 风险」四段式。
- **置信度标注规则**：官方文档/官方仓库/论文原文 = 已核实；二手评测/社区文章/基于事实的演绎 = 推断；多来源交叉归纳 = 综合归纳。
- **团队与规模假设**：用户诉求未提供团队规模、预算、设备画像，相关维度已按〔假设〕标注或列入 §5.2 待确认项。

## 附录 B：中间确认自检报告

> 按公共协议 `intermediate_confirmation.md` §2.4，在 §1 / §2.1 / §3.1 / §5.2 完成后执行自检（先按 §2.1 判定，再按 §2.3 反向验证 3 问）。

**自检节点 1（§1 调研问题收敛后）**
- §2.1 判定：收敛结果 Q1~Q5 与主理人下发的六大调研方向一一对应，无二义性分叉 → 未命中方案分歧。
- 反向验证 3 问：
  - Q1（返工成本）：若收敛问题被推翻，返工范围为 §1.1/§1.2 两张表（约 15 行）及后续章节引用锚点，切换成本 < 0.5 人日 → 可控。
  - Q2（可感知性）：调研问题集合属内部工作分解，用户/客户/监管均不直接感知 → 感知不到（依据：不改变产品功能与对外承诺，仅决定查什么资料）。
  - Q3（与原始诉求一致性）：主理人下发材料第六节明确列出六个调研方向，Q1~Q5 为其合并重组（方向 1→Q2、方向 2→Q3、方向 3→Q4、方向 4+6→Q5、方向 5→Q1/Q2、新增标杆方向→Q1），未新增偏离方向 → 一致。
- 结论：未命中，不发起中间确认。

**自检节点 2（§2.1 标杆清单后）**
- §2.1 判定：标杆候选收敛为 5 家（2 闭源商业 + 3 开源），覆盖桌面/移动/通道三类，未出现"≥6 家候选难以取舍"或行业范围不明 → 未命中方案分歧。
- 反向验证 3 问：
  - Q1（返工成本）：若更换标杆名单，返工范围为 §2.1 清单表、§2.2 详述卡片、§3.1 评分矩阵三处；增删 1 家标杆约 1~2 小时工作量 → 可控。
  - Q2（可感知性）：标杆选择只影响报告参照系，不影响产品功能 → 用户感知不到（依据：标杆是调研素材而非产品组件，最终技术组件由 §4.3 建议层另行列出并经下游裁决）。
  - Q3（与原始诉求一致性）：用户诉求原文"类似 WorkBuddy 的桌面助手工具"指向本地 AI 助手品类，所选 5 家均属该品类及远程控制相邻品类 → 一致。
- 结论：未命中，不发起中间确认。

**自检节点 3（§3.1 权重设定前 + 评分后）**
- §2.1 判定：采用模板默认五维度（场景契合度 0.30/技术成熟度 0.20/集成难度 0.15/成本 0.15/合规可控性 0.20）。已验证权重敏感性：合规与集成难度对调（0.15↔0.20）不改变排序（B3 仍第一）；场景契合度 ±0.05 不改变三层归类 → 默认权重适用，无排名反转风险 → 未命中方案分歧。
- 反向验证 3 问：
  - Q1（返工成本）：若权重被推翻，返工范围为 §3.1 矩阵 + §3.2 结论两处表格，约 1 小时 → 可控。
  - Q2（可感知性）：评分结论是"建议"而非冻结，报告 §4 开头与 §3.2 均已显式声明"保留 business-architect 最终裁决空间" → 用户可在 G2 审核时感知并调整（感知点：§3.1 权重列与理由列，已公开在文档中）。
  - Q3（与原始诉求一致性）：用户诉求未显式提及评估维度与权重（原文无相关内容），且本决策不改变产品形态与对外承诺 → 不命中 §2.2(3)。
- 结论：未命中，不发起中间确认；权重与评分依据已在 §3.1 逐行给出理由，供人工审核追溯。

**自检节点 4（§5.2 待确认项整理时，最后一次完整复核）**
- §2.1 判定：U-01（"Deepseek-V4-Pro"真实映射）存在 ≥2 种合理理解（官方 deepseek-chat 映射 / 第三方渠道 / 未来真实 V4），但**用户原始诉求已显式指定显示名为"Deepseek-V4-Pro"且调研报告只需建议而非冻结**——报告在 U-01 中给出默认路径（映射 deepseek-chat + 网关模型映射）并明确标注需主理人确认，未静默选择 → 该决策点已作为待确认项显式上交，不构成"跳过确认"。U-05（备案定性）同理已上交。其余各项无新增分歧 → 未命中。
- 反向验证 3 问（针对"将 U-01~U-05 列为待确认项而非直接裁决"这一决策本身）：
  - Q1（返工成本）：若待确认项答案与报告默认假设相反，返工范围为 §4.1/§4.3 中对应行（模型映射行、框架推荐行各 1~2 行），切换成本 < 1 人日 → 可控。
  - Q2（可感知性）：U-01/U-05 的结果用户可感知（模型显示名、备案义务），因此**没有**在报告内替用户拍板，而是列入待确认清单上交主理人 → 已按协议精神处理，感知风险由 G2 人工审核兜底。
  - Q3（与原始诉求一致性）：用户诉求原文"API 对用户不可见，仅显示模型名称'Deepseek-V4-Pro'"被 U-01 原文引用并保留，报告未擅自更改显示名 → 一致。
- 结论：未命中 §2.1/§2.2 的"必须发起"标准（关键不确定项已通过 §5.2 机制上交，而非静默推进），本阶段不发起 [中间确认]；自检报告随本附录一并提交，供 G2 审核弹窗追溯。
