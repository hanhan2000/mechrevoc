//! mechrevoc-daemon 守护进程的 UNIX socket 客户端。
//!
//! 供 TUI 等客户端复用:连接 socket、查询状态、发送切换命令、接收状态推送。

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use crate::protocol::line::{parse_state_line, CMD_GET, CMD_SUB, CMD_TOGGLE};

/// socket 位置:系统级守护进程(root)监听于此,普通用户客户端连接。
/// 由守护进程创建并放开权限(0666)。
pub fn default_socket_path() -> String {
    "/run/mechrevoc.sock".into()
}

/// 守护进程客户端连接
pub struct Client {
    stream: UnixStream,
    /// 未处理完的半行数据(非阻塞读取的残余)
    pending: Vec<u8>,
    /// 触摸板是否开启
    pub enabled: bool,
    /// 状态描述(如 "ON, LED off, clicks on")
    pub desc: String,
}

impl Client {
    /// 连接守护进程并获取初始状态
    pub fn connect(path: &str) -> Result<Self, String> {
        let stream = UnixStream::connect(path).map_err(|e| format!("connect {path}: {e}"))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| format!("set nonblocking: {e}"))?;

        let mut client = Client {
            stream,
            pending: Vec::new(),
            enabled: true,
            desc: String::new(),
        };
        client.get_state()?;
        Ok(client)
    }

    /// 查询当前状态
    pub fn get_state(&mut self) -> Result<(), String> {
        self.send(CMD_GET)?;
        let line = self.read_line_blocking(Duration::from_secs(1))?;
        self.apply_state(&line);
        Ok(())
    }

    /// 订阅状态推送;订阅后守护进程会在状态变化时推送 STATE 行
    pub fn subscribe(&mut self) -> Result<(), String> {
        self.send(CMD_SUB)?;
        let line = self.read_line_blocking(Duration::from_secs(1))?;
        self.apply_state(&line);
        Ok(())
    }

    /// 请求切换触摸板(实际由守护进程执行,结果通过推送反馈)
    pub fn toggle(&mut self) -> Result<(), String> {
        self.send(CMD_TOGGLE)
    }

    /// 非阻塞读取状态推送;收到新状态返回 true 并更新字段
    pub fn poll(&mut self) -> Result<bool, String> {
        if let Some(line) = self.read_line()? {
            self.apply_state(&line);
            return Ok(true);
        }
        Ok(false)
    }

    fn send(&mut self, cmd: &str) -> Result<(), String> {
        writeln!(self.stream, "{cmd}").map_err(|e| format!("send {cmd}: {e}"))
    }

    /// 解析 STATE 行更新本地状态
    fn apply_state(&mut self, line: &str) {
        if let Some((enabled, desc)) = parse_state_line(line) {
            self.enabled = enabled;
            self.desc = desc.to_string();
        }
    }

    /// 尝试读一行(非阻塞);暂无完整行时返回 None
    fn read_line(&mut self) -> Result<Option<String>, String> {
        loop {
            if let Some(pos) = self.pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.pending.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                return Ok(Some(line));
            }
            let mut buf = [0u8; 256];
            match self.stream.read(&mut buf) {
                Ok(0) => return Ok(None),
                Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) => return Err(format!("read: {e}")),
            }
        }
    }

    /// 阻塞等待一行(带超时),用于 GET/SUB 的同步响应
    fn read_line_blocking(&mut self, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(line) = self.read_line()? {
                return Ok(line);
            }
            if Instant::now() >= deadline {
                return Err("timeout waiting for daemon response".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通过 UnixStream::pair 构造连接到"模拟 daemon"的 Client
    fn client_with_mock() -> (Client, UnixStream) {
        let (server, client) = UnixStream::pair().expect("socket pair");
        client.set_nonblocking(true).expect("set nonblocking");
        (
            Client {
                stream: client,
                pending: Vec::new(),
                enabled: true,
                desc: String::new(),
            },
            server,
        )
    }

    #[test]
    fn apply_state_parses_on_line() {
        let (mut client, _server) = client_with_mock();
        client.apply_state("STATE 1 ON, LED off, clicks on");
        assert!(client.enabled);
        assert_eq!(client.desc, "ON, LED off, clicks on");
    }

    #[test]
    fn apply_state_parses_off_line() {
        let (mut client, _server) = client_with_mock();
        client.apply_state("STATE 0 OFF, LED on, clicks off");
        assert!(!client.enabled);
        assert_eq!(client.desc, "OFF, LED on, clicks off");
    }

    #[test]
    fn apply_state_ignores_non_state_line() {
        let (mut client, _server) = client_with_mock();
        client.apply_state("ERR unknown command: FOO");
        // 非 STATE 行不修改本地状态
        assert!(client.enabled);
        assert!(client.desc.is_empty());
    }

    #[test]
    fn read_line_assembles_partial_chunks() {
        let (mut client, mut server) = client_with_mock();
        server.write_all(b"STATE 1 ON,").expect("write");
        // 尚未收到换行,不应返回完整行
        assert!(client.read_line().expect("read").is_none());
        server.write_all(b" LED off, clicks on\n").expect("write");
        assert_eq!(
            client.read_line().expect("read").as_deref(),
            Some("STATE 1 ON, LED off, clicks on")
        );
    }

    #[test]
    fn read_line_splits_multiple_lines() {
        let (mut client, mut server) = client_with_mock();
        server
            .write_all(b"STATE 1 ON\nSTATE 0 OFF\n")
            .expect("write");
        assert_eq!(
            client.read_line().expect("read").as_deref(),
            Some("STATE 1 ON")
        );
        assert_eq!(
            client.read_line().expect("read").as_deref(),
            Some("STATE 0 OFF")
        );
    }
}
