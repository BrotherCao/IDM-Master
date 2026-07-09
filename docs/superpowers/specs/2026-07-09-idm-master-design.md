# IDM Master 设计文档

> 创建日期：2026-07-09
> 状态：已确认，进入实现阶段

## 概述

IDM Master 是一款 Windows 平台下的高性能网络下载管理器，对标 Internet Download Manager (IDM)。提供桌面客户端 + Chrome 浏览器扩展，支持多线程分段下载，下载速度远超普通下载工具。

### 核心目标

1. **性能优先**：Rust 原生下载引擎，Direct I/O 直写磁盘，多线程分段下载
2. **浏览器集成**：Chrome Manifest V3 扩展，自动接管浏览器下载事件
3. **简洁美观**：React + Tailwind CSS，信息密度高、不花哨的现代 UI

---

## 技术选型

| 层 | 技术 | 理由 |
|---|---|---|
| 桌面壳 | Tauri v2 | 轻量（~5-10MB）、Rust 原生性能 |
| 下载引擎 | Rust + tokio + reqwest | 全异步、零拷贝、Win32 API 直调 |
| UI | React 18 + TypeScript + Tailwind CSS | 高效开发、足够美观 |
| 状态管理 | Zustand | 轻量，适合高频进度更新 |
| 持久化存储 | SQLite (via rusqlite) | 下载历史、配置、断点续传信息 |
| Chrome 扩展 | TypeScript + Manifest V3 | 现代 Chrome 扩展标准 |
| 通信协议 | RESTful HTTP (localhost) + JSON + SSE | 简单可靠，调试方便 |

---

## 总体架构

```
┌──────────────────────────────────────────────────────────┐
│                     IDM Master 整体架构                     │
├──────────────────────────────────────────────────────────┤
│                                                          │
│   ┌──────────────┐         ┌──────────────────────┐      │
│   │ Chrome 扩展   │ ─HTTP─→ │   本地 HTTP Server    │      │
│   │ (Manifest V3)│ ←─JSON─ │   (127.0.0.1:16888)  │      │
│   └──────────────┘         └────────┬─────────────┘      │
│                                     │                    │
│                          ┌──────────▼──────────┐        │
│                          │   Rust 下载引擎核心    │        │
│                          │                      │        │
│                          │  任务调度器 │ 连接池   │        │
│                          │  文件写入器 │ 速度计   │        │
│                          └──────────┬──────────┘        │
│                                     │                    │
│                          ┌──────────▼──────────┐        │
│                          │  React 前端 (UI)     │        │
│                          │  列表 · 进度 · 设置   │        │
│                          └─────────────────────┘        │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### 核心数据流

```
用户点击下载链接
    │
    ├── 场景A: 浏览器中 → Chrome扩展捕获 → 发往本地HTTP Server
    │
    └── 场景B: 客户端内 → 直接调用Rust引擎
           │
           ▼
    Rust引擎收到URL
           │
           ▼
    HEAD请求 → 获取文件大小、是否支持Range、文件名
           │
           ├── 不支持Range → 单连接下载
           │
           └── 支持Range → 动态分段策略计算N个分段
                    │
                    ▼
              连接池启动N个并发任务 → 每个下载一个range
                    │
                    ▼
              数据块写入对应文件偏移位置（Direct I/O）
                    │
                    ▼
              进度汇总 → Tauri Event → UI更新
                    │
                    ▼
              全部完成 → 校验 → 文件重命名
