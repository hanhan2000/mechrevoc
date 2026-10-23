//! mechrevoc-daemon 部署操作:状态检测 / 安装 / 启动 / 卸载。
//!
//! 系统级操作均通过 polkit(`pkexec`)提权执行,与 UI 渲染解耦,
//! 引导界面(bootstrap)与主界面(System 模块)共用本层。

use std::path::Path;
use std::process::Command;

/// 检测到的守护进程状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DaemonState {
    /// 服务文件未安装(需先安装)
    NotInstalled,
    /// 已安装但未运行(可直接启动)
    InstalledStopped,
}

/// 检测守护进程状态:优先看服务文件是否存在,再确认是否已激活
pub fn detect_daemon() -> DaemonState {
    let service_installed = Path::new("/etc/systemd/system/mechrevoc-daemon.service").exists()
        || Path::new("/usr/lib/systemd/system/mechrevoc-daemon.service").exists();
    if !service_installed {
        return DaemonState::NotInstalled;
    }
    // 服务文件存在:检查是否激活(理论上激活时不会进入引导,防御性处理)
    match Command::new("systemctl")
        .args(["is-active", "mechrevoc-daemon"])
        .output()
    {
        Ok(out) if String::from_utf8_lossy(&out.stdout).contains("active") => {
            DaemonState::InstalledStopped // 防御:进入引导说明客户端连不上,视为未运行
        }
        _ => DaemonState::InstalledStopped,
    }
}

/// 通过 polkit 启动守护进程
pub fn try_start_daemon() -> bool {
    Command::new("pkexec")
        .args(["systemctl", "start", "mechrevoc-daemon"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 通过 polkit 执行安装(复制 daemon、写服务文件、启用服务)
pub fn run_install() -> bool {
    let Some(daemon_bin) = find_daemon_binary() else {
        return false;
    };
    // 由 TUI 内置的服务单元定义(与 deploy/mechrevoc-daemon.service 保持一致)
    let script = format!(
        "install -m 755 '{}' /usr/local/sbin/mechrevoc-daemon && \
         cat > /etc/systemd/system/mechrevoc-daemon.service <<'EOF'\n{}\nEOF\n\
         systemctl daemon-reload && systemctl enable --now mechrevoc-daemon",
        daemon_bin.display(),
        SERVICE_UNIT
    );
    Command::new("pkexec")
        .args(["sh", "-c", &script])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 通过 polkit 删除 daemon 服务与二进制(卸载)
pub fn run_uninstall() -> bool {
    let cmd = "systemctl disable --now mechrevoc-daemon; \
               rm -f /etc/systemd/system/mechrevoc-daemon.service \
                     /usr/local/sbin/mechrevoc-daemon \
                     /var/lib/mechrevoc/touchpad_state; \
               rmdir /var/lib/mechrevoc 2>/dev/null; \
               systemctl daemon-reload";
    Command::new("pkexec")
        .args(["sh", "-c", cmd])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 查找 mechrevoc-daemon 二进制:优先与 tui 同目录(项目 target/release),
/// 其次已安装位置(/usr/local/sbin)
fn find_daemon_binary() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidates = [
        exe_dir.join("mechrevoc-daemon"),
        std::path::PathBuf::from("/usr/local/sbin/mechrevoc-daemon"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

/// 内置的 systemd 服务单元定义
const SERVICE_UNIT: &str = r#"[Unit]
Description=Mechrevo hardware control daemon (touchpad)
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/local/sbin/mechrevoc-daemon
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
"#;
