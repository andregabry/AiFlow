# Build on Windows

Install Node.js, Git, Rust stable MSVC, Visual Studio C++ Build Tools and WebView2. See [official prerequisites](https://v2.tauri.app/start/prerequisites/). This workspace can also use its local `.tools` Rust installation.

```powershell
npm ci
npm run desktop:build
```

The Tauri bundler generates `src-tauri/target/release/aiflow.exe`, NSIS setup EXE and WiX MSI under `src-tauri/target/release/bundle/` when platform tools are available. MSI may require Windows VBScript support. Native compilation success and installer generation are tracked separately in IMPLEMENTATION_STATUS.

For a faster local build without installers:

```powershell
npm run desktop:build -- --no-bundle
```

Provider discovery inherits PATH from the app's launch environment. Windows npm shims resolve to Node entrypoints without shell interpolation. Install CLIs separately and restart AiFlow after changing PATH.

Unsigned packages are for development. Apply your Windows code-signing identity before public distribution; see SIGNING.md.
