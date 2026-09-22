//! 跨线程事件 → iced `Subscription` 桥接。
//!
//! 把音频线程回传的 [`AudioEvent`] 与托盘菜单动作 [`TrayAction`] 经 `crossbeam`
//! 通道桥接为 [`AppMessage`]，供 MVU 的 `update` 消费。桥接在独立线程里阻塞
//! `recv`，再用 `futures::channel::mpsc` 以非阻塞方式回传，UI 运行时永不因音频
//! 处理而阻塞。

use crate::app::message::AppMessage;
use crate::audio;
use iced::futures::SinkExt;
use iced::stream;

/// 订阅音频事件（位置 / 频谱 / 结束 / 错误）。
///
/// 用 `iced::stream::channel` 在一个独立任务里把 `crossbeam` 接收端桥接为
/// [`AppMessage::AudioEvent`]。通道未初始化时订阅为空（不报错）。
pub fn audio_subscription() -> iced::Subscription<AppMessage> {
    iced::Subscription::run(|| {
        stream::channel(
            256,
            |mut output: iced::futures::channel::mpsc::Sender<AppMessage>| async move {
                let (done_tx, done_rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    if let Some(rx) = audio::audio_event_receiver() {
                        for ev in rx {
                            // 对于高频频谱，队列满时丢弃瞬态帧保障实时性；位置/结束等关键事件可靠传输
                            if let crate::audio::AudioEvent::Spectrum(_) = &ev {
                                if output.try_send(AppMessage::AudioEvent(ev)).is_err()
                                    && output.is_closed()
                                {
                                    break;
                                }
                            } else if iced::futures::executor::block_on(
                                output.send(AppMessage::AudioEvent(ev)),
                            )
                            .is_err()
                            {
                                break;
                            }
                        }
                    }
                    let _ = done_tx.send(());
                });
                let _ = done_rx.await;
            },
        )
    })
}

/// 订阅托盘菜单动作。仅在 `gui` 特性下存在。
#[cfg(feature = "gui")]
pub fn tray_subscription() -> iced::Subscription<AppMessage> {
    iced::Subscription::run(|| {
        stream::channel(
            64,
            |mut output: iced::futures::channel::mpsc::Sender<AppMessage>| async move {
                let (done_tx, done_rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    if let Some(rx) = audio::tray_event_receiver() {
                        for action in rx {
                            if iced::futures::executor::block_on(
                                output.send(AppMessage::TrayAction(action)),
                            )
                            .is_err()
                            {
                                break;
                            }
                        }
                    }
                    let _ = done_tx.send(());
                });
                let _ = done_rx.await;
            },
        )
    })
}

/// 订阅主窗口尺寸变化（T11 / REQ-409）。
///
/// `window::resize_events()` 返回所有窗口的 `(Id, Size)`；
/// 主窗口过滤与持久化交给 MVU 的 `WindowResized` 处理器。
pub fn resize_subscription() -> iced::Subscription<AppMessage> {
    iced::window::resize_events()
        .map(|(id, size)| AppMessage::WindowResized(id, size.width, size.height))
}

/// 1s 持久化 tick（T11）：有窗口尺寸脏标记时落盘 settings.json。
pub fn persist_subscription() -> iced::Subscription<AppMessage> {
    iced::time::every(std::time::Duration::from_secs(1)).map(|_| AppMessage::PersistTick)
}

/// 组合所有订阅（音频 + 托盘 + 窗口关闭 / 尺寸事件 + 落盘 tick）。
impl crate::app::state::AppState {
    /// 返回应用全部 `Subscription`（音频事件、托盘动作、窗口关闭请求、
    /// 窗口尺寸变化、持久化 tick）。
    pub fn subscription(&self) -> iced::Subscription<AppMessage> {
        #[cfg(feature = "gui")]
        {
            // 所有窗口均设 `exit_on_close_request: false`，关闭按钮发出
            // `CloseRequested`（而非直接销毁），由 MVU 决定隐藏还是退出。
            iced::Subscription::batch(vec![
                audio_subscription(),
                tray_subscription(),
                iced::window::close_requests().map(AppMessage::WindowClose),
                resize_subscription(),
                persist_subscription(),
            ])
        }
        #[cfg(not(feature = "gui"))]
        {
            let _ = audio_subscription();
            iced::Subscription::none()
        }
    }
}
