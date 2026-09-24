//! 独立 WebDAV 云端管理与缓存控制台窗口。
//!
//! 包含三大模块：
//! 1. 服务器配置管理（添加、删除、测试连通性）；
//! 2. 云端目录资源浏览器（层级导航、单曲/批量导入播放列表）；
//! 3. 磁盘持久化缓存中心（空间统计、配额调节、一键清空）。

use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, space, text, text_input,
};
use iced::{Background, Border, Color, Element, Length};

use crate::app::message::{AppMessage, WebDavTab};
use crate::app::state::AppState;
use crate::ui::style::{
    hifi_scrollable_style, hifi_text_input_style, primary_play_button_style, subtle_button_style,
    Palette,
};

/// 渲染 WebDAV 独立控制台主视图。
pub fn webdav_window_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    // 1. 顶栏标题与关闭动作
    let title_label =
        text("☁ WebDAV 云端音乐与流媒体缓存管理")
            .size(15.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            });

    let close_btn = button(text("×").size(15.0))
        .padding([2, 8])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::CloseWebDavWindow);

    let header = row![title_label, space::horizontal(), close_btn]
        .align_y(iced::alignment::Vertical::Center)
        .padding([12, 16]);

    // 2. Tab 分页切换栏
    let tab_btn = |label: &'static str, tab: WebDavTab| {
        let is_active = state.webdav_tab == tab;
        button(
            text(label)
                .size(12.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(if is_active {
                        palette.accent
                    } else {
                        palette.text_sub
                    }),
                }),
        )
        .padding([6, 14])
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(if is_active {
                palette.surface_elevated
            } else {
                Color::TRANSPARENT
            })),
            border: Border {
                color: if is_active {
                    palette.accent_subtle
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .on_press(AppMessage::SwitchWebDavTab(tab))
    };

    let tab_bar = container(
        row![
            tab_btn("1. 服务器配置", WebDavTab::Servers),
            tab_btn("2. 云端资源浏览", WebDavTab::Explorer),
            tab_btn("3. 缓存与缓冲中心", WebDavTab::Cache),
        ]
        .spacing(8),
    )
    .padding(iced::Padding {
        top: 0.0,
        right: 16.0,
        bottom: 8.0,
        left: 16.0,
    });

    // 3. Tab 主体内容
    let content: Element<'_, AppMessage> = match state.webdav_tab {
        WebDavTab::Servers => servers_tab_view(state, palette),
        WebDavTab::Explorer => explorer_tab_view(state, palette),
        WebDavTab::Cache => cache_tab_view(state, palette),
    };

    container(
        column![header, tab_bar, content]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(move |_theme| container::Style {
        background: Some(Background::Color(palette.bg)),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// ── 分页 1：服务器配置管理 ──
fn servers_tab_view<'a>(state: &'a AppState, palette: Palette) -> Element<'a, AppMessage> {
    let mut server_cards = Vec::new();

    if state.settings.webdav_servers.is_empty() {
        server_cards.push(
            container(
                column![
                    text("暂未添加 WebDAV 服务器").size(13.0).style(
                        move |_theme: &iced::Theme| text::Style {
                            color: Some(palette.text_sub),
                        }
                    ),
                    text("支持挂载群晖/QNAP/TrueNAS、Alist、Nextcloud、坚果云等各种支持 WebDAV 协议的设备。")
                        .size(11.0)
                        .style(move |_theme: &iced::Theme| text::Style {
                            color: Some(palette.text_muted),
                        }),
                ]
                .spacing(4),
            )
            .padding(16)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(palette.surface)),
                border: Border::default().rounded(6.0),
                ..Default::default()
            })
            .width(Length::Fill)
            .into(),
        );
    } else {
        for (i, srv) in state.settings.webdav_servers.iter().enumerate() {
            let is_selected = state.selected_webdav_server == i;

            let srv_name = text(&srv.name)
                .size(13.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(if is_selected {
                        palette.accent
                    } else {
                        palette.text_main
                    }),
                });

            let srv_info = text(format!("{} (用户: {})", srv.endpoint, srv.username))
                .size(11.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_muted),
                });

            let test_btn = button(text("测试连接").size(11.0))
                .padding([3, 8])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::TestWebDavConnection(i));

            let browse_btn = button(text("浏览文件").size(11.0))
                .padding([3, 8])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::SelectWebDavServer(i));

            let del_btn = button(text("删除").size(11.0))
                .padding([3, 8])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => button::Style {
                        background: Some(Background::Color(Color::from_rgba8(239, 68, 68, 0.2))),
                        text_color: Color::from_rgb8(239, 68, 68),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    },
                    _ => button::Style {
                        text_color: palette.text_muted,
                        ..Default::default()
                    },
                })
                .on_press(AppMessage::DeleteWebDavServer(i));

            let mut action_row = row![test_btn, browse_btn, del_btn]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center);

            if is_selected {
                if let Some(ref st) = state.webdav_test_status {
                    action_row =
                        action_row.push(text(st).size(11.0).style(move |_theme: &iced::Theme| {
                            text::Style {
                                color: Some(palette.vfd_green),
                            }
                        }));
                }
            }

            let card = container(
                row![
                    column![srv_name, srv_info].spacing(2),
                    space::horizontal(),
                    action_row
                ]
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill),
            )
            .padding(12)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(if is_selected {
                    palette.surface_elevated
                } else {
                    palette.surface
                })),
                border: Border {
                    color: if is_selected {
                        palette.accent_subtle
                    } else {
                        palette.border
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .width(Length::Fill);

            server_cards.push(card.into());
        }
    }

    // 表单：新增 WebDAV 服务器
    let form_title = text("添加新 WebDAV 服务器")
        .size(13.0)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_main),
        });

    let name_input = text_input("服务名称（如：家庭群晖 NAS）", &state.webdav_form_name)
        .on_input(AppMessage::SetWebDavFormName)
        .size(12.0)
        .padding(8)
        .style(hifi_text_input_style(palette));

    let endpoint_input = text_input(
        "WebDAV 地址（如：http://192.168.1.50:5244/dav）",
        &state.webdav_form_endpoint,
    )
    .on_input(AppMessage::SetWebDavFormEndpoint)
    .size(12.0)
    .padding(8)
    .style(hifi_text_input_style(palette));

    let user_input = text_input("用户名", &state.webdav_form_username)
        .on_input(AppMessage::SetWebDavFormUsername)
        .size(12.0)
        .padding(8)
        .style(hifi_text_input_style(palette));

    let pwd_input = text_input("密码", &state.webdav_form_password)
        .on_input(AppMessage::SetWebDavFormPassword)
        .secure(true)
        .size(12.0)
        .padding(8)
        .style(hifi_text_input_style(palette));

    let insecure_check = checkbox(state.webdav_form_allow_insecure)
        .on_toggle(AppMessage::SetWebDavFormAllowInsecure)
        .size(14.0);

    let insecure_row = row![
        insecure_check,
        text("允许自签 / 无效证书（局域网内网 HTTPS 推荐勾选）")
            .size(11.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            })
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let save_btn = button(text("＋ 保存并添加服务器").size(12.0))
        .padding([8, 16])
        .style(primary_play_button_style(palette))
        .on_press(AppMessage::SaveWebDavServer);

    let form_card = container(
        column![
            form_title,
            row![name_input, endpoint_input].spacing(8),
            row![user_input, pwd_input].spacing(8),
            row![insecure_row, space::horizontal(), save_btn]
                .align_y(iced::alignment::Vertical::Center),
        ]
        .spacing(10),
    )
    .padding(14)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border::default().rounded(6.0),
        ..Default::default()
    })
    .width(Length::Fill);

    scrollable(
        column![
            column(server_cards).spacing(8),
            space::vertical().height(8),
            form_card,
        ]
        .padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 16.0,
            left: 16.0,
        })
        .spacing(8),
    )
    .style(hifi_scrollable_style(palette))
    .height(Length::Fill)
    .into()
}

