# IDM Master

高性能 Windows 网络下载管理器，对标 Internet Download Manager (IDM)。

**核心优势：** Rust 原生下载引擎 + 多线程分段下载 + Direct I/O 直写磁盘，下载速度远超普通下载工具。

---

## 功能特性

### 下载引擎
- **多线程分段下载** — 动态分段策略：<1MB→1线程, 1-10MB→4线程, 10-100MB→8线程, >100MB→16线程
- **Direct I/O 直写** — `FILE_FLAG_NO_BUFFERING` 绕过操作系统缓存，减少内存拷贝
- **Keep-Alive 连接池** — reqwest 连接复用，全局并发上限 32
- **断点续传** — 分段进度实时落 SQLite，重启/暂停后可从断点恢复
- **滑动窗口测速** — 5 秒窗口精确计算实时下载速度

### 桌面客户端
- **Tauri v2 壳** — 轻量（安装包 < 10MB），Rust 原生性能
- **React + Tailwind UI** — 深色主题，简洁美观，信息密度高
- **系统托盘** — 关闭窗口缩至托盘，右键菜单快速操作
- **本地 HTTP Server** — 监听 `127.0.0.1:16888`，供浏览器扩展通信
- **SQLite 持久化** — 下载历史、分段信息、站点规则、设置全部持久化

### 浏览器扩展
- **Chrome (Manifest V3)** — 拦截浏览器下载，自动转交桌面客户端
- **Firefox (Manifest V3)** — 同样功能，含 browser polyfill
- **右键菜单** — "使用 IDM Master 下载"
- **Alt+Click** — 页面内按住 Alt 点击链接快速下载
- **视频嗅探** — 自动检测 `<video>`/`<audio>` 元素，浮动下载按钮
- **Cookie 提取** — 自动携带站点登录态，支持需登录的下载

### 高级功能
- **下载分类** — 按文件类型自动分文件夹（视频/音频/文档/压缩包/图片/程序/其他）
- **站点规则** — 按域名自定义保存路径和最大连接数，支持 `*.example.com` 通配
- **定时下载** — 支持设定未来时间点自动开始下载

---

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                       IDM Master 整体架构                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐          ┌──────────────────────┐        │
│  │ Chrome/Firefox │ ─HTTP─→ │  本地 HTTP Server     │        │
│  │   浏览器扩展    │ ←─JSON─ │  (127.0.0.1:16888)   │        │
│  └──────────────┘          └────────┬─────────────┘        │
│                                     │                       │
│                          ┌──────────▼──────────┐           │
│                          │   Rust 下载引擎核心    │           │
│                          │  · 任务调度器 · 连接池  │           │
│                          │  · 文件写入器 · 速度计  │           │
│                          │  · 分类引擎 · 规则引擎  │           │
│                          └──────────┬──────────┘           │
│                                     │                       │
│                          ┌──────────▼──────────┐           │
│                          │   React 前端 (Tauri)  │           │
│                          │  列表 · 进度 · 设置   │           │
│                          └─────────────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

### 数据流

```
用户点击下载链接
    │
    ├─ 浏览器中 → 扩展拦截 → chrome.downloads.cancel → POST /api/download → 桌面客户端
    │
    └─ 客户端内 → 直接调用 Rust 引擎
                  │
                  ▼
         HEAD 请求 → 文件大小 · 是否支持 Range · 文件名
                  │
                  ▼
         动态分段 · 站点规则匹配 · 分类目录
                  │
                  ▼
         连接池启动 N 个并发任务 → HTTP Range 分段下载
                  │
                  ▼
         Direct I/O 写入文件对应偏移 → 进度推送 UI
                  │
                  ▼
         完成 → 校验 → 通知
```

---

## 技术栈

| 层 | 技术 | 说明 |
|---|---|---|
| 桌面框架 | Tauri v2 | Rust 原生壳，~5MB |
| 下载引擎 | Rust + tokio + reqwest | 全异步，零拷贝 |
| 前端 | React 18 + TypeScript + Tailwind CSS 3 | 深色主题 |
| 状态管理 | Zustand | 轻量，适合高频更新 |
| 持久化 | SQLite (rusqlite, bundled) | 断点续传、历史、设置 |
| HTTP Server | axum 0.7 | 与 Chrome 扩展通信 |
| 扩展标准 | Manifest V3 | Chrome + Firefox |
| 安装包 | NSIS | Windows 原生安装体验 |
| CI/CD | GitHub Actions | 自动构建 + 测试 |

---

## 快速开始

### 前置要求

