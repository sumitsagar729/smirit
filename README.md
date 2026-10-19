# Smriti

Private, offline family photo library: find every photo and video by **who** is in it, **when** and **where**.
Everything runs on your computer; nothing is uploaded.

- Product spec: [PRD & Technical Specification](https://claude.ai/code/artifact/400d72ad-77f9-42a6-b816-190324baec79)
- Stack: Tauri 2 · Rust core · React 19 + TypeScript · SQLite · ONNX Runtime

## Layout

```
apps/desktop/            React UI (Vite, Tailwind)
apps/desktop/src-tauri/  Tauri shell: typed commands, window, installers
crates/smriti-core/      Library engine: database, sources, (soon) scanning, faces, search
```

## Prerequisites (Windows)

- Rust stable (`rustup`), MSVC C++ Build Tools with the Windows 11 SDK
- Node 22 (`.nvmrc`) and pnpm 9

## Common tasks

```bash
pnpm install
pnpm dev                                  # desktop app with hot reload
pnpm ui:dev                               # UI only in a browser, with mock data
pnpm --filter @smriti/desktop bindings    # regenerate TS types from the Rust commands
cargo test -p smriti-core                 # Rust tests
pnpm --filter @smriti/desktop test        # UI tests
pnpm build                                # NSIS + MSI installers
```

Set `SMRITI_LIBRARY_DIR` to keep a dev library somewhere other than `%LOCALAPPDATA%\Smriti`.
