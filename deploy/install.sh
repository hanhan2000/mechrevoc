#!/usr/bin/env bash
# mechrevoc 安装脚本(系统级服务方案)
# 用法: ./install.sh
# daemon 以 root 运行(systemd 系统服务),无需 udev 规则 / input 组

set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> 1/3 安装二进制"
sudo cp target/release/mechrevoc-daemon /usr/local/sbin/mechrevoc-daemon
sudo cp target/release/mechrevoc-tui /usr/local/bin/mechrevoc-tui

echo "==> 2/3 安装 systemd 服务"
sudo cp deploy/mechrevoc-daemon.service /etc/systemd/system/mechrevoc-daemon.service
sudo systemctl daemon-reload

echo "==> 3/3 启动并设置开机自启"
sudo systemctl enable --now mechrevoc-daemon

echo
echo "部署完成!"
echo "  - 守护进程: systemctl status mechrevoc-daemon"
echo "  - 日志:      journalctl -u mechrevoc-daemon"
echo "  - 控制台:    mechrevoc-tui"
