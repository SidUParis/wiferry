# Wiferry

**主机一个轻量程序，附近设备只需浏览器，不经过云端。**

Wiferry 是 MIT 许可证的局域网文件投递工具。它面向 Linux、macOS 和
Windows 主机；主机拖入文件或给出路径，其他手机、平板、电视和电脑扫描
临时二维码后即可直接下载，不需要安装接收端。

当前版本为 Rust `0.2.0-alpha.1`。Linux x86-64 已在本机真实构建和验证；
Windows、macOS Apple Silicon 与 Intel 的构建/测试任务已经配置，但只有在
首次公开 CI 通过后才会标记为已验证平台。

## 使用

大文件建议通过路径原地分享，不产生临时副本：

```bash
wiferry report.pdf demo.mp4
wiferry --file report.pdf --file demo.mp4
```

也可以直接启动管理界面：

```bash
wiferry
```

管理页面支持文件拖拽、文件选择器和本机路径输入。浏览器拖拽会在本机生成
临时副本，单次上限 2 GiB；路径模式直接读取原文件。接收设备连接同一局域网，
用相机扫码即可。

## Rust 差异化

差异化不是“用了 Rust”本身，而是一组可验证的边界：

- 2.5 MB 左右的单文件 Linux 主机程序；
- 128 KiB 固定分块，内存不随文件大小线性增长；
- 应用层局域网子网限制；
- 192-bit 临时能力链接和传输中撤销；
- 公开二进制体积、启动时间、RSS、吞吐与哈希基准。

最终 256 MiB loopback 基准中，Rust 版本比旧 Python 打包版小 88.7%，
HTTP ready 时间约快 287 倍，空闲进程树 RSS 低 92.1%；这次 Rust 的
loopback 吞吐中位数高 8.2%，但单次结果波动明显。因此项目只稳定宣称
Rust 版本更轻、启动更快，不宣称所有设备或网络上的吞吐绝对更快。

## 安全边界

二维码是临时访问能力，不等于加密。0.2 alpha 使用 HTTP，只适合可信家庭、
办公室、课堂或个人热点；分享结束后点击 **Stop sharing**。公共 Wi‑Fi 上的
敏感文件应等待后续带接收确认和加密的版本。

管理端和下载端使用两个独立端口：管理端只绑定 `127.0.0.1`，管理密钥通过
启动 URL 的 fragment 交给本机浏览器，不会进入 HTTP 请求或网页响应；局域网
访客端没有任何管理路由。服务端还会校验白名单内的本机 `Host`、浏览器写操作的 `Origin`
以及访客来源子网。

完整设计见 [README.md](README.md)、[ROADMAP.md](ROADMAP.md) 和
[docs/PRODUCT_SCOPE.md](docs/PRODUCT_SCOPE.md)。
