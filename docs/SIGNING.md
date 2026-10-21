# Release signing

No signing keys, Apple credentials or certificates are supplied with this project. No unsigned development build is claimed to be trusted for public distribution.

Windows: configure a certificate thumbprint/signing command in a private Tauri build configuration or your release system. Sign the executable and both installer formats with a timestamp authority. Keep signing secrets outside the repository and verify signatures after packaging.

macOS: use a Developer ID Application identity, the appropriate team ID and Apple notarization credentials in the native build environment. Set Tauri-supported signing/notarization environment variables (`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, or supported App Store Connect credentials), then build and verify codesign, notarization and Gatekeeper on a second machine. Never commit these values.

References: [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/), [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).
