//! 触摸板后端抽象:接口与具体实现分离(依赖倒置)。
//!
//! 调用方(daemon)只依赖本 trait,不依赖具体硬件实现:
//! - [`HidrawTouchpad`](crate::hardware::touchpad::HidrawTouchpad) 是真实的 hidraw 后端
//! - 测试可注入内存 mock 后端(见 `backend::tests::MockTouchpad`)
//!
//! 未来支持新机型/新协议 = 新增一个 [`TouchpadBackend`] 实现,调用方零改动。

/// 触摸板后端统一接口:查询 / 切换 / 监听。
///
/// 设计为非阻塞:调用方在自身事件循环中周期性调用 [`poll`](TouchpadBackend::poll),
/// 即可在等待自身事件的同时处理固件注入的组合键。
pub trait TouchpadBackend {
    /// 打开后端(具体实现决定如何定位设备)
    fn open() -> Result<Self, String>
    where
        Self: Sized;

    /// 触摸板是否开启
    fn enabled(&self) -> bool;

    /// 当前状态的可读描述(如 "ON, LED off, clicks on")
    fn state_description(&self) -> &'static str;

    /// 非阻塞处理一批事件;组合键触发切换时返回 true
    fn poll(&mut self) -> Result<bool, String>;

    /// 手动翻转开关状态
    fn toggle(&mut self) -> Result<(), String>;

    /// 以硬件实际状态刷新内存缓存;返回缓存值是否发生变化。
    ///
    /// 挂起恢复后固件回到出厂状态、外部工具改动开关,都会让缓存与硬件脱节。
    /// 调用方据此判断"硬件被谁改了",决定是否需要按用户意图校正。
    fn refresh_state(&mut self) -> Result<bool, String>;

    /// 幂等设置开关状态(绝对值,而非翻转)。
    ///
    /// 与 [`toggle`](TouchpadBackend::toggle) 的区别:无论当前处于什么状态,
    /// 调用后一定处于 `enabled` 指定的状态,适合"把用户意图写回硬件"。
    fn set_enabled(&mut self, enabled: bool) -> Result<(), String>;

    /// 设备句柄是否仍然可用(挂起恢复、设备重枚举后可能失效)。
    ///
    /// 默认认为健康:非硬件后端(如测试 mock)无需关心设备生命周期。
    fn is_healthy(&self) -> bool {
        true
    }

    /// 重新枚举设备并重建句柄(挂起恢复后调用)。
    ///
    /// 成功后缓存为硬件当前实际值,调用方应接着
    /// [`set_enabled`](TouchpadBackend::set_enabled) 校正为期望状态。
    fn reattach(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内存 mock 后端:验证 trait 语义,不接触任何硬件。
    /// daemon 侧测试可参照本实现注入假后端。
    ///
    /// `hardware` 与 `enabled` 刻意分离:前者模拟硬件真实状态(会被固件复位
    /// 等外部因素改动),后者是后端的内存缓存,用于验证状态漂移检测。
    struct MockTouchpad {
        enabled: bool,
        hardware: bool,
        combo_triggered: bool,
    }

    impl MockTouchpad {
        fn new(enabled: bool) -> Self {
            MockTouchpad {
                enabled,
                hardware: enabled,
                combo_triggered: false,
            }
        }

        /// 模拟一次固件注入的组合键
        fn inject_combo(&mut self) {
            self.combo_triggered = true;
        }

        /// 模拟硬件被外部改动(如挂起恢复后固件回到出厂状态)
        fn drift_hardware(&mut self, enabled: bool) {
            self.hardware = enabled;
        }
    }

    impl TouchpadBackend for MockTouchpad {
        fn open() -> Result<Self, String> {
            Ok(MockTouchpad::new(true))
        }

        fn enabled(&self) -> bool {
            self.enabled
        }

        fn state_description(&self) -> &'static str {
            if self.enabled {
                "ON, LED off, clicks on"
            } else {
                "OFF, LED on, clicks off"
            }
        }

        fn poll(&mut self) -> Result<bool, String> {
            if self.combo_triggered {
                self.combo_triggered = false;
                self.toggle()?;
                return Ok(true);
            }
            Ok(false)
        }

        fn toggle(&mut self) -> Result<(), String> {
            self.hardware = !self.hardware;
            self.enabled = self.hardware;
            Ok(())
        }

        fn refresh_state(&mut self) -> Result<bool, String> {
            let changed = self.enabled != self.hardware;
            self.enabled = self.hardware;
            Ok(changed)
        }

        fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
            self.hardware = enabled;
            self.enabled = enabled;
            Ok(())
        }
    }

    #[test]
    fn open_returns_initial_state() {
        let backend = MockTouchpad::open().expect("open");
        assert!(backend.enabled());
        assert_eq!(backend.state_description(), "ON, LED off, clicks on");
    }

    #[test]
    fn toggle_flips_state_back_and_forth() {
        let mut backend = MockTouchpad::open().expect("open");
        backend.toggle().expect("toggle");
        assert!(!backend.enabled());
        assert_eq!(backend.state_description(), "OFF, LED on, clicks off");
        backend.toggle().expect("toggle");
        assert!(backend.enabled());
    }

    #[test]
    fn poll_detects_combo_triggered_toggle() {
        let mut backend = MockTouchpad::open().expect("open");
        backend.inject_combo();
        assert!(backend.poll().expect("poll"));
        assert!(!backend.enabled());
        // 无事件时返回 false 且状态保持不变
        assert!(!backend.poll().expect("poll"));
        assert!(!backend.enabled());
    }

    #[test]
    fn usable_through_dyn_dispatch() {
        // 验证 daemon 的用法:Box<dyn TouchpadBackend> 动态分发
        let mut backend: Box<dyn TouchpadBackend> = Box::new(MockTouchpad::open().expect("open"));
        backend.toggle().expect("toggle");
        assert!(!backend.enabled());
        assert!(!backend.poll().expect("poll"));
    }

    #[test]
    fn refresh_state_detects_hardware_drift() {
        let mut backend = MockTouchpad::open().expect("open");
        // 模拟挂起恢复:硬件被固件复位,内存缓存尚未察觉
        backend.drift_hardware(false);
        assert!(backend.enabled());
        assert!(backend.refresh_state().expect("refresh"));
        assert!(!backend.enabled());
        // 无漂移时不报告变化
        assert!(!backend.refresh_state().expect("refresh"));
    }

    #[test]
    fn set_enabled_is_idempotent() {
        let mut backend = MockTouchpad::open().expect("open");
        backend.set_enabled(true).expect("set");
        assert!(backend.enabled());
        backend.set_enabled(false).expect("set");
        assert!(!backend.enabled());
        // 重复写入同一目标值结果不变
        backend.set_enabled(false).expect("set");
        assert!(!backend.enabled());
    }

    #[test]
    fn set_enabled_reasserts_after_drift() {
        let mut backend = MockTouchpad::open().expect("open");
        backend.set_enabled(false).expect("set");
        backend.drift_hardware(true);
        // 按用户意图校正后,硬件与缓存重新一致
        backend.set_enabled(false).expect("reassert");
        assert!(!backend.enabled());
        assert!(!backend.refresh_state().expect("refresh"));
    }
}
