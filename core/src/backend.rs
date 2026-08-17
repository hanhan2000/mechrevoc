//! 触摸板后端抽象:接口与具体实现分离(依赖倒置)。
//!
//! 调用方(daemon)只依赖本 trait,不依赖具体硬件实现:
//! - [`HidrawTouchpad`](crate::touchpad::HidrawTouchpad) 是真实的 hidraw 后端
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内存 mock 后端:验证 trait 语义,不接触任何硬件。
    /// daemon 侧测试可参照本实现注入假后端。
    struct MockTouchpad {
        enabled: bool,
        combo_triggered: bool,
    }

    impl MockTouchpad {
        fn new(enabled: bool) -> Self {
            MockTouchpad {
                enabled,
                combo_triggered: false,
            }
        }

        /// 模拟一次固件注入的组合键
        fn inject_combo(&mut self) {
            self.combo_triggered = true;
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
            self.enabled = !self.enabled;
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
}
