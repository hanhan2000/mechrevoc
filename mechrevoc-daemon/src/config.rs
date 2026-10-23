//! mechrevoc-daemon 运行配置:集中管理运行期常量,避免魔数散落。

use std::time::Duration;

/// 主循环轮询间隔:客户端命令处理与组合键检测的节拍
pub const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// UNIX socket 文件权限(root 创建后放开写权限,允许普通用户客户端连接)
pub const SOCKET_PERMISSIONS: u32 = 0o666;

/// 触摸板开关状态持久化文件(重启后恢复上次状态)
pub const STATE_FILE: &str = "/var/lib/mechrevoc/touchpad_state";

/// 判定「进程曾被冻结(挂起/休眠)」的时间差阈值。
///
/// 远大于轮询间隔,正常调度不可能达到;即便误判也只是多做一次无害的重建与对账。
pub const RESUME_GAP: Duration = Duration::from_secs(2);

/// 状态对账间隔:发现固件复位 / 外部改动的最大延迟
pub const RECONCILE_INTERVAL: Duration = Duration::from_secs(2);
