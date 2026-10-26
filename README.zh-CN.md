# Supercy

**轻量的菜单栏剪贴板管理器，在你自己的设备之间私密同步。**

[English](README.md)

![状态](https://img.shields.io/badge/状态-规划中-lightgrey)
![许可证](https://img.shields.io/badge/许可证-待定-lightgrey)

> [!NOTE]
> Supercy 处于**规划阶段**，还没有可下载的应用；本仓库目前只有项目骨架、开发规范和测试框架。

## 它将是什么

- **轻：** 原生菜单栏应用（先做 macOS，SwiftUI/AppKit），不用网页视图。目标：内存不超过 30 MB，空闲时不占 CPU。
- **历史随手可取：** 文本、富文本、链接和小图片；模糊搜索（支持中文拼音）；置顶；全局快捷键；键盘操作；直接粘贴回刚才使用的应用。
- **默认保护隐私：** 密码管理器标记为隐藏或临时的内容一律不保存；可忽略指定应用；敏感条目自动过期；历史在磁盘上加密，密钥放在系统钥匙串。不用注册、不上云、没有遥测。
- **在自己的设备之间同步：** 局域网内端到端加密，用二维码或一次性码配对。默认不需要服务器。

## 平台

先做 macOS，再做 Windows 和 Linux；移动端最后做，且只能手动发送和接收，因为 iOS 和 Android 不允许应用在后台读取剪贴板。

## 开发

核心用 Rust 编写（`crates/`），每个平台一个很薄的原生外壳（`apps/`，从里程碑 M3 开始）。所有检查都在 Linux 容器里运行，不在你的电脑上编译或安装任何东西：

```sh
make hooks     # 启用仓库自带的 git 钩子
make privacy   # 检查没有本地路径、密钥或真实身份信息
make test      # 在 Docker 中运行 cargo fmt --check、clippy -D warnings 和 cargo test
```

参与开发前请先阅读 [docs/development/standards.md](docs/development/standards.md)（英文）。

## 许可证

尚未选定。在加入许可证之前保留所有权利（见 [LICENSE](LICENSE)）。
