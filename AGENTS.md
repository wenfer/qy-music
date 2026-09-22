# AGENTS.md — 聆风 (Lingfeng) 播放器开发与维护指南

> 本文档面向所有参与本项目代码编写、重构、排查与测试的 AI Agents。
> 在进行任何代码修改或功能开发前，**必须**完整阅读并严格遵守本文档所列出的核心规范与设计约定。

---

## 1. 项目定位与核心愿景

- **产品定位**：仿「千千静听」经典交互范式的跨平台桌面音乐播放器。
- **原创品牌**：**聆风 / Lingfeng (LFPlayer)**。**禁止**在界面、代码或文档中使用已受版权保护的第三方专有商标与素材。
- **核心体验**：超低资源开销、复古怀旧的竖窄单列操作手感、纯粹高保真的音频回放、实时分段 LED 频谱跳动、LRC 同步歌词。
- **技术栈**：Rust 2021 + [iced 0.14](file:///Users/admin/vscode/qy-music/Cargo.toml#L65)（MVU 架构与多窗口 daemon）+ [symphonia 0.6](file:///Users/admin/vscode/qy-music/Cargo.toml#L32) + [cpal 0.18](file:///Users/admin/vscode/qy-music/Cargo.toml#L35) + [rubato 5.0](file:///Users/admin/vscode/qy-music/Cargo.toml#L38) + [rustfft 6.4](file:///Users/admin/vscode/qy-music/Cargo.toml#L41)。

---

## 2. 必须遵守的架构红线 (Critical Constraints)

任何 Agent 在提交变更时，均不得违反以下 5 条硬性约束：

### 2.1. 主窗口绝对保持千千静听经典「竖窄单列七分区」
- **禁止改为横宽三栏布局**（历史版本曾因横宽设计被整体打回重构）。
- **几何尺寸约束**：默认尺寸为 **340×640**，最小尺寸为 **320×520**，窗口高宽比必须 **≥ 1.6**，支持自由拉伸并持久化尺寸。
- **七分区自上而下排列**：
  1. 标题栏（含歌曲滚动标题与迷你模式/关闭按钮）
  2. 当前曲目信息（歌曲名、艺术家）
  3. LED 频谱窄条（固定高度 80px）
  4. 进度条与播放时间（当前时间 / 总时长）
  5. 控制按钮行（播放/暂停、停止、上一首、下一首、音量、静音、循环模式切换、EQ 折叠切换）
  6. 主体 Tab 区域（`Fill` 高度，包含「播放列表」与「歌词」两个 Tab 页）
  7. 10 段均衡器面板（可折叠：收起 28px，展开 160px 内部滚动）

### 2.2. 中文字体与 UI 图标字符规范
- **杜绝中文字体豆腐块 (tofu)**：项目内置了开源中文字体 [`NotoSansSC-Regular.otf`](file:///Users/admin/vscode/qy-music/assets/fonts/NotoSansSC-Regular.otf)（SIL OFL 1.1 协议）。所有全局字体必须通过 [`src/theme/fonts.rs`](file:///Users/admin/vscode/qy-music/src/theme/fonts.rs) 统一加载与引用。
- **禁止在 UI 中使用 Emoji 符号**：`Noto Sans SC` 字符集不包含 emoji（如 ⏸、⏭、🔇 等，会导致乱码）。UI 控件中的图形必须使用**纯文本 Unicode 符号**（如 `◀◀`、`▶`、`▶▶`、`×`、`♫`、`🔊` 等）或内嵌图标。

### 2.3. 纯逻辑与 GUI 层绝对解耦
- 项目在 [`Cargo.toml`](file:///Users/admin/vscode/qy-music/Cargo.toml#L22-L24) 中配置了 `gui` 特性（`default = ["gui"]`）。
- 模块 [`src/audio`](file:///Users/admin/vscode/qy-music/src/audio), [`src/playlist`](file:///Users/admin/vscode/qy-music/src/playlist), [`src/lyrics`](file:///Users/admin/vscode/qy-music/src/lyrics), [`src/config`](file:///Users/admin/vscode/qy-music/src/config), [`src/visualizer`](file:///Users/admin/vscode/qy-music/src/visualizer)（除 Canvas 组件外）必须是**纯逻辑 Rust 代码**，绝对不能直接或间接引入 `iced`、`winit` 等 GUI 依赖。
- 必须确保 `cargo test --lib --no-default-features` 能在任何无图形界面的 CI/无头环境下**秒级编译与全绿通过**。

### 2.4. 单 PCM 音频流水线（零二次解码）
- 音频处理流水线为单向拉模式：
  $$\text{Symphonia 解码} \longrightarrow \text{Rubato 重采样} \longrightarrow \text{自研 RBJ Biquad EQ 链} \longrightarrow \text{RustFFT 频谱 Tap} \longrightarrow \text{CPAL 输出}$$
- 频谱 FFT 分析必须与音频输出共用同一份已经过 EQ 和音量处理的 PCM 样本。**严禁**为频谱显示开辟独立解码线程或对音频文件重复读取。

### 2.5. 线程安全与跨线程通信
- 音频回调运行在 CPAL 实时高优先级音频线程中，**绝对严禁**在音频回调中执行阻塞式 I/O、内存重分配（预分配 RingBuffer/Vector）或持有重量级互斥锁。
- 音频线程向 UI 线程传递状态统一采用无锁或轻量的 `crossbeam_channel`，并在 UI 侧通过 `iced::Subscription` 转换为 [`AppMessage`](file:///Users/admin/vscode/qy-music/src/app/message.rs) 驱动状态机更新。

---

## 3. 代码模块架构与职责地图

```
src/
├── main.rs                 # 程序二进制入口（配置日志并启动 iced::daemon）
├── lib.rs                  # 库入口与模块门控（区分纯逻辑与 GUI 模块）
├── error.rs                # 统一错误模型 LingfengError 与 Result
├── app/                    # [GUI 专属] MVU 核心状态机
│   ├── mod.rs              # run() 入口与主窗口初始设定
│   ├── state.rs            # AppState 主状态结构体
│   ├── message.rs          # AppMessage 枚举与用户操作动作
│   ├── update.rs           # MVU update() 分发器
│   └── subscription.rs     # iced 异步事件订阅（音频事件、托盘、窗口尺寸与定时器）
├── audio/                  # 音频核心引擎与 DSP
│   ├── mod.rs              # 模块导出与 PlaybackState 状态
│   ├── decoder.rs          # Symphonia 多格式解码封装 (MP3, FLAC, WAV, OGG)
│   ├── resampler.rs        # Rubato 重采样器（音源采样率 -> 声卡设备采样率）
│   ├── equalizer.rs        # 10 段均衡器预设与增益数据结构
│   ├── dsp.rs              # RBJ Biquad IIR 峰值滤波算法与主增益削波防护
│   ├── output.rs           # CPAL 输出设备初始化与拉模式 Stream 管理
│   ├── engine.rs           # PlaybackEngine 核心循环与命令消费
│   └── events.rs           # AudioEvent 事件通道
├── playlist/               # 播放列表与曲目管理
│   ├── mod.rs              # 导出定义
│   ├── track.rs            # Track 实体（路径、艺术家、曲名、时长）
│   ├── metadata.rs         # Lofty 本地音频标签提取
│   └── playlist.rs         # Playlist 核心（单曲、列表、随机循环与队列逻辑）
├── lyrics/                 # LRC 歌词模块
│   ├── mod.rs              # 导出定义
│   ├── lrc.rs              # LRC 文件解析器（支持 GB18030/UTF-8 自动识别）
│   ├── matcher.rs          # 自动匹配歌曲同级目录下的 .lrc 文件
│   └── sync.rs             # 毫秒级歌词同步、偏移量计算与当前行二分查找
├── visualizer/             # 频谱与可视化
│   ├── mod.rs              # 导出定义
│   ├── fft.rs              # 实数 R2C RustFFT 计算与 Hann 窗平滑
│   ├── spectrum.rs         # 32 对数分块能量映射与 16 段 LED 级别换算
│   └── led.rs              # [GUI 专属] iced Canvas 自定义分段式 LED 绘制
├── theme/                  # 皮肤与样式管理
│   ├── mod.rs              # 导出定义
│   ├── schema.rs           # 皮肤配色与布局参数模型
│   ├── skin.rs             # 内置皮肤（墨蓝经典 / 暖橙怀旧）与 JSON 解析
│   ├── theme.rs            # 皮肤向 iced 控件样式的适配器
│   └── fonts.rs            # Noto Sans SC 静态字体内嵌加载
├── config/                 # 配置与持久化
│   ├── mod.rs              # 导出定义
│   ├── settings.rs         # 用户设置项模型（音量、循环模式、EQ、窗口几何）
│   └── persist.rs          # 基于 dirs 的本地磁盘 JSON 配置读写
└── ui/                     # [GUI 专属] 界面视图与组件
    ├── mod.rs              # 导出定义
    ├── main_window.rs      # 主窗口竖窄七分区总布局
    ├── controls.rs         # 播放控制面板与进度滑块
    ├── playlist_view.rs    # 播放列表 Tab 视图
    ├── lyrics_view.rs      # 滚动歌词 Tab 视图
    ├── equalizer_view.rs   # 10 段 EQ 折叠面板与滑块组
    ├── mini_window.rs      # 迷你悬浮条模式窗口
    ├── tray.rs             # 系统托盘（tray-icon + muda）右键菜单绑定
    └── widgets.rs          # 辅助 UI 控件（自绘进度条、图标按钮等）
```

---

## 4. iced 0.14 踩坑记录与 API 规范

在修改 GUI 或 iced 相关逻辑时，必须注意以下在 iced 0.14 中的关键用法与升级变化：

1. **多窗口与 Daemon 入口**：
   - iced 0.14 不再需要显式启用 `multi-window` feature（已原生默认内置）。
   - [`iced::daemon`](file:///Users/admin/vscode/qy-music/src/app/mod.rs#L45) 参数签名为 `(boot, update, view)`，即直接传入 `(AppState::new, AppState::update, AppState::view)`，随后通过链式 `.title(...)` 配置动态标题，并以 `.run()` 启动。
   - **窗口显隐控制**：iced 0.14 将旧版本的 `window::change_mode` 改名为 [`window::set_mode(id, Mode::Hidden / Mode::Windowed)`](file:///Users/admin/vscode/qy-music/src/app/update.rs)。
2. **定时器后端依赖**：
   - iced 0.14 若使用异步定时器 [`iced::time::every`](file:///Users/admin/vscode/qy-music/src/app/subscription.rs#L73)，必须在 [`Cargo.toml`](file:///Users/admin/vscode/qy-music/Cargo.toml#L65) 的 iced 依赖中包含 `"smol"` feature。
3. **Subscription 桥接跨线程通道**：
   - 0.14 中 `Subscription::run_with_id` 被 `Subscription::run(fn() -> Stream)` 取代。
   - `iced::stream::channel` 产生 `mpsc::Sender<T>`，在闭包中应显式标注参数类型，如 `|mut output: iced::futures::channel::mpsc::Sender<AppMessage>|`。
4. **组件与样式命名规范**：
   - 间距控件：使用 [`iced::widget::space::horizontal()`](file:///Users/admin/vscode/qy-music/src/ui/controls.rs#L60) 或 `Space::new(...)`，不再存在平级的 `horizontal_space()`。
   - 进度条：尺寸方法统一为 [`.length(Length::Fill)`](file:///Users/admin/vscode/qy-music/src/ui/controls.rs#L100)（原 `.width(...)` 已转为私有属性）。
5. **窗口关闭拦截 (CloseRequested)**：
   - 主窗口配置了 `exit_on_close_request: false`，用户点击关闭按钮发出 `window::close_requests()`，由 MVU 决定「最小化到托盘」或「退出进程」。

---

## 5. 常用开发与测试指令

所有 Agent 在提交代码前，必须在项目根目录下完成验证：

```bash
# 1. 快速纯逻辑测试 (推荐高频运行，秒级完成，158 个单测)
cargo test --lib --no-default-features

# 2. 完整单元测试套件 (包含 GUI 测试与布局断言，173 个单测)
cargo test

# 3. 静态检查 (确保无死代码或类型错误)
cargo check --all-targets

# 4. Clippy 代码风格与最佳实践核查
cargo clippy --all-targets

# 5. 构建 Release 二进制包
cargo build --release

# 6. 本地运行完整 GUI 播放器
cargo run --release
```

---

## 6. 核心设计参考文档索引

如需深入了解详细设计决策，可参考 `docs/` 目录下的系统文档：
- [系统架构与实现蓝图](docs/ARCHITECTURE.md)：包含完整的技术选型背景、任务分解、内存生命周期和状态机定义。
- [产品需求文档 (PRD)](docs/PRD.md)：包含用户故事、交互规范、快捷键与功能验收标准。
- [系统类图 (Mermaid)](docs/class-diagram.mermaid)：完整的数据结构与 Trait 关联图。
- [核心时序图 (Mermaid)](docs/sequence-diagram.mermaid)：包含播放链路与歌词同步链路的序列图。
- [界面效果实机截图](docs/screenshot.png)：主窗口千千静听经典竖窄效果参考。

---

## 7. 常见待办与后续演进建议 (Backlog & Tech Debt)

后续接手新功能的 Agent 可优先关注以下优化项：

1. **曲目时长异步预加载**：
   - 目前 [`src/playlist/metadata.rs`](file:///Users/admin/vscode/qy-music/src/playlist/metadata.rs) 为保证添加文件时的极速响应，未对音频流进行完整包扫描，初次载入时显示 `--:--`，直到起播由解码器汇报精准时长。后续可引入后台工作队列异步解析精确时长。
2. **播放列表拖拽排序**：
   - 目前列表支持通过代码调整次序，后续可基于 iced 的拖拽事件或上下移动快捷键增强列表交互。
3. **Linux / Windows 托盘兼容性**：
   - macOS 下通过 `NSStatusItem` 表现极佳；Linux 部分 Wayland 合成器对 `Mode::Hidden` 与托盘支持不一致，后续跨平台打包需根据不同桌面环境做适配测试。
