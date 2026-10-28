# Supercy

**轻量的菜单栏剪贴板管理器，内置截图、标注与贴图，在你自己的设备之间私密同步。**

[English](README.md)

![状态](https://img.shields.io/badge/状态-早期开发-lightgrey)
![许可证](https://img.shields.io/badge/许可证-待定-lightgrey)

> [!NOTE]
> Supercy 处于**早期开发**阶段，还没有可下载的版本。macOS 剪贴板应用（0.1 范围）已可从源码构建，正在人工检查；截图、标注与贴图是下一步。

## 它将是什么

- **轻：** 原生菜单栏应用（先做 macOS，SwiftUI/AppKit），不用网页视图。目标：内存不超过 30 MB，空闲时不占 CPU。
- **历史随手可取：** 文本、富文本、链接、图片和文件引用；精确、模糊与正则搜索（模糊搜索支持中文拼音）；置顶并带专属按键；按最近使用、首次复制或复制次数排序；预览；全局快捷键；全键盘操作；直接粘贴回刚才使用的应用，可带或不带格式；暂停记录或跳过下一次复制。
- **不止是历史：** 收藏夹与片段、粘贴前编辑、把多条合并成一条。
- **截图、标注与贴图：** 截取区域、窗口或全屏，带放大镜与取色；画矩形、箭头、文字、序号、荧光笔、马赛克与模糊；复制、保存，或贴到屏幕上成为置顶浮动的图片。历史里的任何图片都能再标注、贴图，也能在本机识别其中的文字。不截图时不占内存。
- **默认保护隐私：** 密码管理器标记为隐藏或临时的内容一律不保存；可忽略指定应用；敏感条目自动过期；历史在磁盘上加密，密钥放在系统钥匙串。不用注册、不上云、没有遥测。
- **在自己的设备之间同步：** 局域网内端到端加密，用二维码或一次性码配对。默认不需要服务器。

## 路线图

| 版本 | 内容 |
|---|---|
| 0.1 | macOS 剪贴板历史，完整的核心功能 |
| 0.2 | 截图、标注与贴图 |
| 0.3 | 收藏夹、粘贴前编辑、合并、识别文字、文件引用 |
| 0.4 | 局域网内自己的设备之间同步 |
| 1.0 | 发布：实测资源占用、隐私审计 |

## 平台

先做 macOS，再做 Windows 和 Linux；移动端最后做，且只能手动发送和接收，因为 iOS 和 Android 不允许应用在后台读取剪贴板。

## 安装

目前还没有发布任何版本；在那之前，请按[开发](#开发)一节从源码构建。之后的版本会发布在
[Releases 页面](https://github.com/weikxs/supercy/releases)，由 GitHub Actions 从本仓库构建，只提供 macOS 版：一个磁盘映像，适用于
macOS 14（Sonoma）或更新版本的 Apple 芯片和 Intel Mac。没有 Windows 或 Linux 版。

### 安装脚本，全自动

```sh
curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh
```

脚本会下载磁盘映像，用该版本的 `checksums.txt` 校验，然后把 `Supercy.app` 复制到 `/Applications`（或 `~/Applications`），不会出现“从互联网下载”的提示。除此之外不做任何改动。再运行一次即可升级。选项写在 `sh -s --` 之后，例如 `sh -s -- --version v0.1.0`：

| 选项 | 含义 |
|---|---|
| `--version <tag>` | 安装指定版本，而不是最新版 |
| `--dir <folder>` | `Supercy.app` 的安装位置 |

### Homebrew

```sh
brew install --cask weikxs/tap/supercy
brew upgrade --cask supercy            # 以后升级
```

### 磁盘映像

从[最新版本](https://github.com/weikxs/supercy/releases/latest)下载 `supercy_<version>_darwin_universal.dmg`，并用 `checksums.txt` 校验（`shasum -a 256 -c checksums.txt --ignore-missing`）。

macOS 应用没有经过 Apple 签名或公证。安装脚本和 Homebrew 会替你处理。如果你自己安装磁盘映像，请先打开一次 Supercy，再到“系统设置 → 隐私与安全性”中选择**仍要打开**，然后重新打开；也可以运行 `xattr -dr com.apple.quarantine /Applications/Supercy.app`。详见 [SECURITY.md](SECURITY.md#release-packages-and-macos)（英文）。

为了把内容直接粘贴回刚才使用的应用，Supercy 会请求“辅助功能”权限（系统设置 → 隐私与安全性 → 辅助功能）。

### 卸载

退出 Supercy，然后删除 `Supercy.app`（或运行 `brew uninstall --cask supercy`）。你的历史仍保存在 `~/Library/Application Support/Supercy`，密钥在登录钥匙串中（`io.github.weikxs.Supercy`）；要彻底清除，请把两者都删除。

## 开发

核心用 Rust 编写（`crates/`），每个平台一个很薄的原生外壳（`apps/`，从里程碑 M3 开始）。

| Crate | 作用 |
|---|---|
| `cy-model` | 历史条目、表示形式、ID、混合逻辑时钟、内容指纹 |
| `cy-filter` | 决定保存什么：隐藏/临时标记、忽略应用、大小上限、敏感内容 |
| `cy-search` | 内存中的模糊搜索，支持拼音 |
| `cy-store` | 加密的 SQLite 历史（XChaCha20-Poly1305，密钥在系统钥匙串） |
| `cy-platform` | 钥匙串与时钟的 trait，及测试用的内存实现 |
| `cy-engine` | 核心服务：采集、去重、存储、搜索、置顶、过期；不起线程和计时器 |
| `cy-ffi` | 供 Swift 调用的引擎接口（UniFFI）；生成的绑定提交在 `bindings/swift` |

所有检查都在 Linux 容器里运行，不在你的电脑上编译或安装任何东西：

```sh
make hooks     # 启用仓库自带的 git 钩子
make ci        # 每个拉取请求上 CI 运行的内容：隐私、Xcode 工程，以及下面的 test 和 cover
make check     # 全部检查：make ci、bench 和 fuzz；每次提交拉取请求前运行
make privacy   # 检查没有本地路径、密钥或真实身份信息
make test      # 在 Docker 中运行 fmt、clippy -D warnings、cargo deny、cargo audit、测试和绑定检查
make bench     # 搜索速度与内存预算（release 构建）
make cover     # 行覆盖率，至少 80%
make fuzz      # 对解析器和采集过滤器做模糊测试（nightly 容器）
```

macOS 应用（`apps/macos`，Xcode 工程由 `make macos-project` 生成）在 Mac 上构建和测试，隔离在 `/tmp/supercy`：

```sh
make macos-test      # 构建 Rust 库和应用，运行单元测试
make macos-measure   # 空闲内存与 CPU，使用内存中的剪贴板（不会读取你的剪贴板）
make macos-app       # 把 Supercy.app 构建到 /tmp/supercy-build
```

运行构建出的应用会把你真实的剪贴板记录到 `~/Library/Application Support/Supercy`（加密），密钥保存在登录钥匙串中。如果不想这样，可以用试用模式：它像正式应用一样读取真实剪贴板、注册 ⇧⌘V，但所有东西都放在 `/tmp/supercy`，密钥只在内存中，也不注册登录项：

```sh
make macos-trial         # 需要时先构建，然后启动试用（每次启动都从空历史开始）
make macos-trial-clean   # 退出试用并删除 /tmp/supercy
```

图标以 SVG 绘制在 `design/icons`，在容器中渲染（`scripts/docker-test.sh icons`），说明见 [docs/design/icons.md](docs/design/icons.md)。

### 持续集成与发布

- **拉取请求：** CI 的 `check` 任务在 Linux 上直接运行 `make ci`，所用的 `scripts/check.sh` 与你电脑上容器里运行的是同一个脚本。只改文档时只运行隐私检查。
- **`main` 受保护：** 只能通过拉取请求合入，以 squash 方式合并，且 `check` 任务必须通过。任何人都不能绕过，维护者也一样。
- **完整运行**（Actions → Full → Run workflow，发布前手动启动）：CI 的全部检查、性能预算、模糊测试、macOS 应用的构建与单元测试，以及通用版发布构建的安装与启动。
- **发布：** 推送标签 `vX.Y.Z` 后，先检查版本号和发布说明（`.github/release-notes/vX.Y.Z.md`），运行全部检查，构建通用版磁盘映像，在全新的 Apple 芯片和 Intel Mac 上安装并启动，全部通过后才连同 `checksums.txt` 一起发布。

参与开发前请先阅读 [docs/development/standards.md](docs/development/standards.md)（英文）。

## 参与贡献

欢迎贡献，请先开一个 issue 讨论。请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和[行为准则](CODE_OF_CONDUCT.md)（英文），运行 `make check`，并使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式（`make hooks` 会检查）。不要在 issue 或拉取请求中放入真实的剪贴板内容。重要变更记录在[变更日志](CHANGELOG.md)中。

## 安全

Supercy 能看到你复制的所有内容。请按 [SECURITY.md](SECURITY.md)（英文）的说明私下报告漏洞，不要发到公开 issue。

## 许可证

尚未选定。在加入许可证之前保留所有权利（见 [LICENSE](LICENSE)）。
