# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 与 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added
- 项目骨架标准化：README、LICENSE(MIT)、rust-toolchain、rustfmt 配置
- 统一 git 版本管理（根仓库，消除 core/daemon/tui 嵌套仓库）
- workspace 级 clippy lint 配置
- 系统信息面板：core 新增 `system` 模块（只读 sysfs 的 CPU 温度/电池），TUI System 面板展示
- 触摸板状态持久化：状态写入 `/var/lib/mechrevoc/touchpad_state`，daemon 启动时按保存值校正硬件
- daemon 新增 `lifecycle` 模块：以时间跳跃判定挂起恢复，并按固定间隔触发状态对账
- `TouchpadBackend` 新增 `refresh_state` / `set_enabled` / `is_healthy` / `reattach` 四个能力

### Changed
- 模块化：daemon 新增 `config` 模块承载运行常量
- 协议单一来源：新增 `core/protocol/line.rs`，命令字与 STATE 行格式 daemon/客户端双端共享
- 拆分 `tui/boot.rs`：部署操作（检测/安装/启动/卸载）移至新模块 `deploy.rs`
- unsafe 禁令改由 crate 属性 `#![forbid(unsafe_code)]` 声明，Cargo.toml 完整继承 workspace lints
- 清理未使用代码：`hidraw::open_readonly`、`hidraw::poll_read`、`evdev::poll_read`
- 精简冗余重导出：`hardware.rs`/`protocol.rs` 仅保留模块声明，`lib.rs` 为唯一对外门面
- 设备发现抽出 `key_devices`，作为「应打开的键盘设备」的单一判据
- `EvdevReader` 携带设备节点路径，用于句柄集合比对与失效诊断

### Fixed
- 挂起(s2idle)唤醒后触摸板自动打开：daemon 改为以用户意图（状态文件）为唯一权威，
  检测到硬件漂移即写回，启动与唤醒共用同一套对账流程
- 挂起恢复后设备重枚举导致角落双击静默失效：新增句柄健康检查，句柄集合与系统
  设备表不一致时整组重建，并重置组合键状态机以丢弃挂起前残留的半按状态
- `poll` 忽略 `POLLERR`/`POLLHUP`/`POLLNVAL`：失效句柄现会被识别并清理，
  不再出现「进程活着但收不到任何键盘事件」
- 启动校正由 `toggle` 翻转改为幂等 `set_enabled`，避免起点不同导致校正到相反状态

## [0.1.0] - 2026-08-17

首个可运行版本（由既有开发记录整理）。

### Added
- 触摸板开关 + LED 控制（HID feature report 0x06）
- 角落双击自动切换（固件注入 Super+Ctrl+Paste 监听）
- TUI 控制台：侧边栏导航 + 触摸板面板 + 实时状态
- 启动引导：daemon 检测 / 内置安装（pkexec）/ 启动
- systemd 服务部署（deploy/）
