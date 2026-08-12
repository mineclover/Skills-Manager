# Skills Manager

> **A unified desktop application for managing AI coding assistant skills.**
> Seamlessly organize, sync, and share skills for **Claude Code, Codex, Opencode** and other AI tools.

![Version](https://img.shields.io/badge/version-2.1.8-blue) ![Downloads](https://img.shields.io/github/downloads/jiweiyeah/skills-manager/total?color=brightgreen&label=downloads) ![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey) ![Tech](https://img.shields.io/badge/built%20with-Tauri%202.0%20%2B%20React%2019-orange)

[**Website**](https://skillsmanager.freeourdays.com) · [中文说明](./README_CN.md)

## 📖 Introduction

**Skills Manager** is a modern desktop application designed to solve the fragmentation of AI assistant skills configurations. Instead of managing skills and prompts separately for different tools, Skills Manager provides a central hub.

It uses a powerful **symlink synchronization mechanism**, allowing you to write a skill once and instantly use it across 30+ supported AI tools including Claude Code, Codex, Cursor, Gemini CLI, Windsurf, Trae, and more.

## ✨ Key Features

- **🎯 Unified Management**: Centralize all your AI skills in one secure location.
- **🔄 Smart Synchronization**: Automatic symlink management ensures your tools always have the latest version of your skills without file duplication.
- **🎛️ Granular Control**: Enable or disable specific skills for individual tools without deleting the original files.
- **🧭 Provider-aware Scopes**: Manage global, project, and individual CLI skill bindings with explicit targets.
- **🔎 Control-plane Inspector**: Inspect providers, bindings, projects, and operation previews from the CLI.
- **🛒 Marketplace**: Browse, install, and share community-contributed skills directly within the app.
- **🌐 AI Translation**: Translate skill names, descriptions, and content into your preferred language using LLM.
- **⌨️ Command Palette**: Quick navigation and actions via `⌘K` / `Ctrl+K`.
- **🌍 Multilingual UI**: English, Korean, and Chinese interface support.
- **⚡ High Performance**: Built with **Rust** and **Tauri 2.0** for a lightweight, blazing-fast experience.
- **🛡️ Cross-Platform**: Native support for macOS, Windows, and Linux.
- **🔌 Multi-Tool Support**: Out-of-the-box support for 30+ AI tools (Claude Code, Codex, Cursor, Gemini CLI, Windsurf, Trae, Cline, Augment, Goose, and many more), extensible via custom tools.
- **🧩 Custom Tools**: Add your own tools with custom paths and optional icons.
- **🎨 Modern UI**: Beautiful Raycast-style interface built with React 19, Tailwind CSS v4, and Radix UI.

## 📸 Screenshots

<p align="center">
    <img src="https://image.freeourdays.com/sk1.png" alt="Application screenshot 1">
    <img src="https://image.freeourdays.com/sk2.png" alt="Application screenshot 2">
    <img src="https://image.freeourdays.com/sk3.png" alt="Application screenshot 3">
</p>

## 📥 Download

Get the build for your platform from the **[official website](https://skillsmanager.freeourdays.com/#download)**, which detects your OS and architecture automatically, or pick a file yourself on the **[Releases Page](../../releases)**.

| OS | Installer Type |
|----|----------------|
| **macOS** | `.dmg` (Universal) |
| **Windows** | `.msi` / `.exe` |
| **Linux** | `.deb` / `.AppImage` / `.rpm`|

## ⚠️ Windows Important Note

If you encounter permission issues when syncing skills (symbolic link creation errors) or detection issues, please try running the application as **Administrator**. This is often required on Windows to create symbolic links unless Developer Mode is enabled.

## 🚀 Getting Started

1. **Install**: Run the installer for your platform.
2. **Setup**: On first launch, the app will guide you to select your skills storage directory.
3. **Sync**: The app automatically detects installed AI tools (like Claude Code) and links your skills.

## 🔎 Control-plane CLI

The Rust CLI exposes the same provider-aware inventory used by the UI. Use its
read commands to inspect global or project-specific state without opening the app:

```bash
# Global state (default)
npm run inspect -- inspect -- --json
npm run inspect -- providers -- --json
npm run inspect -- bindings -- --json

# Explicit project state
npm run inspect -- inspect -- --project <project-id> --json
npm run inspect -- providers -- --project <project-id> --json
npm run inspect -- bindings -- --project <project-id> --json
```

Run `npm run inspect -- -- --help` for the complete command list. Mutating commands
operate on the selected provider and scope; when they affect a shared root, they
require the explicit `--confirm-shared` flag. Start with `skill preview` before
using `skill enable` or `skill disable`.

## 🧭 Repository & Development

This repository tracks the upstream project separately from local patches. The integrated
application remains at the repository root; `upstream/main`,
`patches/skills-manager-control-plane`, and `main` are the source-control lanes.

- [Development conventions](./DEVELOPMENT.md)
- [Contributing guide](./CONTRIBUTING.md)
- [Upstream and patch guide](./PATCH_GUIDE.md)
- [Control-plane implementation plan](./IMPLEMENTATION_PLAN.md)
- [Skill management roadmap](./SKILL_MANAGEMENT_ROADMAP.md)

## ❗ Linux Troubleshooting

If you encounter a **blank white screen** when launching the `.AppImage` on Linux (especially in virtual machines like VMware/VirtualBox), it is likely a WebKitGTK hardware acceleration issue.

Please run the application from the terminal with the following command:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./Skills-Manager_<version>_amd64.AppImage
```

## 🛠️ Technology Stack

Designed for developers who care about performance and stability:

- **Core**: [Tauri 2.0](https://tauri.app/) (Rust)
- **Frontend**: [React 19](https://react.dev/) + TypeScript
- **Styling**: [Tailwind CSS v4](https://tailwindcss.com/)
- **UI Components**: [Radix UI](https://www.radix-ui.com/)
- **Editor**: [Monaco Editor](https://microsoft.github.io/monaco-editor/)

## 📅 Roadmap

We are actively working on making Skills Manager better. Here is what we are planning:

- [x] Core features (e.g., soft link synchronization, multi-tool support).
- [x] Marketplace – Browse, install, and share community-contributed skills.
- [x] AI translation for skill content.
- [ ] Plugin system to support more AI tool extensions.
- [ ] Integrated AI chat interface for testing Skills directly within the application.

## 🤝 Contributing & Feedback

We welcome all forms of contribution!

- **Found a bug?** Please submit an issue on our [Issues](../../issues) page.
- **Have a feature request?** We'd love to hear your ideas! Feel free to open an issue to discuss new features.

## 💝 Support

If this project helps you, feel free to support via QR code.

| WeChat Support QR | Alipay Support QR |
|---|---|
| <img src="https://image.freeourdays.com/2024/WechatIMG276.jpg" alt="WeChat Support QR" height="300" /> | <img src="https://image.freeourdays.com/zfb.jpg" alt="Alipay Support QR" height="300" /> |

Or support via Ko-fi: [ko-fi.com/yeheboo](https://ko-fi.com/yeheboo)

## 📈 Star History

[![Star History Chart](https://api.star-history.com/svg?repos=jiweiyeah/skills-manager&type=Date)](https://star-history.com/#jiweiyeah/skills-manager&Date)

---

*Made with ❤️ for the AI developer community.*
