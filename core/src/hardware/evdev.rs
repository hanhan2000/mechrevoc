//! 触摸板的 evdev 通道:打开事件节点、查询坐标轴范围、读取输入事件。

use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::ptr;

use crate::hardware::ffi::{self, InputEvent, O_NONBLOCK, O_RDONLY};

/// 单次读取最多解析的事件数
const EVENTS_PER_READ: usize = 64;
/// 读取缓冲区的字节大小
pub const READ_BUFFER_BYTES: usize = InputEvent::SIZE * EVENTS_PER_READ;

/// evdev 事件节点读取器
pub struct EvdevReader {
    fd: RawFd,
}

impl EvdevReader {
    /// 以只读、非阻塞方式打开触摸板事件节点
    pub fn open(dev: &str) -> Result<Self, String> {
        let cpath = CString::new(dev).map_err(|_| "path contains NUL".to_string())?;
        let fd = unsafe { ffi::open(cpath.as_ptr(), O_RDONLY | O_NONBLOCK) };
        if fd < 0 {
            return Err(format!("open({dev}): {}", std::io::Error::last_os_error()));
        }
        Ok(EvdevReader { fd })
    }

    /// 暴露底层文件描述符(供多路复用 poll 使用)
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// 非阻塞读取当前可用的原始事件字节(不进行 poll 等待)
    pub fn read_available<'b>(&self, buf: &'b mut [u8]) -> Result<&'b [u8], String> {
        let n = unsafe { ffi::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if n <= 0 {
            return Ok(&buf[..0]);
        }
        Ok(&buf[..n as usize])
    }
}

impl Drop for EvdevReader {
    fn drop(&mut self) {
        unsafe { ffi::close(self.fd) };
    }
}

/// 将原始事件字节零拷贝迭代为 [`InputEvent`](crate::ffi::InputEvent)。
///
/// 由于缓冲区是字节数组、对齐到 1,逐项读取时使用 `read_unaligned` 保证安全。
pub struct InputEvents<'a> {
    bytes: &'a [u8],
}

impl<'a> InputEvents<'a> {
    /// 从读取到的原始字节构造事件迭代器
    pub fn new(bytes: &'a [u8]) -> Self {
        InputEvents { bytes }
    }
}

impl Iterator for InputEvents<'_> {
    type Item = InputEvent;

    fn next(&mut self) -> Option<InputEvent> {
        if self.bytes.len() < InputEvent::SIZE {
            return None;
        }
        let (chunk, rest) = self.bytes.split_at(InputEvent::SIZE);
        self.bytes = rest;
        Some(unsafe { ptr::read_unaligned(chunk.as_ptr() as *const InputEvent) })
    }
}
