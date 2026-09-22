//! 系统托盘：用 `tray-icon` + `muda` 构建托盘图标与右键菜单，并把菜单动作经
//! 全局通道转发为 [`TrayAction`]。关闭主窗口时由 MVU 逻辑决定最小化到托盘。
//!
//! 仅 `gui` 特性下编译。构建失败向上返回 `anyhow::Error`（调用方仅告警，不阻断主界面）。

use anyhow::{Context, Result};

use muda::{Menu, MenuEvent, MenuItem};
use tray_icon::Icon;
use tray_icon::TrayIconBuilder;

use crate::app::message::TrayAction;
use crate::audio::send_tray_action;

/// 生成 16×16 的蓝色圆角占位图标（RGBA）。
fn build_icon() -> Icon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - (size as f32 / 2.0);
            let dy = y as f32 - (size as f32 / 2.0);
            let inside = (dx * dx + dy * dy) <= (size as f32 / 2.0).powi(2);
            if inside {
                rgba.extend_from_slice(&[79, 140, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("图标数据合法")
}

/// 构托盘，注册菜单与事件转发。托盘对象用 `mem::forget` 保活至进程退出。
pub fn build_tray() -> Result<()> {
    let menu = Menu::new();

    let play_pause = MenuItem::with_id("play_pause", "播放 / 暂停", true, None);
    let next = MenuItem::with_id("next", "下一首", true, None);
    let prev = MenuItem::with_id("prev", "上一首", true, None);
    let show = MenuItem::with_id("show", "显示主窗口", true, None);
    let mini = MenuItem::with_id("mini", "迷你模式", true, None);
    let quit = MenuItem::with_id("quit", "退出", true, None);

    menu.append_items(&[&play_pause, &prev, &next, &show, &mini, &quit])
        .context("追加托盘菜单项失败")?;

    // 菜单事件 → 全局通道（任意动作都转成 TrayAction 交给 MVU）。
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id: &str = event.id().as_ref();
        let action = match id {
            "play_pause" => TrayAction::PlayPause,
            "next" => TrayAction::Next,
            "prev" => TrayAction::Prev,
            "show" => TrayAction::ShowMainWindow,
            "mini" => TrayAction::MiniMode,
            "quit" => TrayAction::Quit,
            _ => return,
        };
        send_tray_action(action);
    }));

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("LFPlayer")
        .with_icon(build_icon())
        .build()
        .context("创建托盘图标失败")?;

    // 保活托盘至进程退出（不被 drop 回收）。
    std::mem::forget(tray);

    Ok(())
}
