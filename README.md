# mechrevoc

机械革命笔记本硬件控制中心 —— 触摸板开关 / LED 控制 + TUI 控制台 + 系统级守护进程。

针对机械革命耀光 15 GM5AQ5E（触摸板 `UNIW0001:00 093A:0255`）逆向实现：

- 触摸板固件 HID feature report `0x06` 控制开关与 LED（`0x00`=关+LED 亮 / `0x03`=开+LED 灭）
- 还原 Windows 驱动行为：**角落双击自动切换**（固件注入 `Super+Ctrl+Paste`，守护进程监听并切换）

## 功能特性

| 功能 | 说明 |
|------|------|
| 触摸板开关 | 硬件级开/关 + LED 同步（HID feature report 0x06） |
| 角落双击自动切换 | 监听固件注入的组合键，还原 Windows 驱动行为 |
| TUI 控制台 | 实时状态显示、手动切换、启动引导（无需 sudo） |
| 系统级守护进程 | systemd 服务，开机自启，唯一接触硬件的进程 |
| 内置安装 | polkit（`pkexec`）自动部署 daemon + systemd 服务 |

## 架构

```
mechrevoc/                    Cargo workspace（3 个 crate）
├── core/                     硬件控制库（零依赖，FFI 调 libc）
│   └── src/
│       ├── ffi.rs            libc syscall + ioctl 常量
│       ├── discovery.rs      sysfs 设备发现（hidraw / evdev）
│       ├── hidraw.rs         HID feature report 0x06 开关封装
│       ├── evdev.rs          非阻塞事件读取（poll + input_event）
│       ├── touchpad.rs       组合键状态机 + 手动切换（TouchpadMonitor）
│       └── client.rs         UNIX socket 客户端协议
├── daemon/                   mechrevoc-daemon（root，systemd 服务）
│   └── src/
│       ├── main.rs           主循环：组合键轮询 + 命令处理 + 状态广播
│       ├── server.rs         UNIX socket 服务（GET/TOGGLE/SUB）
│       └── config.rs         运行常量与配置
├── tui/                      mechrevoc-tui（客户端，无硬件权限）
│   └── src/
│       ├── main.rs           入口：连接 daemon + 事件循环
│       ├── app.rs            Elm 风格状态（App + handle_key → Action）
│       ├── ui.rs             侧边栏导航 + 触摸板面板
│       └── boot.rs           启动引导：检测/安装/启动 daemon
└── deploy/                   安装脚本 + systemd 服务单元
```

### socket 协议（UNIX domain socket: `/run/mechrevoc.sock`）

| 方向 | 命令 | 说明 |
|------|------|------|
| 客户端→daemon | `GET` | 查询状态，响应 `STATE <0\|1> <描述>` |
| 客户端→daemon | `TOGGLE` | 切换触摸板（结果通过推送反馈） |
| 客户端→daemon | `SUB` | 订阅状态推送 |
| daemon→客户端 | `STATE ...` / `ERR ...` | 状态行 / 错误行 |

## 构建

前置要求：Rust 1.85+（本项目锁定 `1.97.1`，见 `rust-toolchain.toml`）

```bash
cargo build --release
# 产物: target/release/mechrevoc-daemon, target/release/mechrevoc-tui
```

## 安装与使用

```bash
# 方式一：命令行安装脚本（一次性 sudo）
./deploy/install.sh

# 方式二：TUI 内置安装（未安装 daemon 时，引导界面按 1，pkexec 授权）
mechrevoc-tui

# 日常使用
mechrevoc-tui                              # 控制台（无需 sudo）
systemctl status mechrevoc-daemon          # 守护进程状态
journalctl -u mechrevoc-daemon             # 守护进程日志
```

## 卸载

```bash
sudo systemctl disable --now mechrevoc-daemon
sudo rm /usr/lib/systemd/system/mechrevoc-daemon.service
sudo systemctl daemon-reload
sudo rm /usr/local/bin/mechrevoc-tui /usr/local/bin/mechrevoc-daemon
```

## 开发

```bash
cargo fmt --all --check     # 格式检查（配置见 rustfmt.toml）
cargo clippy --all-targets  # lint（配置见 workspace Cargo.toml [workspace.lints]）
cargo test --workspace      # 单元 + 集成测试
cargo doc --open            # 文档
```

## 已知边界

- 触摸板关闭后其 input 设备消失，恢复依赖 daemon 常驻监听键盘通道
- 组合键监听打开所有键盘类设备（sysfs capabilities/key 位图过滤）

## 许可证

[MIT](LICENSE) © hanhan2000