| 工具 | 版本 | 说明 |
|---|---|---|
| Rust | ≥ 1.80 | [rustup.rs](https://rustup.rs) 安装，MSVC 工具链 |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org) |
| VS Build Tools | 2022+ | Windows 下 Rust 编译需要 MSVC 链接器 |
| Windows SDK | 10.0.18362+ | Direct I/O 需要 |

### 安装依赖

```bash
# 克隆仓库
git clone git@github.com:BrotherCao/IDM-Master.git
cd IDM-Master

# 安装前端依赖
npm install
```

### 开发模式

```bash
# 启动 Tauri 开发环境（热更新）
npm run tauri dev
```

### 生产构建

```bash
# 构建前端 + 编译 Rust → 可执行文件
npm run tauri build

# 输出:
#   src-tauri/target/release/idm-master-tauri.exe
#   src-tauri/target/release/bundle/msi/IDM Master_0.1.0_x64.msi
```

### 构建 NSIS 安装包（可选）

```bash
# 需要安装 NSIS (https://nsis.sourceforge.io)
# 然后运行:
.\scripts\build-installer.ps1 -Version "0.1.0"
```

---

## 使用指南

### 一、启动桌面客户端

1. 安装后，双击桌面快捷方式 `IDM Master`，或从开始菜单启动
2. 程序启动后自动在 `127.0.0.1:16888` 启动本地 HTTP 服务
3. 关闭窗口时程序缩到系统托盘，右键托盘图标可彻底退出

### 二、手动添加下载

1. 点击客户端工具栏的 **「+ 新建下载」**
2. 粘贴文件 URL（如 `https://example.com/file.zip`）
3. 填写保存目录（如 `C:\Users\你的用户名\Downloads`）
4. 点击 **「开始下载」**

> 💡 如果开启了「下载分类」，文件会自动保存到对应子目录（如 `Downloads/video/movie.mp4`）

### 三、安装 Chrome 扩展

1. 打开 Chrome，地址栏输入 `chrome://extensions/`
2. 右上角开启 **「开发者模式」**
3. 点击 **「加载已解压的扩展程序」**
4. 选择项目中的 `extension/` 目录
5. 扩展图标出现在工具栏，点击可查看下载状态

**测试：** 在 Chrome 中点击任意下载链接，IDM Master 会自动接管下载。

### 四、安装 Firefox 扩展

1. 打开 Firefox，地址栏输入 `about:debugging#/runtime/this-firefox`
2. 点击 **「临时加载附加组件」**
3. 选择 `extension-firefox/manifest.json`

### 五、浏览器快捷键

| 操作 | 方式 |
|---|---|
| 拦截下载 | 点击任何下载链接（自动） |
| 快捷下载 | 按住 `Alt` 点击链接 |
| 右键下载 | 右键链接 → 「使用 IDM Master 下载」 |
| 视频下载 | 鼠标悬停视频 → 点击浮动蓝色 ⬇ 按钮 |

### 六、站点规则配置

针对特定网站自定义下载行为：

```bash
# 示例：将所有 *.example.com 的文件保存到指定目录
# 这可以通过扩展的 popup 设置界面完成，或通过以下 API:

# 添加规则
curl -X POST http://127.0.0.1:16888/api/rules \
  -H "Content-Type: application/json" \
  -d '{"domain_pattern":"*.example.com","save_path":"D:\\Downloads\\Example","max_connections":8}'
```

### 七、设置说明

| 设置项 | 默认值 | 说明 |
|---|---|---|
| 下载分类 | 开启 | 按文件类型自动分文件夹 |
| 全局并发连接 | 32 | 所有下载任务的总连接上限 |
| 速度窗口 | 5 秒 | 实时速度计算的时间窗口 |
| 数据库位置 | `%APPDATA%/IDM-Master/idm-master.db` | SQLite 数据库路径 |

---

## 项目结构

```
idm-master/
├── crates/idm-engine/               # Rust 下载引擎
│   └── src/engine/
│       ├── task.rs                  # 核心数据模型
│       ├── connection.rs            # HTTP 连接池
│       ├── scheduler.rs             # 任务调度 + 并发控制
│       ├── writer.rs                # Direct I/O 文件写入
│       ├── speed.rs                 # 滑动窗口速度计
│       ├── db.rs                    # SQLite 封装
│       ├── classify.rs              # 文件分类引擎
│       └── rules.rs                 # 站点规则引擎
│
├── src-tauri/                       # Tauri v2 桌面壳
│   └── src/
│       ├── main.rs                  # Windows 入口点
│       ├── lib.rs                   # Tauri 命令注册 + 托盘
│       └── server.rs                # axum HTTP Server
│
├── src/                             # React 前端
│   ├── components/                  # UI 组件
│   │   ├── Toolbar.tsx              # 工具栏 + 新建下载对话框
│   │   ├── DownloadList.tsx         # 下载任务列表
│   │   ├── DownloadItem.tsx         # 单个下载项（进度条/暂停/继续）
│   │   └── SidePanel.tsx            # 侧边栏（全局速度/统计）
│   ├── stores/useDownloadStore.ts   # Zustand 状态
│   └── api/index.ts                 # Tauri IPC 封装
│
├── extension/                       # Chrome 扩展 (Manifest V3)
│   ├── manifest.json
│   ├── background/service-worker.js # 下载拦截 + 转发
│   ├── content/content-script.js    # 页面嗅探 + Alt+Click
│   └── popup/                       # 弹出面板
│
├── extension-firefox/               # Firefox 扩展
├── scripts/                         # 构建脚本
│   ├── installer.nsi                # NSIS 安装脚本
│   └── build-installer.ps1          # 一键构建脚本
└── .github/workflows/build.yml      # CI/CD
```

---

## 开发

```bash
# 运行所有 Rust 测试
cargo test --workspace

# 仅检查编译
cargo check

# 前端热更新开发
npm run dev

# 启动完整的 Tauri 应用（含 Rust）
npm run tauri dev
```

---

## License

MIT License — 详见 [LICENSE](LICENSE) 文件。

---

## 路线图

- [x] Phase 1 — Rust 下载引擎（HTTP Range + Direct I/O + 速度计）
- [x] Phase 2 — Tauri 壳 + React UI + SQLite + 系统托盘
- [x] Phase 3 — Chrome 扩展 + 本地 HTTP Server
- [x] Phase 4 — 下载分类 · 视频嗅探 · 站点规则 · 定时下载 · Firefox · NSIS
- [ ] 下一阶段 — 自动更新 · 浏览器视频流捕获 (HLS/DASH) · 下载后扫描