```

---

## Rust 下载引擎（性能核心）

### 动态分段策略

```
文件 < 1MB     → 1 线程（多线程开销 > 收益）
1MB - 10MB    → 4 线程
10MB - 100MB  → 8 线程
> 100MB       → 16 线程（可配置上限到 32）
```

**自适应机制**：
- 初始等分文件为 N 段
- 每个连接下载首块（128KB）后实测该连接的吞吐量
- 快连接获得更大段，慢连接缩小或回收其剩余部分
- 分段信息落 SQLite，支持暂停/恢复

### 文件写入：Direct I/O 绕过 OS 缓存

```
普通下载器:   网络 → OS文件缓存 → fsync → 磁盘
IDM Master:   网络 → 用户态内存池 → FILE_FLAG_NO_BUFFERING → 直写磁盘
```

- 每个下载任务预分配 64MB 内存环缓冲区
- 数据攒够 512KB（扇区对齐）后，用 Win32 `FILE_FLAG_NO_BUFFERING` 直接写盘
- 多分段各自写不同偏移，无锁互不阻塞
- 实现路径：`std::os::windows::fs::OpenOptionsExt::custom_flags(0x20000000)`

### 核心数据结构

```rust
struct DownloadTask {
    id: Uuid,
    url: String,
    file_path: PathBuf,
    total_size: u64,
    segments: Vec<Segment>,
    progress: AtomicProgress,    // 原子操作，无锁更新
    speed_meter: SpeedMeter,     // 滑动窗口测速(最近5秒)
}

struct Segment {
    start: u64,
    end: u64,
    downloaded: AtomicU64,
}

struct DownloadEngine {
    tasks: DashMap<Uuid, DownloadTask>,
    global_semaphore: Semaphore,  // 全局并发连接上限(32)
    buffer_pool: ObjectPool<AlignedBuffer>,
}
```

### 速度优化清单

| 优化项 | 手段 | 预期提升 |
|--------|------|----------|
| 无缓冲写盘 | `FILE_FLAG_NO_BUFFERING` | +15~25% |
| 连接复用 | Keep-Alive 连接池 | +10% (多文件) |
| 内存池复用 | `ObjectPool` 避免频繁分配 | +3~5% |
| 自适应分段 | 基于实测速度动态调整 | +20~30% (不稳定网络) |
| 全局连接数限制 | `Semaphore(32)` 防磁盘争抢 | 稳定性 |

---

## Chrome 扩展

### 结构

```
extension/
├── manifest.json              # Manifest V3
├── background/
│   └── service-worker.ts      # 后台：拦截下载事件
├── content/
│   └── content-script.ts      # 注入页面：捕获链接
└── popup/
    ├── popup.html             # 弹出面板：快速状态
    └── popup.ts
```

### 工作流程

```
用户在Chrome点击下载链接
         │
         ▼
chrome.downloads.onCreated 触发
         │
         ▼
Service Worker 取消Chrome原生下载 (chrome.downloads.cancel)
         │
         ▼
Service Worker 提取: url, filename, referer, cookies, user-agent
         │
         ▼
POST http://127.0.0.1:16888/api/download  → 发给桌面客户端
         │
         ▼
