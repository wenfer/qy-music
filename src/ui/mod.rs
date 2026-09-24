//! 视图层：把 [`AppState`] 渲染为 iced 元素，并按窗口 id 分发到主窗口 / 迷你模式 /
//! 迷你歌词窗口。所有控件经 [`AppMessage`] 上报，绝不直接修改状态。

pub mod controls;
pub mod effects_console;
pub mod equalizer_view;
pub mod lyrics_view;
pub mod main_window;
pub mod mini_window;
pub mod playlist_view;
pub mod style;
pub mod tray;
pub mod webdav_window;
pub mod widgets;

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use iced::Element;

/// 主视图入口：依据窗口 id 渲染不同窗口内容。
impl AppState {
    /// MVU 视图：根据窗口 id 渲染主窗口 / 迷你模式 / 迷你歌词 / 独立音效控制台 / 独立 WebDAV 管理窗口。
    pub fn view(&self, window: iced::window::Id) -> Element<'_, AppMessage> {
        if Some(window) == self.mini_window_id {
            return crate::ui::mini_window::mini_mode_view(self);
        }
        if Some(window) == self.mini_lyrics_id {
            return crate::ui::lyrics_view::mini_lyrics_view(self);
        }
        if Some(window) == self.effects_window_id {
            return crate::ui::effects_console::effects_console_view(self);
        }
        if Some(window) == self.webdav_window_id {
            return crate::ui::webdav_window::webdav_window_view(self);
        }
        crate::ui::main_window::main_view(self)
    }
}
