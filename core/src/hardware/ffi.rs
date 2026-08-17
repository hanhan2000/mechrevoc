//! 最小 libc FFI 绑定与内核 ioctl 常量(零依赖,不使用任何 crate)。
//!
//! 这里集中了所有 `unsafe extern` 声明与内核头文件里的常量,
//! 让上层模块保持纯安全 Rust。

use std::ffi::{c_char, c_ulong};
use std::os::unix::io::RawFd;

pub use std::ffi::{c_int, c_void};

// ---------------------------------------------------------------------------
// 结构体(需与内核 ABI 严格对应)
// ---------------------------------------------------------------------------

/// linux/input.h 中的 `struct input_event`(64 位 Linux 布局,共 24 字节)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub tv_sec: i64,
    pub tv_usec: i64,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    /// `struct input_event` 的字节大小
    pub const SIZE: usize = 24;
}

/// linux/poll.h 中的 `struct pollfd`
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PollFd {
    pub fd: RawFd,
    pub events: i16,
    pub revents: i16,
}

// ---------------------------------------------------------------------------
// FFI 声明
// ---------------------------------------------------------------------------

unsafe extern "C" {
    pub fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    pub fn poll(fds: *mut PollFd, nfds: u64, timeout: c_int) -> c_int;
}

// ---------------------------------------------------------------------------
// open / poll 常量
// ---------------------------------------------------------------------------

pub const O_RDONLY: c_int = 0;
pub const O_RDWR: c_int = 2;
pub const O_NONBLOCK: c_int = 0x800;

pub const POLLIN: i16 = 0x0001;

// ---------------------------------------------------------------------------
// evdev 事件类型与编码(linux/input-event-codes.h)
// ---------------------------------------------------------------------------

pub const EV_ABS: u16 = 3;
pub const EV_KEY: u16 = 1;

// ---------------------------------------------------------------------------
// ioctl 数值计算(linux/ioctl.h 的 _IOC 宏)
// ---------------------------------------------------------------------------

/// _IOC 位域布局
const IOC_DIR_SHIFT: u32 = 30;
const IOC_TYPE_SHIFT: u32 = 8;
const IOC_NR_SHIFT: u32 = 0;
const IOC_SIZE_SHIFT: u32 = 16;

const IOC_READ: u32 = 2;
const IOC_WRITE: u32 = 1;

/// 组装 _IOC 命令字
fn ioc(dir: u32, typ: u8, nr: u32, size: u32) -> c_ulong {
    ((dir as c_ulong) << IOC_DIR_SHIFT)
        | ((typ as c_ulong) << IOC_TYPE_SHIFT)
        | ((nr as c_ulong) << IOC_NR_SHIFT)
        | ((size as c_ulong) << IOC_SIZE_SHIFT)
}

/// HIDIOCSFEATURE(len):向 hidraw 设备发送 feature report
pub fn hid_iocsfeature(size: u32) -> c_ulong {
    ioc(IOC_READ | IOC_WRITE, b'H', 0x06, size)
}

/// HIDIOCGFEATURE(len):读取 hidraw 设备的 feature report
pub fn hid_iocgfeature(size: u32) -> c_ulong {
    ioc(IOC_READ | IOC_WRITE, b'H', 0x07, size)
}