/// ── 分页 2：云端资源文件浏览器 ──
fn explorer_tab_view<'a>(state: &'a AppState, palette: Palette) -> Element<'a, AppMessage> {
    if state.settings.webdav_servers.is_empty() {
        return container(
            text("请先在「1. 服务器配置」中添加至少一台 WebDAV 服务器。")
                .size(13.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_sub),
                }),
        )
        .padding(32)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into();
    }

    let active_server = state
        .settings
        .webdav_servers
        .get(state.selected_webdav_server);
    let srv_name = active_server.map(|s| s.name.as_str()).unwrap_or("未选择");

    let path_label = text(format!(
        "当前服务器: [{}]  路径: {}",
        srv_name, state.webdav_current_path
    ))
    .size(11.5)
    .style(move |_theme: &iced::Theme| text::Style {
        color: Some(palette.text_sub),
    });

    let parent_path = get_parent_dir(&state.webdav_current_path);
    let up_btn = button(text("⬆ 返回上级").size(11.0))
        .padding([4, 8])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ExploreWebDavDir(parent_path));

    let import_all_btn = button(text("＋ 导入本页全部音频").size(11.0))
        .padding([4, 10])
        .style(primary_play_button_style(palette))
        .on_press(AppMessage::ImportAllRemoteAudios);

    let nav_row = row![path_label, space::horizontal(), up_btn, import_all_btn]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .padding([4, 16]);

    let body: Element<'_, AppMessage> = if state.webdav_is_loading {
        container(
            text("正在从 WebDAV 服务器读取资源，请稍候...")
                .size(12.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.accent),
                }),
        )
        .padding(40)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into()
    } else if state.webdav_remote_items.is_empty() {
        container(
            text("当前目录下暂无文件或暂未读取。点击右上角刷新。")
                .size(12.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_muted),
                }),
        )
        .padding(40)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into()
    } else {
        let items: Vec<Element<'_, AppMessage>> = state
            .webdav_remote_items
            .iter()
            .map(|item| {
                let icon_str = if item.is_dir {
                    "📁 "
                } else if item.is_audio_file() {
                    "🎵 "
                } else if item.is_lyrics_file() {
                    "📄 "
                } else {
                    "• "
                };

                let name_txt = text(format!("{}{}", icon_str, item.name))
                    .size(12.0)
                    .width(Length::Fill)
                    .style(move |_theme: &iced::Theme| text::Style {
                        color: Some(if item.is_dir {
                            palette.accent
                        } else if item.is_audio_file() {
                            palette.text_main
                        } else {
                            palette.text_muted
                        }),
                    });

                let size_txt = if item.is_dir {
                    String::from("目录")
                } else if item.size > 0 {
                    format!("{:.1} MB", item.size as f64 / 1_048_576.0)
                } else {
                    String::new()
                };

                let size_label = text(size_txt).size(10.5).width(Length::Fixed(60.0)).style(
                    move |_theme: &iced::Theme| text::Style {
                        color: Some(palette.text_muted),
                    },
                );

                let action_btn: Element<'_, AppMessage> = if item.is_dir {
                    let sub_path = build_subpath(&state.webdav_current_path, &item.name);
                    button(text("打开").size(10.5))
                        .padding([2, 6])
                        .style(subtle_button_style(palette))
                        .on_press(AppMessage::ExploreWebDavDir(sub_path))
                        .into()
                } else if item.is_audio_file() {
                    button(text("＋ 加入列表").size(10.5))
                        .padding([2, 6])
                        .style(subtle_button_style(palette))
                        .on_press(AppMessage::ImportRemoteTrack(item.clone()))
                        .into()
                } else {
                    space::horizontal().width(1.0).into()
                };

                container(
                    row![name_txt, size_label, action_btn]
                        .spacing(8)
                        .align_y(iced::alignment::Vertical::Center)
                        .width(Length::Fill),
                )
                .padding([4, 8])
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(palette.surface)),
                    border: Border::default().rounded(4.0),
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            })
            .collect();

        scrollable(column(items).spacing(3).padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 16.0,
            left: 16.0,
        }))
        .style(hifi_scrollable_style(palette))
        .height(Length::Fill)
        .into()
    };

    column![nav_row, body]
        .spacing(4)
        .height(Length::Fill)
        .into()
}

