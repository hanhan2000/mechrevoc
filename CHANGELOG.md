# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 与 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added
- 项目骨架标准化：README、LICENSE(MIT)、rust-toolchain、rustfmt 配置
- 统一 git 版本管理（根仓库，消除 core/daemon/tui 嵌套仓库）
- workspace 级 clippy lint 配置

### Changed
- 模块化：daemon 新增 `config` 模块承载运行常量

## [0.1.0] - 2026-08-17

首个可运行版本（由既有开发记录整理）。

### Added
- 触摸板开关 + LED 控制（HID feature report 0x06）
- 角落双击自动切换（固件注入 Super+Ctrl+Paste 监听）
- TUI 控制台：侧边栏导航 + 触摸板面板 + 实时状态
- 启动引导：daemon 检测 / 内置安装（pkexec）/ 启动
- systemd 服务部署（deploy/）