桌面客户端确认接收 → Service Worker 关闭
```

### 关键 API

- `chrome.downloads.onCreated` — 拦截所有下载事件
- `chrome.downloads.cancel(downloadId)` — 取消 Chrome 原生下载
- `chrome.downloads.onDeterminingFilename` — 可在文件名确定阶段拦截
- `chrome.contextMenus.create` — 右键菜单 "使用 IDM Master 下载"
- `chrome.cookies.getAll({url})` — 获取 cookies，支持需登录的下载

---

## 通信协议

本地 HTTP Server 监听 `127.0.0.1:16888`。

### API 列表

```
POST   /api/download           Chrome扩展发来新下载
POST   /api/downloads          客户端内添加下载
GET    /api/tasks              获取任务列表
GET    /api/tasks/:id          获取任务详情（含进度）
POST   /api/tasks/:id/pause    暂停
POST   /api/tasks/:id/resume   恢复（断点续传）
DELETE /api/tasks/:id          取消并删除
GET    /api/tasks/:id/events    SSE 实时进度流
GET    /api/settings           获取设置
PUT    /api/settings           更新设置
```

### 请求示例（Chrome 扩展 → 客户端）

```json
POST /api/download
{
    "url": "https://example.com/file.zip",
    "filename": "file.zip",
    "referer": "https://example.com/",
    "cookies": "session=abc123; token=xyz",
    "user_agent": "Mozilla/5.0 ...",
    "content_length": 104857600
}
```

### 启动/离线检测

- 客户端启动时启动 HTTP Server
- Chrome 扩展每次发请求前先 `GET /api/health`
- 客户端未运行时，扩展不拦截下载，回退到 Chrome 原生下载

---

## React 前端 UI

### 设计原则

性能优先，UI 简洁有力。信息密度高，不花哨。

### 主窗口布局

```
┌─────────────────────────────────────────────────────┐
│ IDM Master                              ─ □ ×       │
├─────────────────────────────────────────────────────┤
│ [+ 新建下载]  [▶ 全部开始]  [⏸ 全部暂停]  [⚙ 设置]   │
├──────────────────────────────┬──────────────────────┤
│                              │                      │
│  正在下载 (3)                 │   总计速度: 12.8 MB/s │
│  ┌────────────────────────┐  │                      │
│  │ ubuntu-24.04.iso       │  │   今日总下载: 2.4 GB  │
│  │ ████████████░░░░░ 68%  │  │   队列等待: 2         │
│  │ 8.2 MB/s · 剩余 2m15s  │  │                      │
│  │ 4/8 线程             ⏸ │  │                      │
│  └────────────────────────┘  │                      │
│  ┌────────────────────────┐  │                      │
│  │ video-tutorial.mp4     │  │                      │
│  │ ██████░░░░░░░░░░░ 32%  │  │                      │
│  │ 4.6 MB/s · 剩余 8m42s  │  │                      │
│  │ 6/8 线程             ⏸ │  │                      │
│  └────────────────────────┘  │                      │
│                              │                      │
│  已完成 (15)                  │                      │
│  ┌ file1.zip · 234MB · ✓   ┐│                      │
│  ┌ file2.exe · 1.2GB · ✓   ┐│                      │
│                              │                      │
└──────────────────────────────┴──────────────────────┘
```

### 组件树

```
App
├── TitleBar（自定义标题栏，最小化/最大化/关闭）
├── Toolbar（新建下载/全部开始/全部暂停/设置）
├── MainContent
│   ├── DownloadList
│   │   ├── DownloadItem（进度条、速度、ETA、线程数、暂停/继续）
│   │   └── ...
│   └── SidePanel（全局速度、今日统计、队列状态）
├── AddDownloadDialog（URL输入、保存路径选择）
└── SettingsPanel（并发数、默认路径、开机启动等）
```

### 技术细节

- **进度更新**：Tauri Event 推送 → Zustand → React 仅更新变化数据
- **列表虚拟化**：`@tanstack/react-virtual` 处理大量下载项
- **自定义标题栏**：Tauri `tauri-plugin-decorum`
- **系统托盘**：关闭窗口缩到托盘，托盘菜单显示实时状态

---

## 数据持久化

### 数据库：SQLite

```sql
-- 下载任务
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    total_size INTEGER NOT NULL DEFAULT 0,
    downloaded INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    -- pending | running | paused | completed | error | cancelled
    error_message TEXT,
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    referer TEXT,
    cookies TEXT
);

