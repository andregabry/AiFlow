# Build on macOS

macOS builds require macOS with Xcode Command Line Tools, Node.js, Git and Rust stable. This Windows workspace cannot validate a native macOS app or produce a notarized DMG.

```sh
npm ci
npm run desktop:build -- --bundles app,dmg
```

Build on Apple Silicon and Intel separately or configure a universal target with both Rust targets installed. Artifacts appear under `src-tauri/target/release/bundle/`. The checked-in CI workflow provides native platform build jobs; configuring a workflow does not prove it has run.

CLI PATH differs for apps launched through Finder. Confirm the official providers are reachable in AiFlow diagnostics. A robust platform-specific PATH discovery strategy remains to be validated for user installations. Keychain permissions, native notifications, tray/menu bar, sleep assertions, cancellation and CLI login flows also require live macOS testing.

Configure Apple signing and notarization before distributing; see SIGNING.md. Do not describe an unsigned or untested app bundle as a certified macOS release.
