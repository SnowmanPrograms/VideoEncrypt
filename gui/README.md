# Media Lock GUI

A modern desktop application for video encryption, built with **Tauri 2.0**, **React 19**, **TypeScript**, and **shadcn/ui**, and **Zustand**.

## Features

- **Modern UI**: Clean interface with shadcn/ui components
- **Theme Support**: Light, Dark, and System modes
- **Internationalization**: English and Chinese with runtime switching
- **Real-time Progress**: Live progress updates during encryption/decryption
- **Batch Processing**: Process multiple files or folders
- **Task Management**: Cancel running tasks

## Development

### Prerequisites
- Node.js 18+
- Rust (latest stable)
- pnpm/npm/yarn

### Setup

```bash
cd gui
npm install
```

### Development Mode

```bash
npm run tauri dev
```

### Production Build

```bash
npm run tauri build
```

## Project Structure

```
gui/
├── src/                          # React frontend
│   ├── components/              # UI components
│   │   ├── ui/                # shadcn/ui base components
│   │   ├── FileList.tsx       # File list with status badges
│   │   ├── TaskConfigPanel.tsx # Encryption configuration
│   │   ├── ProgressPanel.tsx  # Real-time progress
│   │   ├── ThemeProvider.tsx  # Theme management
│   │   ├── ThemeSwitcher.tsx  # Theme toggle button
│   │   └── LanguageSwitcher.tsx # Language toggle button
│   ├── hooks/                   # React hooks
│   │   └── useTaskProgress.ts # Tauri event listener
│   ├── lib/                    # Utilities
│   │   ├── i18n.ts           # Translation files (en/zh)
│   │   ├── i18nStore.ts      # i18n Zustand store
│   │   ├── tauri.ts           # Tauri API wrappers
│   │   └── utils.ts           # Formatting utilities
│   ├── stores/                  # State management
│   │   └── appStore.ts         # Main app store (Zustand + persist)
│   ├── types/                   # TypeScript types
│   │   └── index.ts            # Shared type definitions
│   ├── App.tsx                  # Main application component
│   ├── main.tsx                # Application entry point
│   └── index.css                # Tailwind CSS styles
├── src-tauri/                    # Tauri Rust backend
│   ├── src/
│   │   ├── commands.rs          # Tauri IPC commands
│   │   ├── task_manager.rs     # Task execution engine
│   │   ├── progress.rs         # Progress handler bridge
│   │   ├── error.rs             # Error types
│   │   ├── lib.rs               # Library entry
│   │   └── main.rs              # Binary entry
│   ├── Cargo.toml               # Rust dependencies
│   ├── tauri.conf.json         # Tauri configuration
│   ├── capabilities/            # Tauri permissions
│   └── icons/                  # Application icons
├── package.json                  # Node dependencies
├── vite.config.ts                # Vite configuration
├── tailwind.config.js            # Tailwind configuration
├── tsconfig.json                 # TypeScript configuration
└── components.json              # shadcn/ui configuration
```

## Architecture Highlights

### Direct Core Library Integration
- Tauri commands call `media_lock_core` directly
- No IPC/CLI overhead
- Optimal performance

### Real-time Progress Sync
- `ProgressHandler` trait implementation
- Tauri event system for frontend updates
- Live throughput statistics

### Type Safety
- Shared TypeScript type definitions
- Rust types mirrored in frontend
- Full IDE autocomplete

### State Management
- Zustand with persist middleware
- Settings saved to localStorage
- Theme, language, and preferences persisted

## Configuration

### Theme Options
- **Light**: Light color scheme
- **Dark**: Dark color scheme  
- **System**: Follows OS preference

### Language Options
- **English**: Default language
- **Chinese**: Full translation

### Encryption Options
- **Encrypt Audio**: Also encrypt audio tracks (slower)
- **Scrub Metadata**: Remove GPS, titles, etc.
- **Crash Safety (WAL)**: Enable write-ahead logging

## Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start Vite dev server |
| `npm run build` | Build frontend |
| `npm run tauri dev` | Start Tauri in development mode |
| `npm run tauri build` | Build production binaries |

## Dependencies

### Frontend
- React 19
- TypeScript
- Vite 6
- Tailwind CSS 4
- shadcn/ui
- Zustand 5
- Radix UI primitives
- Lucide React icons

### Backend
- Tauri 2.0
- Rust 2021 edition
- media_lock_core (workspace member)

## Building for Production

```bash
# Build MSI installer
npm run tauri build

# Output location
gui/src-tauri/target/release/bundle/
```

## License

MIT License - see LICENSE file for details
