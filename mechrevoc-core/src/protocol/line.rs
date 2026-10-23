//! socket 协议行定义:命令字与状态行的格式化 / 解析。
//!
//! 协议为换行分隔的 UTF-8 文本行,daemon(server)与客户端(client)
//! 双方共用本模块定义,保证命令字与行格式单一来源、双端一致。

/// 客户端命令:查询当前状态
pub const CMD_GET: &str = "GET";
/// 客户端命令:请求切换触摸板(结果通过状态推送反馈)
pub const CMD_TOGGLE: &str = "TOGGLE";
/// 客户端命令:订阅状态推送(订阅后状态变化时主动推送 STATE 行)
pub const CMD_SUB: &str = "SUB";

/// 服务端响应前缀:状态行 `STATE <0|1> <描述>`
pub const STATE_PREFIX: &str = "STATE ";
/// 服务端响应前缀:错误行 `ERR <信息>`
pub const ERR_PREFIX: &str = "ERR ";

/// 将开关状态格式化为协议状态行(如 `STATE 1 ON, LED off, clicks on`)
pub fn format_state_line(enabled: bool) -> String {
    let (value, desc) = if enabled {
        ("1", crate::describe_state(crate::STATE_ON))
    } else {
        ("0", crate::describe_state(crate::STATE_OFF))
    };
    format!("{STATE_PREFIX}{value} {desc}")
}

/// 解析协议状态行;非 STATE 行返回 `None`。
///
/// 成功时返回 `(是否开启, 状态描述)`。
pub fn parse_state_line(line: &str) -> Option<(bool, &str)> {
    let rest = line.strip_prefix(STATE_PREFIX)?;
    let (value, desc) = rest.split_once(' ')?;
    Some((value == "1", desc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_state_line_matches_on_state() {
        assert_eq!(format_state_line(true), "STATE 1 ON, LED off, clicks on");
    }

    #[test]
    fn format_state_line_matches_off_state() {
        assert_eq!(format_state_line(false), "STATE 0 OFF, LED on, clicks off");
    }

    #[test]
    fn parse_state_line_roundtrips() {
        for enabled in [true, false] {
            let line = format_state_line(enabled);
            let (parsed_enabled, desc) = parse_state_line(&line).expect("parse state line");
            assert_eq!(parsed_enabled, enabled);
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn parse_state_line_ignores_other_lines() {
        assert!(parse_state_line("ERR unknown command: FOO").is_none());
        assert!(parse_state_line("").is_none());
    }

    #[test]
    fn parse_state_line_keeps_multi_word_desc() {
        let (enabled, desc) = parse_state_line("STATE 1 ON, LED off, clicks on").expect("parse");
        assert!(enabled);
        assert_eq!(desc, "ON, LED off, clicks on");
    }
}
