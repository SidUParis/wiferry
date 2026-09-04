# Wiferry

**主机一个轻量程序，接收端无需安装 Wiferry，不经过 Wiferry 云存储。**

Wiferry 是 MIT 许可证的局域网文件投递工具。它面向 Linux、macOS 和
Windows 主机；主机拖入文件或给出路径，其他手机、平板、电视和电脑扫描
临时二维码后即可直接下载，不需要安装 Wiferry 接收端。同一局域网使用
Nearby；两端已有 Tailscale 时可以通过 Tailnet 跨网络传输。

当前源码版本为 Rust `0.2.0-alpha.2`。已发布的 alpha.1 通过了 Linux、
Windows、macOS Apple Silicon 与 Intel 的公开 CI；alpha.2 Tailnet 已通过
第二台 Linux 设备的[完整文件、Range 和传输边界测试](docs/TAILNET_VALIDATION.md)，
每个 alpha.2 安装包仍必须分别通过原生构建与 smoke test 才能发布。当前
alpha 仍未签名或公证。

## 使用

大文件建议通过路径原地分享，不产生临时副本：

```bash
wiferry report.pdf demo.mp4
wiferry --file report.pdf --file demo.mp4
```

通过已有 Tailscale 网络分享：

```bash
wiferry --transport tailscale report.pdf
```

接收手机需要连接并获准访问同一个 tailnet，但无需安装 Wiferry，仍然扫码后
在 Safari 或 Chrome 中下载。

也可以直接启动管理界面：

```bash
wiferry
```

管理页面支持文件拖拽、文件选择器和本机路径输入。浏览器拖拽会在本机生成
临时副本，单次上限 2 GiB；路径模式直接读取原文件。Nearby 接收设备连接所选
局域网即可扫码；Tailnet 接收设备还需连接并获准访问同一个 Tailscale 网络。

## Rust 差异化

差异化不是“用了 Rust”本身，而是一组可验证的边界：

- 2.5 MB 左右的单文件 Linux 主机程序；
- 128 KiB 固定分块，内存不随文件大小线性增长；
- 根据所选地址限制为 LAN 子网或 Tailnet 来源；
- 192-bit 临时能力链接和传输中撤销；
- 公开二进制体积、启动时间、RSS、吞吐与哈希基准。

`0.2.0-alpha.1` 的 256 MiB loopback 基准中，Rust 版本比旧 Python 打包版小 88.7%，
HTTP ready 时间约快 287 倍，空闲进程树 RSS 低 92.1%；这次 Rust 的
loopback 吞吐中位数高 8.2%，但单次结果波动明显。因此项目只稳定宣称
Rust 版本更轻、启动更快，不宣称所有设备或网络上的吞吐绝对更快。

## 安全边界

二维码是临时访问能力。Nearby 模式使用 HTTP，只适合可信家庭、办公室、课堂
或个人热点；分享结束后点击 **Stop sharing**。Tailnet 页面虽然仍显示 HTTP，
数据包实际位于 Tailscale 的 WireGuard 加密隧道内；如果经过 DERP，数据仍
保持 WireGuard 加密。这不是 Wiferry 自建的云文件中继。

管理端和下载端使用两个独立端口：管理端只绑定 `127.0.0.1`，管理密钥通过
启动 URL 的 fragment 交给本机浏览器，不会进入 HTTP 请求或网页响应；局域网
访客端没有任何管理路由。服务端还会校验白名单内的本机 `Host`、浏览器写操作的 `Origin`
以及访客来源子网。

完整设计见 [README.md](README.md)、[ROADMAP.md](ROADMAP.md)、
[docs/PRODUCT_SCOPE.md](docs/PRODUCT_SCOPE.md) 和
[docs/TRANSPORTS.md](docs/TRANSPORTS.md)。
