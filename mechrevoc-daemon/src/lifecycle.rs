//! 挂起恢复感知与周期对账的时机判定。
//!
//! 本模块只做时间判断,不接触硬件与全局状态,因此可用注入时间做单元测试。
//!
//! 唤醒检测原理:s2idle / suspend 期间进程被内核冻结,恢复后 [`Instant`]
//! 会跳变。主循环正常间隔为 [`POLL_INTERVAL`](crate::config::POLL_INTERVAL),
//! 若某轮发现距上次已远超该间隔,只可能是进程被冻结过——比订阅 logind 的
//! `PrepareForSleep` 更轻量,还顺带覆盖「系统长时间卡顿 / 进程被冻」等场景。

use std::time::Instant;

use crate::config;

/// 主循环每轮应执行的动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 无需额外动作
    Idle,
    /// 刚从挂起恢复:设备句柄可能失效,需重建并校正状态
    Resume,
    /// 到了周期性对账时刻
    Reconcile,
}

/// 唤醒检测与对账节拍的计时器
pub struct Lifecycle {
    last_tick: Instant,
    last_reconcile: Instant,
}

impl Lifecycle {
    /// 以上一次循环的时刻初始化计时器
    pub fn new(now: Instant) -> Self {
        Lifecycle {
            last_tick: now,
            last_reconcile: now,
        }
    }

    /// 主循环每轮调用一次,返回本轮应执行的动作。
    ///
    /// 优先级 `Resume` > `Reconcile`:唤醒后立即重建设备,本轮不再重复
    /// 触发对账(重建后本就要校正一次状态)。
    pub fn tick(&mut self, now: Instant) -> Action {
        let gap = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        if gap > config::RESUME_GAP {
            // 唤醒轮的校正由 Resume 流程负责,重置对账计时避免紧接着又来一次
            self.last_reconcile = now;
            return Action::Resume;
        }

        if now.saturating_duration_since(self.last_reconcile) >= config::RECONCILE_INTERVAL {
            self.last_reconcile = now;
            return Action::Reconcile;
        }

        Action::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const TICK: Duration = Duration::from_millis(20);

    #[test]
    fn steady_ticks_stay_idle() {
        let start = Instant::now();
        let mut lifecycle = Lifecycle::new(start);
        // 正常 20ms 节拍:未到对账间隔前全为 Idle
        assert_eq!(lifecycle.tick(start + TICK), Action::Idle);
        assert_eq!(lifecycle.tick(start + TICK * 2), Action::Idle);
        assert_eq!(lifecycle.tick(start + TICK * 50), Action::Idle);
    }

    #[test]
    fn reconcile_fires_once_per_interval() {
        let start = Instant::now();
        let mut lifecycle = Lifecycle::new(start);
        let due = start + config::RECONCILE_INTERVAL;

        assert_eq!(lifecycle.tick(due), Action::Reconcile);
        // 紧接着的下一轮不再重复触发
        assert_eq!(lifecycle.tick(due + TICK), Action::Idle);
        // 再过一整个间隔才再次触发
        assert_eq!(
            lifecycle.tick(due + config::RECONCILE_INTERVAL),
            Action::Reconcile
        );
    }

    #[test]
    fn time_jump_is_resume() {
        let start = Instant::now();
        let mut lifecycle = Lifecycle::new(start);
        // 挂起 30s:远超阈值,判定为唤醒而非普通对账
        assert_eq!(
            lifecycle.tick(start + Duration::from_secs(30)),
            Action::Resume
        );
    }

    #[test]
    fn resume_does_not_reconcile_on_next_tick() {
        let start = Instant::now();
        let mut lifecycle = Lifecycle::new(start);
        let wake = start + Duration::from_secs(30);

        assert_eq!(lifecycle.tick(wake), Action::Resume);
        // 唤醒轮的校正已由 Resume 流程完成,下一轮不应重复对账
        assert_eq!(lifecycle.tick(wake + TICK), Action::Idle);
    }

    #[test]
    fn gap_below_threshold_is_not_resume() {
        let start = Instant::now();
        let mut lifecycle = Lifecycle::new(start);
        // 略高于正常节拍但未达阈值:视为调度抖动,不重建设备
        let jitter = config::RESUME_GAP - Duration::from_millis(1);
        assert_ne!(lifecycle.tick(start + jitter), Action::Resume);
    }
}
