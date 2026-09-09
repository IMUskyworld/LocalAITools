# LocalMind × LocalFile 双端协同智能工具

> **自包含开发文档**——本 README 整合了项目全部关键信息（架构、功能、接口、环境、部署、安全、FAQ），目标是让 Claude Code / Codex **无需查阅任何其他资料即可完成全流程开发**。
>
> 项目性质：校级大学生项目（课程作业/大创级别），非商业化产品，≤ 50 用户小范围分发。

---

## 目录

1. [项目简介](#1-项目简介)
2. [系统架构](#2-系统架构)
3. [技术栈与版本](#3-技术栈与版本)
4. [功能说明](#4-功能说明)
5. [工程结构](#5-工程结构)
6. [环境配置](#6-环境配置)
7. [安装步骤](#7-安装步骤)
8. [使用方法](#8-使用方法)
9. [接口契约速查](#9-接口契约速查)
10. [数据存储设计](#10-数据存储设计)
11. [安全基线](#11-安全基线)
12. [部署运维](#12-部署运维)
13. [验收标准速查](#13-验收标准速查)
14. [常见问题 FAQ](#14-常见问题-faq)

---

## 1. 项目简介

### 1.1 一句话定位

一台电脑 + 一部手机即可获得"随时随地可调用的个人 AI 助手"——**断网可用**（端侧推理）、**跨端可控**（手机对话式操控电脑）、**隐私可守**（文件与对话可全程不出设备）。

### 1.2 三组件

| 组件 | 平台 | 形态 | 职责 |
|------|------|------|------|
| **LocalMind** | Windows 10 1809+ | exe 安装包 | 桌面 AI 助手：在线/离线双模对话、桌面助手指令执行（白名单+二次确认） |
| **LocalFile** | Android 10+ | apk 安装包 | 文件 AI 处理器 + 远程控制器：文件摘要/翻译/重命名、在线/离线双模对话、对话式远程操控电脑 |
| **RelayCloud** | 云端（单台 2C4G VPS） | Docker Compose | 云端中继（WSS 指令中转）+ API 网关（key 隐藏/模型映射/令牌额度） |

### 1.3 核心特性

- **在线/离线双模 AI**：在线经网关调云端大模型（API 对用户不可见，仅显示 **Deepseek-V4-Pro**）；离线一键下载本地大模型（llama.cpp 端侧推理），断网可用
- **对话式远程控制**：手机经云端中继向电脑发自然语言指令（"整理下载文件夹"），五态状态机反馈（已发送→已送达→执行中→已完成/失败）；危险操作本机弹窗二次确认
- **精美 GUI 双端**：Windows 端 Fluent 风（Tauri Web 技术栈）、Android 端 Material 3（Jetpack Compose）

### 1.4 关键量化指标（已冻结，验收以此为准）

| 指标 | 目标值 | 说明 |
|------|--------|------|
| 远程指令端到端时延 | P95 ≤ 3s | WSS 中继链路，不含模型生成耗时 |
| 设备配对建立时长 | ≤ 60s | 扫码/输码全流程 |
| 离线推理吞吐 | PC ≥ 5 tok/s；Android ≥ 8 tok/s | 基准：16GB 无独显 PC / 骁龙 8 系旗舰 |
| 安装包体积 | exe ≤ 80MB；apk ≤ 60MB | 不含模型文件 |
| 冷启动时长 | ≤ 3s | 双端 |
| 离线模型下载成功率 | ≥ 95% | 断点续传 + SHA256 校验 |
| 演示连接可靠性 | 10 次配对+连接成功 ≥ 9 次 | 课程演示场景 |
| 云端月成本 | ≤ 150 元 | ≤ 50 用户（云主机+对象存储+DeepSeek API） |
| 核心路径操作步数 | AI 对话 ≤ 3 步；远程控制（已配对）≤ 2 步 | 从启动 App 起算 |

---

## 2. 系统架构

### 2.1 总体架构（三层）

```
┌──────────────────── 接入层（用户/触点）────────────────────┐
│  LocalMind 桌面端 (exe · Tauri GUI)                        │
│  LocalFile 手机端 (apk · Compose GUI)                      │
│  运维触点 (云控制台 + new-api 看板)                          │
└──────────────────────────┬─────────────────────────────────┘
                           │
┌──────────────────────────▼──────────── 业务能力层 ─────────┐
│  M1 对话引擎（双模切换·流式会话）                           │
│  M2 模型管理（检测·下载·启停）                              │
│  M3 文件AI处理（摘要·翻译·重命名）                          │
│  M4 远程控制（配对·指令·白名单·回传）                       │
│  M5 中继调度（WSS网关·设备表·中转）                         │
│  M6 API代理（模型映射·令牌管控）                            │
└──────────────────────────┬─────────────────────────────────┘
                           │
┌──────────────────────────▼──────────── 基础能力层 ─────────┐
│  llama.cpp 端侧推理（Windows/Android）                     │
│  离线模型权重（PC 8B / Android 4B GGUF）                   │
│  new-api 网关（渠道+令牌+看板）                             │
│  DeepSeek 官方 API（deepseek-chat）                        │
│  对象存储/CDN（模型分发直链）                               │
│  云主机 2C4G（中继与网关载体）                              │
└────────────────────────────────────────────────────────────┘
```

### 2.2 数据流向

```
双端设备 ──WSS(指令)/HTTPS+SSE(对话)──> Nginx:443(TLS终结,分流) 
                                            ├─ /ws ──> relay-server:8080 (WSS中继)
                                            └─ /v1 ──> new-api:3000 (REST网关) ──> DeepSeek API
双端设备 ──HTTPS(Range断点续传)──> CDN ──> 对象存储(模型GGUF/安装包)
relay-server ──每日02:00备份──> rclone ──> 备份桶
```

### 2.3 信任边界（安全核心）

| 边界 | 跨越协议 | 控制点 |
|------|---------|--------|
| 端侧 ↔ 公网入口 | WSS/HTTPS（TLS ≥1.2） | 证书链校验（端侧强制）+ authTicket/sk-令牌鉴权 |
| 公网入口 ↔ 云主机容器 | docker 桥接内网 | 安全组仅 443/22；Nginx 限流；8080/3000 不公网暴露 |
| 云主机 ↔ 外部依赖 | HTTPS 出向 | egress 白名单（按 IP 放通 443，域名约束落应用层）；真实 key 仅存云主机 `.env`（600 权限） |
| 双端 ↔ 本地存储 | 进程内 SQL/OS 文件 API | OS 用户目录权限/Android 沙盒；凭证走 OS 安全存储 |

---

## 3. 技术栈与版本

### 3.1 双端技术栈

| 层 | 选型 | 版本 | 选型理由 |
|----|------|------|---------|
| **Windows 框架** | Tauri + React + TypeScript | Tauri 2.x；React 18.3；TS 5.5 | 安装包 ≤80MB（Tauri 复用系统 WebView2，包体 10~20MB 级 vs Electron 捆绑 Chromium 100MB+）；参照开源标杆 Jan |
| **Windows 后端核心** | Rust | 1.80+ (stable) | Tauri 原生语言；llama.cpp FFI、WSS 客户端、文件操作均有成熟 crate；与云端 relay-server 同语言 |
| **Android 框架** | Kotlin + Jetpack Compose | Kotlin 2.0；Compose BOM 2024.09 | 文件处理强依赖 SAF/系统 API，原生阻力最小；Compose 空包 4.2MB/冷启动 127ms |
| **端侧推理引擎** | llama.cpp（GGUF） | b4xxx 稳定 tag（编译期锁定） | 双端覆盖最广、MIT 许可、GGUF 生态模型最全；Windows FFI 嵌入 / Android JNI .so |
| **Android 加速降级链** | CPU→GPU(OpenCL)→NPU(Hexagon) | — | 借鉴 PocketPal；CPU 为基线，可用则启用 GPU/NPU |
| **端侧本地存储** | SQLite + DataStore/SharedPreferences | SQLite 3.45（WAL 模式） | 会话/模型资产/文件记录/白名单配置 |

### 3.2 云端技术栈

| 层 | 选型 | 版本 | 选型理由 |
|----|------|------|---------|
| **中继服务** | Rust + tokio + axum + tokio-tungstenite | Rust 1.80+；axum 0.7；tokio 1.40 | 单实例 2C4G 内存受限，Rust 常驻内存低（100MB 以内）且 WSS 长连接性能稳定；与端侧复用信封类型 |
| **API 网关** | new-api（one-api 系） | v0.8.x 稳定版（锁 digest） | 开源、Docker 一键部署、模型映射 + 令牌额度/速率管控（恰好覆盖"隐藏 key + 显示 Deepseek-V4-Pro"） |
| **反向代理** | Nginx | 1.24（容器化） | 443 唯一入口；TLS 终结；WSS/REST 分流；IP 限流初筛 |
| **云端数据库** | SQLite | 3.45（WAL 模式） | 数据量极小（万行级）；单文件 200MB 以内；随 relay-server 部署 |
| **容器编排** | Docker + docker-compose | Engine 24+ / compose v2 | 单台云主机轻量编排 |
| **备份** | rclone + cron | rclone 1.67 | 每日 02:00 SQLite .backup → gzip → 上传备份桶 |

### 3.3 模型选型

| 端 | 模型 | 规格 | 许可 |
|----|------|------|------|
| Windows（离线默认） | DeepSeek-R1-0528-Qwen3-8B Q4_K_M | 约 5.2GB | MIT（可再分发） |
| Android（离线默认） | Qwen3-4B 或 R1-Distill-Qwen-7B Q4 | 约 4GB | Apache-2.0 / MIT |
| 在线（网关映射） | deepseek-chat（DeepSeek 官方 API） | 输入 2 元/输出 8 元每百万 tokens；错峰 00:30-08:30 五折 | 按量计费 |

> ⚠️ **重要**：客户端仅显示 **Deepseek-V4-Pro**（品牌化显示名），真实模型名 `deepseek-chat` 经 new-api 网关模型映射，客户端零接触。

---

## 4. 功能说明

### 4.1 功能清单（F1~F14）

| 编号 | 模块 | 功能 | 优先级 | MVP |
|------|------|------|--------|-----|
| F1 | 对话引擎 | LocalMind 在线对话（SSE 流式，显示 Deepseek-V4-Pro，多会话+历史） | P0 | ✅ |
| F2 | 对话引擎 | LocalMind 离线对话（llama.cpp 本地推理，tok/s 实时显示） | P0 | ✅ |
| F3 | 模型管理 | 模型一键下载（断点续传+SHA256 校验+进度可视）+ 模式切换 + 推理启停 | P0 | ✅ |
| F4 | 远程控制 | 桌面助手指令执行器（白名单动作 + 危险动作本机二次确认 + 日志留痕） | P0 | ✅ |
| F5 | 文件处理 | SAF 文件浏览选择（文档/图片/文本，最近列表，多选） | P0 | ✅ |
| F6 | 文件处理 | 文件 AI 处理（摘要/中英翻译/批量重命名建议，结果导出/分享/复制） | P0 | ✅ |
| F7 | 对话引擎 | LocalFile 在线对话（同 F1 链路） | P0 | ✅ |
| F8 | 模型管理 | LocalFile 离线对话与模型管理（设备 RAM/SoC 检测，不达标引导在线） | P0 | ✅ |
| F9 | 远程控制 | 设备配对（6 位配对码 5 分钟有效 + 二维码，设备列表管理） | P0 | ✅ |
| F10 | 远程控制 | 对话式指令通道（WSS 中继 + 五态状态机 + 结果文本回传） | P0 | ✅ |
| F11 | 中继调度 | RelayCloud 中继服务（WSS 网关 + 设备在线状态 + 指令路由） | P0 | ✅ |
| F12 | API代理 | 网关配置与令牌管控（模型映射 + 按设备签发令牌 + 用量看板） | P1 | ✅ |
| F13 | 模型管理 | 多档模型智能推荐（按设备画像 1.5B/3B/4B/7B/8B + 基准测速页） | P1 | 完整版 |
| F14 | 远程控制 | P2P 通道（WebRTC DataChannel，中继兜底）+ 桌面截图单帧回传 | P2 | 完整版 |

### 4.2 LocalMind 页面与交互控件（Windows 端）

#### P-W1 主对话页
- **顶部栏**：App Logo 与名称；`模型模式切换器`（分段控件：☁ 在线 Deepseek-V4-Pro / ⛅ 离线本地模型，当前态高亮）；`连接状态指示灯`（绿=在线已连通 / 黄=离线模式 / 红=网关异常，悬停显示详情）
- **左侧会话栏**：`+ 新对话`按钮；会话列表（标题+时间，右键重命名/删除）；`搜索会话`输入框
- **中部对话区**：消息气泡（用户右/AI 左，AI 消息带模型名小字标注）；流式打字机渲染；`停止生成`按钮（生成中显示）；代码块`复制`按钮；`重新生成`按钮
- **底部输入区**：`消息输入框`（多行，Enter 发送 / Shift+Enter 换行）；`发送`按钮；`快捷指令`按钮（展开预设面板：总结剪贴板/打开应用/整理桌面文件等）；离线模式显示`当前速度: x.x tok/s`小字
- **状态栏**：当前模型名、内存占用（离线）、网关延迟（在线）

#### P-W2 模型管理页
- `当前模式`卡片：在线/离线单选切换 + 状态说明
- 离线模型卡片：模型名、大小 5.2GB、许可标识（MIT）；`一键下载`按钮 / 下载中`进度条+百分比+剩余时间+暂停/继续+取消` / 已下载`校验通过 ✓` + `删除模型`按钮
- 推理服务卡片：`启动本地推理`/`停止`按钮；服务状态（运行中/已停止+端口）；`推理参数`折叠面板（线程数滑块、上下文长度下拉 2048/4096/8192）
- `存储位置`行：模型目录路径 + `更改`按钮 + `打开文件夹`按钮

#### P-W3 远程控制面板页
- 配对卡片：`生成配对码`按钮；6 位大字号配对码 + 二维码 + 倒计时（5:00）；`复制配对码`按钮
- 已配对设备列表：设备名、机型、最近在线；`重命名`/`解绑`按钮
- 指令执行记录列表：时间、来源设备、指令内容、状态（成功/失败/被拦截）；`清空记录`按钮
- 安全设置：`远程控制总开关`；`危险操作需本机确认`开关（默认开）；白名单动作勾选项列表
- 二次确认弹窗（被远程触发时本机弹出）：指令内容大字 + `允许执行`/`拒绝` + 15 秒倒计时默认拒绝

#### P-W4 设置页
- 网络设置：网关地址（只读预置）；`测试连接`按钮 + 延迟显示
- 外观：浅色/深色/跟随系统单选；字号滑块
- 关于：版本号、模型许可文本入口、`查看开源许可`按钮、检查更新

### 4.3 LocalFile 页面与交互控件（Android 端）

#### P-A1 首页框架（底部三 Tab）
- 顶部：页面标题 + `模型模式`小胶囊（在线/离线，点击跳 P-A5）
- 底部导航：`文件`（folder 图标）、`对话`（chat 图标）、`遥控`（phone_sync 图标）；选中态高亮+标签

#### P-A2 文件处理页
- `选择文件`大按钮（唤起 SAF 系统选择器，文档/图片/文本分类 FilterChips）
- 最近文件列表：文件名、大小、时间缩略行
- 处理方式按钮组（载入文件后）：`生成摘要`/`翻译`/`智能重命名`/`提取要点`（Chip 单选）
- `开始处理`主按钮；处理中`进度指示器+取消`按钮
- 结果卡片：结果文本区（可滚动）；`复制`/`导出为 txt`/`分享`/`重新处理`按钮；重命名场景为"旧名→新名"对照列表 + `应用重命名`确认按钮

#### P-A3 对话页
- 同桌面端对话交互的移动版：消息列表、输入框+`发送`、模式切换入口（顶部胶囊）、生成中`停止`按钮、tok/s 速度小字（离线时）

#### P-A4 远程控制页
- 未配对态：`扫码配对`按钮（调相机）+ `输入配对码`按钮（6 位数字键盘输入框 + `连接`按钮）
- 已连接态：顶部设备卡片（电脑名、在线绿点、中继延迟 ms、`断开`按钮）
- 指令输入区：`指令输入框`（占位"想让电脑做什么？"）+ `发送`按钮；`快捷指令`横滑 Chip 组（打开应用/锁屏/调音量/总结剪贴板/整理下载文件夹）
- 指令状态列表（倒序）：每条指令卡片含指令文本、状态徽标（五态流转）、结果文本折叠区、失败原因红字；需本机确认时显示`等待电脑端确认…`

#### P-A5 模型与设置页
- 设备检测卡片：RAM 大小、SoC 型号、结论文案（"骁龙 8 Gen2 · 12GB RAM，可流畅运行 4B 模型 ✓" 或 "6GB RAM，建议使用在线模式"）
- 模型卡片：模型名+大小（约 4GB）+ `一键下载`/进度条+暂停继续/已下载`校验通过`+`删除`；不达标机型按钮置灰 + 降级引导 + `使用在线模式`跳转按钮
- 设置列表：网关连接状态+`重新连接`；外观；关于与开源许可入口

---

## 5. 工程结构

```
LocalAITools/
├── localmind/                       # LocalMind Windows 桌面端（Tauri 2.x）
│   ├── src/                         # React 18 + TypeScript 前端（GUI）
│   │   ├── api/                     # 接口封装（网关 HTTPS 客户端、WSS 客户端）
│   │   ├── stores/                  # 全局状态（Zustand：会话/模式/设备/下载）
│   │   ├── pages/                   # 页面：chat/ models/ remote/ settings（P-W1~P-W4）
│   │   ├── components/              # 通用组件：MessageBubble/ ModeSwitch/ ProgressBar/ ConfirmDialog
│   │   ├── hooks/                   # 自定义 Hooks（useStreamChat/ useDownload/ useRelay）
│   │   ├── types/                   # TS 类型（与 shared-contract 对齐生成）
│   │   ├── utils/                   # 工具函数
│   │   └── routes.tsx               # 路由配置
│   ├── src-tauri/                   # Rust 后端核心（端侧业务逻辑）
│   │   ├── src/chat/                # M1 对话引擎：模式路由、流式会话
│   │   ├── src/model/               # M2 模型管理：下载/校验/推理服务启停
│   │   ├── src/remote/              # M4 执行面：WSS 客户端、白名单执行器、确认弹窗
│   │   ├── src/storage/             # 端侧 SQLite 访问层
│   │   ├── src/inference/           # llama.cpp FFI 封装（llama-cpp-2 binding）
│   │   └── src/common/              # 端内公共：错误码/日志/信封类型（引用 shared-contract）
│   └── package.json / Cargo.toml
├── localfile/                       # LocalFile Android 端（Kotlin + Compose）
│   ├── app/src/main/java/com/localmind/localfile/
│   │   ├── ui/                      # Compose 页面：files/ chat/ remote/ settings（P-A1~P-A5）
│   │   ├── chat/                    # M1 对话引擎（在线 SSE 客户端 + 离线推理桥接）
│   │   ├── files/                   # M3 文件处理：SAF 访问、处理任务、导出分享
│   │   ├── model/                   # M2 模型管理：设备检测、DownloadManager 下载、校验
│   │   ├── remote/                  # M4 控制面：配对、指令发送、状态追踪
│   │   ├── inference/               # llama.cpp JNI 桥接（libllama.so）
│   │   ├── storage/                 # Room/SQLite + DataStore
│   │   └── common/                  # 端内公共：错误码/日志/信封类型（引用 shared-contract）
│   └── build.gradle.kts
├── relay-cloud/                     # RelayCloud 云端（后端工程根目录）
│   ├── common/                      # 公共模块：DTO/Enum/Exception/Constants + 信封 Schema 校验
│   ├── relay-server/                # M5 中继服务（Rust + tokio + axum/tungstenite）
│   │   ├── src/ws/                  # WSS 连接管理、心跳、在线状态表
│   │   ├── src/pairing/             # 配对码签发/校验/核销
│   │   ├── src/router/              # 指令路由与 ACK 中转
│   │   ├── src/store/               # SQLite 访问层（sqlx）
│   │   └── src/observability/       # /healthz /readyz /metrics、结构化日志
│   ├── gateway-config/              # M6 new-api 部署配置（docker-compose.yml、渠道/映射/令牌初始化脚本）
│   └── ops/                         # 备份脚本（sqlite .backup + rclone）、恢复 SOP
├── shared-contract/                 # 三方共享契约（Shared Kernel）
│   ├── envelope/v1/                 # 指令信封 JSON Schema + TS/Rust/Kotlin 类型（由 Schema 生成）
│   ├── error-code/                  # 全局错误码注册表（单一来源，三端引用）
│   └── whitelist/                   # 白名单动作枚举与危险等级定义
└── docs/                            # 开发文档与图示源
```

> 📌 **依赖方向铁律**：业务模块依赖公共模块（`shared-contract`），**禁止反向**。

---

## 6. 环境配置

### 6.1 开发环境要求

#### Windows 端（LocalMind）
| 项 | 要求 |
|----|------|
| OS | Windows 10 1809+（开发机建议 Windows 11） |
| Rust | 1.80+（stable，`rustup` 安装） |
| Node.js | ≥ 20（前端构建） |
| 包管理 | pnpm 或 Yarn ≥ 4.5 |
| Tauri CLI | `cargo install tauri-cli --version "^2.0"` |
| 系统依赖 | WebView2 Runtime（Windows 11 自带；Windows 10 需安装，Tauri 打包时可引导） |
| C++ 构建工具 | Visual Studio Build Tools（MSVC，Rust 编译 llama.cpp 需要） |

#### Android 端（LocalFile）
| 项 | 要求 |
|----|------|
| Android Studio | Hedgehog 2023.1.1+ |
| JDK | 17 |
| Android SDK | API 34（compileSdk）；minSdk 29（Android 10） |
| Kotlin | 2.0 |
| Android NDK | r26+（编译 llama.cpp JNI .so 需要） |
| CMake | 3.22+（llama.cpp Android 构建） |

#### 云端（RelayCloud）
| 项 | 要求 |
|----|------|
| 云主机 | 2 vCPU / 4GB 内存 / 60GB SSD / 5Mbps（境内 Region，轻量应用服务器或 CVM 入门型） |
| OS | Ubuntu 22.04 LTS |
| Docker | Engine 24+ |
| docker-compose | v2 |
| 对象存储 | 云厂商对象存储（S3 兼容）：模型桶 ≈ 10GB + 备份桶 ≈ 5GB |
| CDN | 支持 Range 请求（断点续传必需） |
| 域名+证书 | 1 个域名（A 记录指向 EIP）+ 免费 DV 证书（Let's Encrypt 或云厂商） |

### 6.2 外部依赖账号

| 依赖 | 用途 | 获取 |
|------|------|------|
| DeepSeek API key | 在线模型供应（仅存云主机 `.env`，客户端零接触） | https://platform.deepseek.com 注册充值 |
| 云厂商账号 | VPS + 对象存储 + CDN | 任选主流云厂商（学生优惠更佳） |
| 模型权重源 | GGUF 模型下载（HF 镜像/ModelScope，经对象存储中转） | 开源模型仓库 |

---

## 7. 安装步骤

### 7.1 克隆与初始化

```bash
git clone <repo-url> LocalAITools
cd LocalAITools
```

### 7.2 RelayCloud 云端部署（先部署云端，双端才能联调）

```bash
cd relay-cloud

# 1. 配置 L4 密钥（.env 权限 600，root 属主，不入 git）
cp gateway-config/.env.example gateway-config/.env
# 编辑 .env 填入：DEEPSEEK_API_KEY / HMAC_SECRET / NEWAPI_ADMIN / RCLONE_AKSK
chmod 600 gateway-config/.env

# 2. 启动 relay-server + new-api + Nginx（docker-compose 编排）
docker compose up -d

# 3. 初始化 new-api：配置 DeepSeek 渠道 + 模型映射（Deepseek-V4-Pro → deepseek-chat）+ 创建令牌分组
bash gateway-config/init-gateway.sh

# 4. 健康检查
curl http://localhost:8080/healthz   # relay-server
curl http://localhost:3000/api/status # new-api
```

### 7.3 LocalMind Windows 端构建

```bash
cd localmind

# 1. 安装前端依赖
pnpm install

# 2. 配置预置参数（网关地址、模型清单、显示名——不含真实 key）
cp src-tauri/config.example.json src-tauri/config.json
# 编辑 config.json：GATEWAY_HOST=wss://relay.example.com/ws；API_BASE=https://relay.example.com/v1

# 3. 开发模式运行
pnpm tauri dev

# 4. 打包 exe 安装包（含 WebView2 引导）
pnpm tauri build
# 产物：src-tauri/target/release/bundle/nsis/*.exe（≤ 80MB 校验）
```

### 7.4 LocalFile Android 端构建

```bash
cd localfile

# 1. 配置预置参数（同 LocalMind，不含真实 key）
# 编辑 app/src/main/assets/config.json：GATEWAY_HOST / API_BASE

# 2. 编译 llama.cpp JNI（libllama.so，需 NDK + CMake）
./gradlew :inference:buildNative

# 3. 开发模式安装到设备
./gradlew installDebug

# 4. 打包 release apk（需签名 keystore 配置到 CI Secret 或本地 keystore.properties）
./gradlew assembleRelease
# 产物：app/build/outputs/apk/release/*.apk（≤ 60MB 校验）
```

### 7.5 离线模型准备（首次使用离线模式）

- 方式一（推荐）：应用内一键下载——模型管理页点`一键下载`，经对象存储/CDN 直链（断点续传 + SHA256 校验）
- 方式二（运维预置）：从 HF 镜像/ModelScope 手动下载 GGUF，经对象存储上传后分发
  - Windows：DeepSeek-R1-0528-Qwen3-8B Q4_K_M（约 5.2GB）
  - Android：Qwen3-4B 或 R1-Distill-Qwen-7B Q4（约 4GB）

---

## 8. 使用方法

### 8.1 快速开始（演示流程）

1. **启动云端**：确认 relay-server + new-api + Nginx 三容器运行（`docker compose ps`）
2. **安装双端**：Windows 装 exe、Android 装 apk，首启默认在线模式
3. **配对**：电脑端 P-W3 点`生成配对码` → 手机端 P-A4 点`扫码配对`（或输 6 位码）→ ≤ 60s 完成绑定
4. **三大场景演示**：
   - PC 对话：P-W1 切换在线/离线模式对话
   - 手机文件处理：P-A2 选文件 → 选处理方式 → 导出/分享
   - 手机远程控制：P-A4 输入"总结剪贴板" → 观察五态状态机 → 电脑端执行并回传结果

### 8.2 核心操作路径

#### 场景一：PC 在线/离线对话与切换
打开 LocalMind → P-W1 顶部`模型模式切换器`选择在线（Deepseek-V4-Pro）或离线（本地模型）→ 输入框输入问题 → 流式输出。切换 ≤ 2 次点击，会话不丢失。

#### 场景二：离线模型一键下载
P-W2（或 P-A5）→ 设备检测（Android）→ `一键下载` → 进度条+断点续传 → SHA256 校验 → `启动本地推理` → 离线对话可用。不达标 Android 机型按钮置灰 + 引导在线模式。

#### 场景三：手机文件 AI 处理
P-A2 `选择文件`（SAF）→ 选`生成摘要`/`翻译`/`智能重命名` → `开始处理` → 结果卡片`复制`/`导出`/`分享`。重命名场景显示"旧名→新名"对照 + `应用重命名`确认。

#### 场景四：手机远程控制电脑
P-A4（已配对）→ `指令输入框`输入"整理下载文件夹"（或点`快捷指令`Chip）→ `发送` → 观察五态流转 → 完成后展开结果文本。命中危险动作时电脑端弹窗 15 秒倒计时，允许后才执行；手机端同步显示"等待电脑端确认…"。

#### 场景五：运维巡检（兼职运维者）
云控制台看中继存活与在线设备数 → new-api 看板看令牌消耗与账单预估 → 月度核对三项账单 ≤ 150 元，超阈值收紧令牌额度或引导错峰（DeepSeek 00:30-08:30 五折）。

---

## 9. 接口契约速查

### 9.1 协议总览

| 链路 | 协议 | 端口 | 鉴权 | 说明 |
|------|------|------|------|------|
| 双端 ↔ 中继 | WSS | 443（/ws） | authTicket（HMAC）+ 配对绑定校验 | 指令通道：配对、指令信封、状态事件 |
| 双端 ↔ 网关 | HTTPS + SSE | 443（/v1） | Bearer sk-设备令牌（new-api 签发） | 在线对话：OpenAI v1 兼容 |
| 双端 ↔ CDN | HTTPS | 443 | 签名 URL（防盗链+次数限制） | 模型/安装包下载：Range 断点续传 |
| 运维 ↔ 云主机 | SSH | 22 | 密钥登录（禁密码）+ IP 白名单 + fail2ban | 全部管理动作唯一通道 |

### 9.2 WSS 指令信封（JSON，MVP 纯文本；完整版叠加 E2E 加密层）

```json
{
  "version": "v1",
  "id": "uuid-v4",
  "type": "command | ack | state | pair",
  "from_device_id": "phone-uuid",
  "to_device_id": "pc-uuid",
  "intent_text": "整理下载文件夹",
  "state": "sent | delivered | running | done | failed",
  "result_text": "执行结果或失败原因",
  "timestamp": 1716000000
}
```

**指令五态状态机**：`sent`（手机本地）→ `delivered`（中继 ACK）→ `running`（电脑 ACK）→ `done` / `failed`（含原因）。

### 9.3 配对流程

1. 电脑端向中继请求签发配对码：中继生成 6 位随机码，绑定电脑设备 ID，5 分钟 TTL + 一次性
2. 手机端输码/扫码（二维码内容为配对码字符串，不含密钥）→ 向中继提交配对码 + 本设备信息
3. 中继校验（存在性、未过期、未使用）→ 建立双向绑定关系，写入设备表
4. 双端各自持久化绑定关系；之后经 WSS 互报在线状态

### 9.4 在线对话接口（OpenAI v1 兼容）

```
POST https://relay.example.com/v1/chat/completions
Authorization: Bearer sk-设备令牌
Content-Type: application/json

{
  "model": "Deepseek-V4-Pro",   // 显示名，网关映射到 deepseek-chat
  "messages": [{"role":"user","content":"..."}],
  "stream": true                 // SSE 流式
}
```

### 9.5 全局错误码（6 位格式：`[端/模块][大类][序号]`）

| 段 | 范围 | 示例 |
|----|------|------|
| 通用 | 1xxxxx | 100001 未知错误 / 100003 网络不可达 / 100004 请求超时 |
| 对话 | 2xxxxx | 201001 网关异常 / 201002 令牌额度耗尽 / 201003 离线模型未就绪 |
| 模型 | 3xxxxx | 301001 下载失败 / 301002 校验未通过 / 301003 存储不足 / 301004 设备不达标 |
| 远程控制 | 4xxxxx | 401001 未配对 / 401002 配对码无效过期 / 402001 对端离线 / 402002 白名单外 / 402003 危险操作被拒 |
| 中继 | 5xxxxx | 501001 配对码签发失败 / 502001 指令路由失败 |
| 网关 | 6xxxxx | 601001 渠道异常 / 602001 模型映射缺失 |

> 完整错误码注册表为 `shared-contract/error-code` 单一来源，三端引用。

---

## 10. 数据存储设计

### 10.1 云端 RelayCloud 数据表（SQLite，万行级）

| 表 | 关键字段 | 量级 | 清理策略 |
|----|---------|------|---------|
| `device` | id(PK)、device_name、platform(windows/android)、last_seen_at | ≤ 100 行 | 解绑即物理删除 |
| `pairing` | id(PK)、pc_device_id(FK)、phone_device_id(FK)、active | ≤ 100 行 | 解绑置 active=0 保留 30 天 |
| `pairing_code` | code(PK，6 位)、pc_device_id、expires_at、used | ≤ 10 行 | 每 5 分钟删过期/已用 |
| `command_log` | id(PK)、from/to_device_id、intent_text、state（五态）、error_reason | ≤ 3 万行/月 | 保留 90 天 |

> new-api 令牌数据由其自带数据库存储，不在 relay-server 库重复设计。

### 10.2 端侧本地库（LocalMind SQLite / LocalFile Room）

| 表 | 关键字段 | 量级 | 说明 |
|----|---------|------|------|
| `session` | id(PK)、title、mode(online/offline)、deleted | ≤ 500 个/端 | 软删 30 天清理 |
| `message` | id(PK)、session_id(FK)、role、content、model_label | ≤ 5 万行/端 | 随会话级联删除 |
| `model_asset` | id(PK)、model_name、file_path、sha256、state、download_offset | ≤ 5 行/端 | 断点续传位点 |
| `file_record`（Android） | id(PK)、file_name、saf_uri、process_type、result_summary | ≤ 1 万行 | 保留 90 天 |
| `whitelist_config`（PC） | action_type(PK)、enabled、danger_level(normal/danger) | ≤ 20 行 | 动作枚举，用户可改 |

> 🔒 **数据本地化铁律**：会话/文件内容/执行日志仅存端侧；云端不留存用户文件与对话内容（隐私可守卖点）。

---

## 11. 安全基线

### 11.1 核心安全控制（施工红线）

| 控制项 | 实现 | 说明 |
|--------|------|------|
| **key 零入包** | 构建管线扫描安装包，0 个真实 key 与真实模型名 | CI 红线校验（V3）；真实 key 仅存云主机 `.env`（600 权限）永不离开服务器 |
| **传输加密** | 端↔云全链路 TLS ≥1.2（WSS/HTTPS）；完整版 P2P 叠加 E2E | 端侧强制证书链校验 |
| **接口鉴权** | 在线 API 走设备令牌（Bearer sk-设备级）；远程指令走配对绑定校验 | 令牌按设备签发，带额度+速率上限 |
| **指令执行防护** | 白名单校验 + 危险动作本机二次确认（15 秒超时默认拒绝）+ 执行日志留痕 | 白名单外动作直接拒绝（402002） |
| **配对码安全** | 6 位随机、5 分钟 TTL、一次性、不含密钥材料 | 过期/已用即废 |
| **密钥托管** | 不启用 KMS（成本考量）；`.env` 600 + root 属主 + 不入 git + 泄露即轮换 SOP | 生产/非生产密钥禁止跨环境共用（红线） |
| **模型完整性** | SHA256 校验下载的模型文件 | 防篡改 |
| **WAF** | 不启用独立 WAF；Nginx 基础防护承担 | client_max_body_size 1m / 超时 15s / 30 QPS/IP / 安全响应头 / server_tokens off / 非白名单路径 444 |
| **安全组** | 入站 443（0.0.0.0/0）+ 22（运维 IP 白名单）；出站按 IP 放通 443（域名约束落应用层）；其余全拒 | 主机红线：禁止 0.0.0.0/0 开放 22/23/3306/6379/8080/3000 |
| **SSH 防护** | 密钥登录（PasswordAuthentication no、PermitRootLogin no）+ IP 白名单 + fail2ban（5 次封禁 1h） | 堡垒机等价替代 |

### 11.2 STRIDE 威胁速查（重点：远程控制通道滥用）

| 威胁 | 场景 | 缓解 |
|------|------|------|
| 仿冒 | 伪造配对码/设备 | 配对码短时效+一次性+绑定校验；设备指纹 |
| 篡改 | 指令/模型文件篡改 | TLS + SHA256 校验 |
| 否认 | 否认发过指令/做过确认 | 双留痕（云端 command_log 90 天 + 端侧 execution_log 180 天）+ traceId |
| 信息泄露 | key 泄露/对话内容泄露 | key 零入包 + .env 600 + 日志脱敏 + 数据本地化 |
| DoS | CC 攻击/洪泛 | Nginx 限流 + 云厂商基础抗 DDoS + 安全组收敛 |
| 权限提升 | 危险操作越权 | 白名单 + 本机二次确认 + 15 秒超时默认拒绝 |

### 11.3 合规要点

- **模型许可**：仅分发 MIT/Apache-2.0 模型（DeepSeek-R1/Qwen3 系）；许可文本随应用分发
- **分发合规**：≤ 50 人小范围私有分发，暂缓生成式 AI 备案评估；**上架公开渠道前必须重启评估**
- **个人信息**：无账号体系（设备配对即身份），不采集个人信息；设备 ID 随机生成

---

## 12. 部署运维

### 12.1 CI/CD 流水线（GitHub Actions，8 阶段）

| 阶段 | LocalMind | LocalFile | RelayCloud |
|------|-----------|-----------|------------|
| 1. 触发 | push tag / PR | push tag / PR | push tag / PR（relay-cloud/） |
| 2. 代码检查 | ESLint + cargo clippy + fmt | ktlint + detekt + lint | cargo clippy + fmt |
| 3. 单元测试 | vitest + cargo test | JUnit + Compose UI test | cargo test |
| 4. 安全扫描 | cargo audit + npm audit | dependency-check | cargo audit + Trivy |
| 5. 构建 | `tauri build`（exe+msi） | `assembleRelease`（apk） | `docker build`（锁 digest） |
| 6. 红线校验 | **产物扫描：0 个真实 key/真实模型名** | 同左（apk 扫描） | digest 记录 + SBOM |
| 7. 打包签名 | NSIS exe（含 WebView2 引导，≤80MB 校验） | apk 签名（≤60MB 校验） | 镜像推送仓库 |
| 8. 发布/部署 | GitHub Release + 对象存储备份 | GitHub Release + 对象存储备份 | SSH 至云主机 `docker compose pull && up -d` |

**CD 策略**：单实例滚动更新；healthcheck 通过后切流；失败自动回滚至上一 digest。发布窗口低峰期（00:30-08:30，与 DeepSeek 错峰半价重合）。

### 12.2 监控告警（三件套）

- **Metrics**：relay-server `/metrics` + 云主机监控 + new-api 看板
- **Logs**：relay-server/Nginx 本地 JSON 日志（含 traceId + tenantId=deviceId）+ logrotate（应用 30 天/ERROR 90 天）
- **Dashboards**：在线设备 / WSS 连接心跳 / 指令吞吐时延 / 令牌用量 / 主机水位 / 错误码分布

**告警 P0~P3**（Owner 均为运维者，含 Runbook）：
- **P0**：relay-server/new-api 进程退出 → docker 重启 → 无法恢复按 RTO ≤ 4h
- **P1**：异常出向/磁盘 ≥85%、配对失败率 ≥50%、入向洪泛/指令失败率 ≥20% → 定位+收紧+清理
- **P2**：令牌额度 ≥80%（收紧/错峰）、DeepSeek 渠道异常（查 key/切备用）
- **P3**：备份失败（手动验证补做）、带宽 ≥80%（升带宽）

### 12.3 高可用与恢复

| 故障域 | 恢复 | RPO/RTO |
|--------|------|---------|
| 进程级 | docker `restart: always` + 端侧指数退避重连 | ≤ 2 分钟 |
| 主机级 | 控制台重启 / 更换实例 + DNS 切流（TTL 600s） | RTO ≤ 4h |
| 数据级 | 从备份桶恢复最近快照；配对关系丢失时双端重新配对（≤ 60s） | RPO ≤ 24h，RTO ≤ 4h |
| 外部依赖 | DeepSeek 故障→引导离线模式兜底；对象存储/CDN→切备用源 | 端侧离线即时可用 |

> 💡 **核心可用性兜底**：云端全挂时，双端本地 AI 对话仍可用（llama.cpp 端侧推理），仅远程控制与在线模式中断。

### 12.4 备份策略

- **执行**：每日 02:00 cron → `sqlite3 .backup` 热备 → gzip → rclone 上传备份桶（专用 AKSK）
- **保留**：30 天滚动；失败重试 3 次 + AL-08 告警
- **禁止**：手工 scp 业务数据出云主机（仅 rclone 脚本化）

---

## 13. 验收标准速查

### 13.1 核心场景验收（Given/When/Then 摘录）

**US-3 离线模型一键下载**
- AC 正常：Given 网络正常，When 点`一键下载`，Then 进度可视、断点续传、SHA256 校验通过，成功率 ≥ 95%
- AC 异常·磁盘不足：Given 磁盘不足，When 点下载，Then 拦截并提示所需空间 + `更改`存储位置入口
- AC 异常·校验失败：Given SHA256 不一致，When 校验，Then 删除文件并提示重新下载

**US-8 扫码配对**
- AC 正常：Given 双端在线，When 5 分钟内扫码，Then 全流程 ≤ 60s，双方显示配对成功
- AC 异常·过期码：Given 超 5 分钟，When 提交，Then 提示"配对码已过期，请重新生成"，不建立绑定
- AC 演示：Given 课程演示，When 连续 10 次配对+连接，Then 成功 ≥ 9 次

**US-9 对话式远程控制**
- AC 正常：Given 已配对双端在线，When 发"总结剪贴板"，Then 五态流转，端到端 P95 ≤ 3s，结果可读
- AC 危险动作：Given 指令涉及文件删除，When 电脑端弹窗，Then 手机端显示"等待确认"；拒绝/超时后手机端状态变"被拦截"
- AC 异常·电脑离线：Given 电脑离线，When 发指令，Then 2 秒内收到"送达失败：电脑当前离线"

**US-10 低端机降级**
- AC 降级：Given 6GB RAM 设备，When 进 P-A5，Then 100% 展示降级结论+在线引导，下载按钮置灰，无 OOM
- AC 双重拦截：Given 不达标标记，When 任何路径尝试下载，Then 均拦截并一致解释

### 13.2 完整版验收（F13/F14）

- **US-11 多档推荐**：Given 8GB RAM Android，When 进模型页，Then 3B/4B 标"推荐"、7B+ 标"不建议"；测速页输出 tok/s/首字延迟/内存
- **US-12 P2P+截图**：Given 同 Wi-Fi，When 发指令，Then 通道标识"直连"；打洞失败自动回落中继（P95 ≤ 3s 不变）；截图开关开时回传单帧截图

---

## 14. 常见问题 FAQ

### 14.1 安装与环境

**Q1：Windows 安装后启动闪退/白屏？**
A：多为缺 WebView2 Runtime。安装包已含引导，按提示安装 WebView2 后重启即可。系统要求 Windows 10 1809+。

**Q2：Tauri 编译报错（Rust/MSVC 相关）？**
A：确认已装 Visual Studio Build Tools（MSVC）与 Rust 1.80+ stable；`rustup default stable`；Windows 端编译 llama.cpp FFI 需要 C++ 工具链。

**Q3：Android 编译 llama.cpp JNI 失败？**
A：确认 NDK r26+ 与 CMake 3.22+ 已装；检查 `local.properties` 的 `ndk.dir` 指向；首次编译较慢（约 10~20 分钟）。

### 14.2 模型与推理

**Q4：离线模型下载慢/失败？**
A：模型走对象存储+CDN 直链（非 HuggingFace 直连，规避大陆可达性问题）；支持断点续传，网络恢复后点继续即可；仍失败检查剩余存储（PC 需 ≥ 6GB、Android ≥ 5GB）。

**Q5：低端 Android 机离线模式跑不动/闪退？**
A：设计使然——设备检测（RAM/SoC）不达标时下载按钮置灰并引导在线模式（V4 指标：100% 触达降级）。完整版 F13 会推荐更小档模型（1.5B/3B）替代。

**Q6：离线推理速度很慢？**
A：基准吞吐 PC ≥ 5 tok/s（16GB 无独显）、Android ≥ 8 tok/s（骁龙 8 系旗舰）。低于此值：PC 检查内存是否充足（8B Q4 约需 5.2GB）；Android 检查是否启用 GPU/NPU 加速（CPU 为基线）；可降低上下文长度（2048）。

### 14.3 在线模式与网关

**Q7：在线对话提示"在线服务暂时不可用"（201001）？**
A：① 检查网关连通（设置页`测试连接`）；② 运维者查 new-api 容器与 DeepSeek 渠道；③ 可切离线模式继续使用（若已下载模型）。

**Q8：提示"本月额度已用完"（201002）？**
A：令牌月度额度封顶（防账单失控，V3）。联系运维者调高额度或等次月重置；可切离线模式。

**Q9：客户端会泄露真实 API key 或真实模型名吗？**
A：不会。客户端 0 个 key 入包（构建管线扫描红线）；真实 key 仅存云主机 `.env`（600 权限）；显示名 Deepseek-V4-Pro 经网关映射到真实模型，客户端零接触。

### 14.4 远程控制

**Q10：配对码提示"无效或已过期"（401002）？**
A：配对码 5 分钟 TTL + 一次性。在电脑端 P-W3 重新`生成配对码`后 5 分钟内完成配对。

**Q11：发指令提示"电脑当前离线"（402001）？**
A：确认电脑端 LocalMind 已启动且中继连通（P-W3 看在线状态）；检查电脑端网络。

**Q12：指令被拒绝"该操作不在电脑允许范围"（402002）？**
A：动作命中白名单外。在电脑端 P-W3 白名单勾选项中添加对应动作类型（如文件操作/系统设置）；危险动作还需本机二次确认。

**Q13：远程控制安全吗？会被滥用吗？**
A：四道防线：① 配对绑定校验（仅已配对设备可发指令）；② 动作白名单（白名单外直接拒绝）；③ 危险动作本机弹窗 15 秒倒计时默认拒绝（室友在场可拦截）；④ 全部执行日志双端留痕可查。传输 TLS 加密，完整版叠加 E2E。

### 14.5 云端与成本

**Q14：云端月成本会超 150 元吗？**
A：设计控制在 83~145 元/月（云主机 40~60 + 对象存储/CDN 10~20 + DeepSeek API 30~60 + 域名 3~5）。护栏：令牌额度封顶、对象存储下载次数限制、引导错峰（DeepSeek 五折时段）、月度账单人工核对。超 80% 触发 AL-06。

**Q15：云端服务挂了怎么办？**
A：进程级 docker 自动重启（≤ 2 分钟）；主机级控制台重启/更换实例（RTO ≤ 4h）；数据级从备份桶恢复（RPO ≤ 24h），配对关系丢失时双端重新配对（≤ 60s）。**云端全挂时双端本地 AI 对话仍可用**（离线模式兜底）。

**Q16：如何备份与恢复？**
A：每日 02:00 自动备份（SQLite .backup → gzip → rclone 上传备份桶，30 天滚动）。恢复：停服 → 从备份桶恢复最近快照 → 启服 → 必要时双端重新配对。

---

## 附录 A：MVP 与完整版边界

| 阶段 | 时间窗 | 范围 | 说明 |
|------|--------|------|------|
| **MVP** | W1~W10 | F1~F11 全 P0 | 三大演示场景跑通：①PC 在线+离线对话；②手机文件 AI 处理；③手机远程操控电脑 ≥5 类白名单指令 |
| **完整版** | W11~W16 | + F12~F14 | WebRTC P2P 通道、多档模型推荐、桌面截图单帧回传、用量统计增强 |

**演进纪律**：MVP 架构骨架是完整版子集（WSS 中继保留为兜底路径，指令信封格式复用叠加 E2E 加密层），不推倒重来。

## 附录 B：术语表

| 术语 | 含义 |
|------|------|
| LocalMind | Windows 桌面 AI 助手（exe） |
| LocalFile | Android 文件处理 + 远程控制器（apk） |
| RelayCloud | 云端中继 + API 网关 |
| Deepseek-V4-Pro | 在线模型品牌化显示名（客户端可见，经网关映射到 deepseek-chat） |
| deepseek-chat | DeepSeek 官方真实模型（仅云端可见，客户端零接触） |
| llama.cpp | 端侧 LLM 推理引擎（GGUF，MIT） |
| authTicket | WSS 指令通道业务鉴权票据（HMAC） |
| 五态状态机 | 指令状态：sent→delivered→running→done/failed |
| Shared Kernel | 三方共享契约（shared-contract：信封/错误码/白名单） |

---

> 📦 **文档完整性声明**：本 README 为自包含开发文档，整合了架构设计、功能规格、接口契约、数据设计、安全基线、部署运维与验收标准。Claude Code / Codex 可直接据此进入开发，无需查阅其他资料。
>
> 如需更细粒度的设计细节（完整错误码注册表、完整 US 七段式、完整 STRIDE 表、完整资源清单），可参考 `delivery/` 目录下的六份专题文档（调研报告/高层架构设计/系统设计/部署设计/安全设计/UserStory）。
