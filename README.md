# 聆风 / Lingfeng

> 跨平台音乐播放器（致敬千千静听经典交互范式），基于 **Rust 2021** 与 **iced 0.14**。

![界面预览](docs/screenshot.png)

## 特性一览

- **千千静听经典竖窄界面**：高宽比 ≥ 1.6 的经典七分区单列布局（默认 340×640），保留复古操作手感与超低桌面占用。
- **全新独立音效控制台（独立窗口）**：
  - 点击主界面「音效」按钮即刻弹出独立宽屏调音台窗口，参数调节直观从容。
  - 内置多种经典均衡器预设，支持用户自由添加、编辑、重命名与删除自定义音效方案。
  - 支持将音效参数导出为 JSON 文件备份/分享，或从本地文件一键导入。
  - 主界面配备实时音效下拉选框，内置音效与自定义音效随心切换。
- **平滑渐入渐出缓冲 (Fade-in / Fade-out)**：
  - 播放起播与暂停停止时自动引入毫秒级音量包络渐变，彻底消除瞬态截断与爆音杂音，带来丝滑顺畅的回放过渡体验。
- **纯 Rust 音频流流水线**：
  - **Symphonia 0.6**：解码 MP3、FLAC、WAV、Vorbis 等主流音频格式。
  - **Rubato 5.0**：高质量音频重采样（源率 → 设备首选率），零拷贝交错缓冲区处理。
  - **自研 10 段 RBJ Biquad 均衡器**：峰值滤波 + 主增益平滑调节。
  - **RustFFT 6.4**：实数 FFT 分析与对数分块，驱动 16 段分段式 LED 频谱柱状图实时跳动。
  - **CPAL 0.18**：拉模式音频流输出，音频与频谱共用同一份处理后 PCM，零二次解码开销。
- **动态同步 LRC 歌词**：
  - 支持 UTF-8 与 GB18030/GBK 编码自动回退识别。
  - 精准时间戳匹配与高亮平滑滚动，支持 ±500ms 动态偏移微调。
  - 独立置顶的**桌面迷你歌词窗口**。
- **播放列表与四种循环模式**：
  - 列表循环、单曲循环、随机播放（洗牌周期内不重复）、顺序播放。
  - 临时插播优先队列（Queue）。
  - 本地音频文件/文件夹导入与元数据异步解析（Lofty）。
- **个性化与系统集成**：
  - 经典多套配色主题皮肤（内置「墨蓝经典」、「暖橙怀旧」），支持 JSON 自定义皮肤。
  - 系统托盘常驻（tray-icon + muda），关闭窗口最小化到托盘，支持后台快捷控制。
  - 内嵌开源中文字体（Noto Sans SC），开箱即用，无任何跨平台豆腐块 (tofu) 乱码。

## 架构与分层

代码采用 MVU 架构，纯逻辑与 GUI 层完全解耦：

- **纯逻辑层**（音频 DSP、LRC 解析、播放列表、频谱算法、配置持久化）：脱离 GUI 独立编译与毫秒级单测。
- **GUI 层**（`gui` feature）：基于 `iced 0.14` 的 `iced::daemon` 多窗口架构。

详细设计与实现细节请参阅：
- [系统架构与实现蓝图](docs/ARCHITECTURE.md)
- [类图与数据模型](docs/class-diagram.mermaid)
- [核心时序图](docs/sequence-diagram.mermaid)
- [产品需求文档 (PRD)](docs/PRD.md)
- [Agent 开发与协作指南](AGENTS.md)

## 安装包下载

从 [GitHub Releases](https://github.com/wenfer/qy-music/releases) 下载最新发行版原生安装包：

| 平台 | 安装包格式 | 说明 |
| :--- | :--- | :--- |
| **Windows** | `.exe` (安装向导) / `.zip` (免安装便携包) | 支持 Windows 10/11 (x86_64, arm64) |
| **macOS** | `.dmg` (拖拽安装) / `.tar.gz` | 支持 Universal 通用双架构、Apple Silicon (arm64) 与 Intel (x86_64) |
| **Linux** | `.deb` (Debian/Ubuntu) / `.tar.gz` | 支持 amd64 与 arm64 |

### macOS 用户重要提示（提示“文件已损坏，无法打开”解决办法）

由于本播放器为开源个人项目，未购买苹果年费商业开发者证书（$99/年）并进行在线公证，macOS Gatekeeper（看门狗）安全机制在检测到从网页下载的无证书应用时，可能会提示 **“‘Lingfeng’已损坏，无法打开。你应该将它移到废纸篓”** 或拦截运行。这属于系统的外部应用隔离保护机制，并非安装包真实损坏。

#### 解决方法（任选其一，推荐方法 1）：

1. **终端命令清除隔离标记（最快、最彻底，推荐）**：
   将 `Lingfeng.app` 拖入「访达」→「应用程序」后，打开 Mac 自带的「终端」(Terminal)，复制执行以下命令：
   ```bash
   sudo xattr -cr /Applications/Lingfeng.app
   ```
   输入开机密码回车确认即可。*(若在 Apple Silicon 上遇到签名校验，可额外执行 `sudo codesign --force --deep --sign - /Applications/Lingfeng.app`)*。之后直接双击即可秒开正常运行！

2. **系统设置放行（图形界面）**：
   - 打开 Mac「系统设置」→「隐私与安全性」；
   - 滚动到「安全性」区域，会看到系统提示：*“已阻止使用‘Lingfeng’，因为来自身份不明的开发者”*；
   - 点击旁边的「仍要打开」按钮，输入系统开机密码授权即可。

## 本地开发与编译

### 依赖环境

- Rust 1.75+ (推荐最新稳定版)
- OS: macOS / Linux / Windows

### 运行应用

```bash
# 启动完整 GUI 播放器
cargo run --release
```

### 运行测试

```bash
# 1. 秒级运行纯逻辑测试（无需加载 GUI，适合高频单测开发）
cargo test --lib --no-default-features

# 2. 运行完整测试套件（含 GUI / 布局测试）
cargo test
```

## 许可证

- 源码协议：MIT License
- 字体资源：`assets/fonts/NotoSansSC-Regular.otf` 遵循 SIL Open Font License 1.1。