-- 分段信息（断点续传恢复）
CREATE TABLE segments (
    task_id TEXT NOT NULL,
    segment_index INTEGER NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    downloaded INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (task_id, segment_index),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- 设置
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

**断点续传**：暂停时落盘 segments.downloaded，恢复时从 `start_byte + downloaded` 发起 HTTP Range。

---

## 项目结构

```
idm-master/
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # Tauri 入口 + HTTP Server 启动
│   │   ├── lib.rs
│   │   ├── engine/               # 下载引擎
│   │   │   ├── mod.rs
│   │   │   ├── task.rs           # DownloadTask + Segment
│   │   │   ├── scheduler.rs      # 任务调度 + 并发控制
│   │   │   ├── connection.rs     # HTTP 连接池
│   │   │   ├── writer.rs         # Direct I/O 文件写入
│   │   │   └── speed.rs          # 滑动窗口速度计
│   │   ├── server/               # 本地 HTTP Server
│   │   │   ├── mod.rs
│   │   │   └── routes.rs
│   │   ├── db/                   # SQLite
│   │   │   ├── mod.rs
│   │   │   └── migrations.rs
│   │   └── ipc/                  # Tauri 命令处理
│   │       └── mod.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── icons/
├── src/                          # React 前端
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── Toolbar.tsx
│   │   ├── DownloadList.tsx
│   │   ├── DownloadItem.tsx
│   │   ├── AddDownloadDialog.tsx
│   │   ├── SettingsPanel.tsx
│   │   └── SpeedMeter.tsx
│   ├── stores/
│   │   └── useDownloadStore.ts   # Zustand
│   └── styles/
│       └── index.css             # Tailwind
├── extension/                    # Chrome 扩展
│   ├── manifest.json
│   ├── src/
│   │   ├── background.ts         # Service Worker
│   │   ├── content.ts            # Content Script
│   │   └── popup.ts
│   └── popup.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
└── README.md
```

---

## 开发路线图

### Phase 1：Rust 下载引擎（命令行可测试）

| 任务 | 产出 |
|------|------|
| HTTP HEAD/GET with Range | `connection.rs` |
| 内存池 + Direct I/O 写入 | `writer.rs` |
| 任务调度 + 并发控制 | `scheduler.rs` |
| 滑动窗口速度计 | `speed.rs` |
| CLI 测试入口 | `cargo run -- url` 下载文件 |

**验收标准**：单文件下载速度与 IDM 差距 ≤ 20%

### Phase 2：Tauri 壳 + React 基础 UI

| 任务 | 产出 |
|------|------|
| Tauri 项目搭建 | 框架跑通 |
| Tauri IPC 命令 | UI ↔ 引擎通信 |
| React 基础 UI | 下载列表、添加对话框、进度展示 |
| SQLite 持久化 | 断点续传、重启恢复 |
| 系统托盘 | 后台运行 |

**验收标准**：能在 UI 中添加 URL 并完成下载，重启后历史保留

### Phase 3：Chrome 扩展

| 任务 | 产出 |
|------|------|
| Manifest V3 项目结构 | 扩展骨架 |
| Service Worker 下载拦截 | 接管 Chrome 下载 |
| Content Script 链接捕获 | 页面内右键下载 |
| 右键菜单集成 | 上下文菜单 |
| Cookie 提取 | 支持需登录的下载 |

**验收标准**：Chrome 中点击下载链接 → 自动弹出 IDM Master 并开始下载

### Phase 4：高级功能

- 视频/音频嗅探（自动检测页面媒体资源）
- 站点规则（特定站点自动使用特定配置）
- 下载分类（按文件类型自动分文件夹）
- 定时下载 / 队列调度
- 多浏览器支持（Edge, Firefox）
- 安装包构建 + 自动更新

---

## 关键决策汇总

| 决策 | 选择 | 理由 |
|------|------|------|
| 桌面框架 | Tauri v2 | Rust 性能 + Web UI 平衡 |
| UI 框架 | React + Tailwind | 够用不拖累性能 |
| 扩展通信 | 本地 HTTP Server (`127.0.0.1:16888`) | 简单可靠 |
| 数据库 | SQLite | 轻量无依赖 |
| 异步运行时 | tokio | Rust 生态标准 |
| HTTP 客户端 | reqwest | HTTP/2 支持、连接池 |
| 扩展标准 | Manifest V3 | Chrome 强制要求 |
| 块对齐大小 | 512KB (扇区对齐) | Direct I/O 要求 |
| 内存池大小 | 64MB / 任务 | 平衡内存与 IO 效率 |
