//! 应用层：消息 / 状态 / 更新 / 订阅 的聚合入口，以及 `run()`。
//!
//! 本模块仅在 `gui` 特性下编译（依赖 iced）。纯逻辑构建（`--no-default-features`）
//! 不包含此模块。
//!
//! 采用 iced 0.13 的 [`iced::daemon`] 入口：daemon 默认不开窗、不在所有窗口关闭后
//! 退出，正好适配「系统托盘常驻 + 多窗口（主 / 迷你 / 迷你歌词）」的需求。主窗口
//! 由 [`AppState::new`] 返回的初始任务经 `window::open` 打开，尺寸取持久化的
//! `settings.window_size`（默认 340×640 竖窄窗口，见增量设计 v1.1 / T11）。

pub mod message;
pub mod state;
pub mod subscription;
pub mod update;

pub use message::{AppMessage, TrayAction};
pub use state::AppState;

use crate::config::{DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH};

/// 主窗口默认尺寸（竖窄：千千静听经典样式）。
const MAIN_WINDOW_SIZE: (f32, f32) = (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);

/// 主窗口最小尺寸（低于此布局会溢出，禁止继续缩小）。
const MIN_WINDOW_SIZE: (f32, f32) = (320.0, 520.0);

/// 应用入口：构造 iced 多窗口 daemon 并运行。
///
/// - daemon 默认不开窗，由初始任务打开主窗口；
/// - 注册内嵌 CJK 字体（Noto Sans SC）并设为全局默认字体，解决中文 tofu；
/// - 标题随当前曲目变化；
/// - 订阅音频 / 托盘 / 窗口关闭请求 / 窗口尺寸变化 / 落盘 tick；
/// - 启动系统托盘（失败仅告警，不影响主界面）。
pub fn run() -> iced::Result {
    // 启动系统托盘（最佳努力：即便环境缺失桌面托盘服务或初始化异常，也绝不影响主界面正常启动）。
    let tray_init = std::panic::catch_unwind(std::panic::AssertUnwindSafe(crate::ui::tray::build_tray));
    match tray_init {
        Ok(Ok(())) => log::info!("系统托盘初始化成功"),
        Ok(Err(e)) => log::warn!("初始化系统托盘未就绪（不影响主界面）: {e}"),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "未知异常"
            };
            log::warn!("初始化系统托盘异常捕获（不影响主界面）: {msg}");
        }
    }

    // 注册内嵌 CJK 字体并设为全局默认（T15）：
    // `.font(impl Into<Cow<'static, [u8]>>)` 加载 Noto Sans SC，
    // `.default_font(Font)` 让所有未显式指定字体的文本（含子窗口，同进程
    // Renderer 共享）都用该族渲染。托盘菜单 / 文件对话框为 OS 原生渲染，
    // 由系统字体负责。
    iced::daemon(AppState::new, AppState::update, AppState::view)
        .title(|state: &AppState, window: iced::window::Id| {
            if Some(window) == state.effects_window_id {
                return "聆风 - 音效控制台 & 均衡器 (DSP Console)".to_string();
            }
            if Some(window) == state.mini_lyrics_id {
                return "聆风 - 桌面歌词".to_string();
            }
            state
                .current_track()
                .map(|t| t.display_name())
                .unwrap_or_else(|| "LFPlayer".to_string())
        })
        .font(crate::theme::fonts::CJK_BYTES)
        .default_font(crate::theme::fonts::default_font())
        .subscription(AppState::subscription)
        .theme(|_state: &AppState, _window: iced::window::Id| iced::Theme::Dark)
        .run()
}

/// 由持久化尺寸构造主窗口设置。
///
/// `exit_on_close_request: false`：关闭按钮不直接销毁窗口，而是发出
/// `CloseRequested` 事件，交由 MVU 决定「最小化到托盘」或「退出」。
#[cfg(feature = "gui")]
pub(crate) fn main_window_settings(size: (f32, f32)) -> iced::window::Settings {
    // 非法（非正数）尺寸回退默认竖窄窗口，避免 0 尺寸窗口。
    let (w, h) = if size.0 > 0.0 && size.1 > 0.0 {
        size
    } else {
        MAIN_WINDOW_SIZE
    };
    iced::window::Settings {
        size: iced::Size::new(w, h),
        min_size: Some(iced::Size::new(MIN_WINDOW_SIZE.0, MIN_WINDOW_SIZE.1)),
        resizable: true,
        exit_on_close_request: false,
        visible: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认尺寸为竖窄 340×640，最小 320×520（REQ-409）。
    #[test]
    fn default_window_geometry_is_vertical_narrow() {
        assert_eq!(MAIN_WINDOW_SIZE, (340.0, 640.0));
        assert_eq!(MIN_WINDOW_SIZE, (320.0, 520.0));
        assert!(MIN_WINDOW_SIZE.0 <= MAIN_WINDOW_SIZE.0);
        assert!(MIN_WINDOW_SIZE.1 <= MAIN_WINDOW_SIZE.1);
    }

    /// 设置函数应透传尺寸并允许调整大小。
    #[test]
    fn main_window_settings_uses_persisted_size() {
        let s = main_window_settings((400.0, 700.0));
        assert_eq!(s.size.width, 400.0);
        assert_eq!(s.size.height, 700.0);
        assert!(s.resizable);
        assert_eq!(s.min_size, Some(iced::Size::new(320.0, 520.0)));
        assert!(!s.exit_on_close_request);
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试（增量 v1.1 / T11 / REQ-409）
    // ──────────────────────────────────────────────────────────────

    /// 非法（0 / 负数）持久化尺寸应回退默认竖窄窗口，不得产生 0 尺寸窗口。
    #[test]
    fn main_window_settings_falls_back_on_nonpositive_size() {
        for bad in [(0.0, 0.0), (-5.0, 640.0), (340.0, -1.0), (0.0, 640.0)] {
            let s = main_window_settings(bad);
            assert_eq!(
                (s.size.width, s.size.height),
                MAIN_WINDOW_SIZE,
                "非法尺寸 {bad:?} 应回退默认"
            );
        }
    }

    /// 竖窄比例约束：高/宽 ≥ 1.6（默认与最小尺寸均须满足）。
    #[test]
    fn window_geometry_ratio_is_vertical_narrow() {
        let def_ratio = MAIN_WINDOW_SIZE.1 / MAIN_WINDOW_SIZE.0;
        let min_ratio = MIN_WINDOW_SIZE.1 / MIN_WINDOW_SIZE.0;
        assert!(def_ratio >= 1.6, "默认高宽比 {def_ratio:.2} 应 ≥ 1.6");
        assert!(min_ratio >= 1.6, "最小高宽比 {min_ratio:.2} 应 ≥ 1.6");
        // min_size 不得大于默认尺寸，否则窗口打开即被强制放大。
        assert!(MIN_WINDOW_SIZE.0 <= MAIN_WINDOW_SIZE.0);
        assert!(MIN_WINDOW_SIZE.1 <= MAIN_WINDOW_SIZE.1);
    }
}
