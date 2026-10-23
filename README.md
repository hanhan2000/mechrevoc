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
| 状态持久化 | 开关状态落盘，重启后按上次状态恢复（固件不保存状态） |
| 挂起恢复自愈 | 唤醒后重建设备句柄并与状态对账，固件复位也会被纠正 |
| TUI 控制台 | 实时状态显示、手动切换、启动引导（无需 sudo） |
| 系统级守护进程 | systemd 服务，开机自启，唯一接触硬件的进程 |
| 内置安装 | polkit（`pkexec`）自动部署 daemon + systemd 服务 |

## 架构

```
mechrevoc/                      Cargo workspace（3 个 crate）
├── mechrevoc-core/             硬件控制库（零依赖，FFI 调 libc）
│   └── src/
│       ├── lib.rs              模块声明 + 对外重导出（扁平 API）
│       ├── backend.rs          接口层：TouchpadBackend trait
│       ├── hardware.rs         硬件实现层：模块声明
│       ├── hardware/
│       │   ├── ffi.rs          libc syscall + ioctl 常量
│       │   ├── discovery.rs    sysfs 设备发现（hidraw / evdev）
│       │   ├── hidraw.rs       HID feature report 0x06 开关封装
│       │   ├── evdev.rs        非阻塞事件读取（poll + input_event）
│       │   └── touchpad.rs     HidrawTouchpad：组合键监听 + 切换（backend 实现）
│       ├── protocol.rs         通信协议层：模块声明
│       │   ├── line.rs         协议行定义（daemon 与客户端共享单一来源）
│       │   └── client.rs       UNIX socket 客户端协议
│       └── system.rs           系统信息：只读 sysfs（CPU 温度 / 电池）
├── mechrevoc-daemon/           守护进程（root，systemd 服务）
│   └── src/
│       ├── main.rs             主循环：命令处理 + 组合键轮询 + 唤醒/对账调度
│       ├── server.rs           UNIX socket 服务（GET/TOGGLE/SUB）
│       ├── lifecycle.rs        挂起恢复感知与对账节拍（纯时间判定，可单测）
│       ├── state_store.rs      开关状态持久化（/var/lib/mechrevoc/touchpad_state）
│       └── config.rs           运行常量与配置
├── mechrevoc-tui/              控制台（客户端，无硬件权限）
│   └── src/
│       ├── main.rs             入口：仅装配（连接 → init → event_loop → restore）
│       ├── terminal.rs         终端生命周期：init/restore + 鼠标捕获
│       ├── event_loop.rs       主事件循环：事件分发 / 推送接收 / 系统刷新 / 卸载
│       ├── state.rs            界面状态 State + 输入处理（Elm 风格）
│       ├── view.rs             渲染框架：布局 / Pane trait / 面板分发
│       ├── view/               面板目录：touchpad.rs / system.rs
│       ├── bootstrap.rs        启动引导：安装 / 启动向导流程
│       ├── service.rs          daemon 服务管理：检测 / 安装 / 启动 / 卸载（polkit）
│       └── components/         交互组件层：kind.rs / layer.rs
└── deploy/                     安装脚本 + systemd 服务单元
```

### 分层设计（core 内部自底向上）

```
调用方（daemon / tui）
      │ 依赖 trait 与扁平 API（lib.rs 重导出）
      ▼
┌──────────────────────────────┐
│ backend（接口层）             │  TouchpadBackend trait：open/enabled/toggle/…
├──────────────────────────────┤
│ hardware（硬件实现层）        │  ffi + discovery + hidraw + evdev + touchpad
├──────────────────────────────┤
│ protocol（通信协议层）        │  UNIX socket 客户端（与硬件无关）
└──────────────────────────────┘
```

- `HidrawTouchpad`（hidraw + evdev）是真实硬件后端；测试可注入内存 mock 后端（见 `mechrevoc-core/src/backend.rs` 测试）
- 未来支持新机型/新协议 = 新增一个 backend 实现，调用方零改动

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

## 挂起恢复机制

硬件在 s2idle 恢复时会被复位（触摸板固件回到出厂开启状态），而进程内存里还是
挂起前的快照。daemon 的处理方式：

1. 主循环每 20ms 一轮；若某轮发现距上次已超过 2s，判定进程曾被冻结
   （零依赖，不订阅 D-Bus 的 `PrepareForSleep`）
2. 唤醒后整组重建设备句柄（hidraw 与 evdev 节点在恢复后可能重编号）
3. 以状态文件记录的**用户意图**为准校正硬件 —— 固件复位被自动纠正
4. 运行期每 2s 对账一次：句柄集合与系统设备表不一致则重建，硬件状态漂移则写回意图

## 已知边界

- 触摸板关闭后其 input 设备消失，恢复依赖 daemon 常驻监听键盘通道
- 组合键监听打开所有键盘类设备（sysfs capabilities/key 位图过滤）
- 唤醒后的设备重建受 2s 对账节拍约束，该窗口内的角落双击可能丢失

## 许可证

[MIT](LICENSE) © hanhan2000