/// ── 分页 3：缓存与缓冲中心 ──
fn cache_tab_view<'a>(state: &'a AppState, palette: Palette) -> Element<'a, AppMessage> {
    let cache_dir = crate::cache::CacheManager::default_dir();
    let cache_dir_str = cache_dir.to_string_lossy().into_owned();

    let used_mb = state.cache_used_bytes as f64 / 1_048_576.0;
    let limit_str = if state.settings.cache_config.max_size_mb == 0 {
        "无限制".to_string()
    } else {
        format!(
            "{} MB ({:.1} GB)",
            state.settings.cache_config.max_size_mb,
            state.settings.cache_config.max_size_mb as f64 / 1024.0
        )
    };

    let title_1 = text("本地磁盘持久化缓存")
        .size(13.0)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_main),
        });

    let desc_1 = text("播放在线或 WebDAV 音乐时，系统自动边下边播并分块保存在本地磁盘。完整缓存后二次播放完全 0 流量毫秒级起播。")
        .size(11.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_muted),
        });

    let path_label = text(format!("缓存物理存储路径：{}", cache_dir_str))
        .size(10.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_sub),
        });

    let usage_label = text(format!(
        "当前已占用空间：{:.2} MB  /  配额上限：{}",
        used_mb, limit_str
    ))
    .size(12.5)
    .style(move |_theme: &iced::Theme| text::Style {
        color: Some(palette.vfd_green),
    });

    let limits = vec![
        "500 MB",
        "1024 MB (1.0 GB)",
        "2048 MB (2.0 GB - 默认)",
        "5120 MB (5.0 GB)",
        "10240 MB (10.0 GB)",
        "无限制 (0 MB)",
    ];

    let current_pick = match state.settings.cache_config.max_size_mb {
        500 => "500 MB",
        1024 => "1024 MB (1.0 GB)",
        2048 => "2048 MB (2.0 GB - 默认)",
        5120 => "5120 MB (5.0 GB)",
        10240 => "10240 MB (10.0 GB)",
        _ => "无限制 (0 MB)",
    };

    let limit_picker = pick_list(limits, Some(current_pick), |choice| {
        let mb = match choice {
            "500 MB" => 500,
            "1024 MB (1.0 GB)" => 1024,
            "2048 MB (2.0 GB - 默认)" => 2048,
            "5120 MB (5.0 GB)" => 5120,
            "10240 MB (10.0 GB)" => 10240,
            _ => 0,
        };
        AppMessage::SetCacheLimitMb(mb)
    })
    .text_size(11.0)
    .padding([4, 8]);

    let clear_btn = button(text("清空全部磁盘缓存").size(11.5))
        .padding([6, 14])
        .style(move |_theme, status| match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color::from_rgba8(239, 68, 68, 0.25))),
                text_color: Color::from_rgb8(239, 68, 68),
                border: Border::default().rounded(4.0),
                ..Default::default()
            },
            _ => button::Style {
                background: Some(Background::Color(Color::from_rgba8(239, 68, 68, 0.12))),
                text_color: Color::from_rgb8(239, 68, 68),
                border: Border::default().rounded(4.0),
                ..Default::default()
            },
        })
        .on_press(AppMessage::ClearDiskCache);

    let mut card_content = column![
        title_1,
        desc_1,
        path_label,
        usage_label,
        row![
            text("调整最大缓存配额：").size(11.5),
            limit_picker,
            space::horizontal(),
            clear_btn
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    ]
    .spacing(12);

    if let Some(ref msg) = state.cache_status_msg {
        card_content =
            card_content.push(text(msg).size(11.0).style(move |_theme: &iced::Theme| {
                text::Style {
                    color: Some(palette.accent),
                }
            }));
    }

    let card = container(card_content)
        .padding(16)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            border: Border::default().rounded(6.0),
            ..Default::default()
        })
        .width(Length::Fill);

    column![card]
        .padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 16.0,
            left: 16.0,
        })
        .into()
}

/// 路径辅助：获取父级目录。
fn get_parent_dir(path: &str) -> String {
    let clean = path.trim_matches('/');
    if clean.is_empty() {
        return "/".to_string();
    }
    match clean.rfind('/') {
        Some(idx) => format!("/{}", &clean[..idx]),
        None => "/".to_string(),
    }
}

/// 路径辅助：拼接子路径。
fn build_subpath(current: &str, child: &str) -> String {
    let base = current.trim_end_matches('/');
    let clean_child = child.trim_matches('/');
    if base.is_empty() || base == "/" {
        format!("/{}", clean_child)
    } else {
        format!("{}/{}", base, clean_child)
    }
}
