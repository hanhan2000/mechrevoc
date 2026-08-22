//! mechrevoc TUI 控制台(客户端)。
//!
//! 通过 UNIX socket 连接 mechrevoc-daemon 守护进程,查询 / 切换触摸板状态,
//! 本身不接触任何硬件。启动时若守护进程未运行,进入引导流程。
//!
//! 本文件仅做装配;运行期逻辑见各模块:
//! - `state` 界面状态与输入, `view` 渲染, `event_loop`/`terminal` 运行时,
//! - `bootstrap` 启动引导, `service` daemon 服务管理, `components` 交互组件层。

// 客户端不允许任何 unsafe 代码(硬件 FFI 全部收敛于 core)
#![forbid(unsafe_code)]

mod bootstrap;
mod components;
mod event_loop;
mod service;
mod state;
mod terminal;
mod view;

use bootstrap::BootResult;
use state::State;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接守护进程;失败则进入引导流程(安装/启动)
    let socket_path = mechrevoc_core::default_socket_path();
    let mut client = match mechrevoc_core::Client::connect(&socket_path) {
        Ok(client) => client,
        Err(_) => match bootstrap::run(&socket_path)? {
            BootResult::Connected(client) => client,
            BootResult::Quit => return Ok(()),
        },
    };
    client.subscribe()?;

    // 装配:初始化终端 + 应用状态
    let mut terminal = terminal::init()?;
    let mut state = State::new();
    state.sync_from_client(&client);
    state.system_info = mechrevoc_core::read_system_info();
    state.record_temp();

    let result = event_loop::run(&mut terminal, &mut state, &mut client);

    terminal::restore();
    result
}
