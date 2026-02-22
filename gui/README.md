# Media Lock GUI

Tauri 2.0 + React + TypeScript + shadcn/ui 桌面应用

## 开发环境

### 前置要求
- Node.js 18+
- Rust (latest stable)
- pnpm/npm/yarn

### 安装依赖

```bash
cd gui
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建发布

```bash
npm run tauri build
```

## 项目结构

```
gui/
├── src/                      # React 前端
│   ├── components/           # UI 组件
│   │   ├── ui/               # shadcn/ui 基础组件
│   │   ├── FileList.tsx      # 文件列表
│   │   ├── TaskConfigPanel.tsx # 配置面板
│   │   └── ProgressPanel.tsx  # 进度面板
│   ├── hooks/                # React hooks
│   │   └── useTaskProgress.ts # 进度监听
│   ├── lib/                  # 工具库
│   │   ├── tauri.ts          # Tauri API 封装
│   │   └── utils.ts          # 通用工具
│   ├── stores/               # 状态管理
│   │   └── appStore.ts       # Zustand store
│   ├── types/                # TypeScript 类型
│   │   └── index.ts
│   ├── App.tsx               # 主应用
│   ├── main.tsx              # 入口
│   └── index.css             # Tailwind 样式
├── src-tauri/                # Tauri Rust 后端
│   ├── src/
│   │   ├── commands.rs       # Tauri 命令
│   │   ├── task_manager.rs   # 任务管理
│   │   ├── progress.rs       # 进度桥接
│   │   ├── error.rs          # 错误类型
│   │   ├── lib.rs            # 库入口
│   │   └── main.rs           # 程序入口
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## 架构亮点

### 1. 直接调用核心库
- Tauri commands 直接调用 `media_lock_core`
- 无需 IPC/CLI 开销，性能最优

### 2. 实时进度同步
- `ProgressHandler` trait 实现
- Tauri 事件系统向前端推送进度

### 3. 类型安全
- Rust ↔ TypeScript 类型自动同步
- 前端完整的类型定义

### 4. 任务管理
- 支持批量文件处理
- 异步任务执行
- 任务取消功能
