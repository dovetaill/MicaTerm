//! macOS Finder / Termius style Source List Sidebar:
//! Recessed `NSSegmentedControl` (`主机 / 片段 / 密钥`), Spotlight search input,
//! dedicated collapse button, and asset rows with line icons.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, ClipboardItem, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

use crate::domain::{FlatHostRow, KeychainEntry, KeychainKind, ShortcutAction, SnippetItem, tooltip};
use crate::ui::theme::{
    macos_env_badge_colors, macos_segmented_track, macos_squircle_badge,
};
use crate::ui::workspace::MicaWorkspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarSection {
    #[default]
    Hosts,
    Snippets,
    Keys,
}

#[derive(IntoElement)]
pub struct LeftSidebar {
    workspace: Entity<MicaWorkspace>,
    section: SidebarSection,
    search_input: Entity<InputState>,
    host_rows: Vec<FlatHostRow>,
    snippets: Vec<SnippetItem>,
    keychain: Vec<KeychainEntry>,
    selected_id: Option<String>,
}

impl LeftSidebar {
    pub fn new(
        workspace: Entity<MicaWorkspace>,
        section: SidebarSection,
        search_input: Entity<InputState>,
        host_rows: Vec<FlatHostRow>,
        snippets: Vec<SnippetItem>,
        keychain: Vec<KeychainEntry>,
        selected_id: Option<String>,
    ) -> Self {
        Self {
            workspace,
            section,
            search_input,
            host_rows,
            snippets,
            keychain,
            selected_id,
        }
    }
}

#[inline(never)]
fn render_segment_pill(
    id: &'static str,
    label: &'static str,
    active: bool,
    target: SidebarSection,
    workspace: Entity<MicaWorkspace>,
    cx: &App,
) -> AnyElement {
    div()
        .id(id)
        .flex_1()
        .h(px(23.0))
        .rounded(px(5.5))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .bg(if active {
            cx.theme().tab_active
        } else {
            cx.theme().transparent
        })
        .hover(|s| {
            if active {
                s
            } else {
                s.bg(cx.theme().list_hover)
            }
        })
        .on_click(move |_, _, cx| {
            workspace.update(cx, |ws, cx| ws.set_sidebar_section(target, cx));
        })
        .child(
            div()
                .text_xs()
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if active {
                    cx.theme().tab_active_foreground
                } else {
                    cx.theme().muted_foreground
                })
                .child(label),
        )
        .into_any_element()
}

#[inline(never)]
fn render_sidebar_header(
    workspace: Entity<MicaWorkspace>,
    section: SidebarSection,
    cx: &App,
) -> AnyElement {
    let ws_create_host = workspace.clone();
    let ws_create_folder = workspace.clone();
    let ws_create_snippet = workspace.clone();
    let ws_create_key = workspace.clone();
    let ws_collapse = workspace.clone();

    let create_control = match section {
        SidebarSection::Hosts => Button::new("sidebar-create-host-menu")
            .tab_stop(false)
            .ghost()
            .xsmall()
            .compact()
            .icon(LucideIcon::Plus)
            .tooltip("新建主机或分组文件夹")
            .dropdown_menu(move |menu, _, _| {
                let w1 = ws_create_host.clone();
                let w2 = ws_create_folder.clone();
                menu.item(
                    PopupMenuItem::new("新建 SSH 主机")
                        .icon(LucideIcon::Server)
                        .on_click(move |_, window, cx| {
                            w1.update(cx, |ws, cx| ws.open_host_dialog(None, None, window, cx));
                        }),
                )
                .item(
                    PopupMenuItem::new("新建分组文件夹")
                        .icon(LucideIcon::FolderPlus)
                        .on_click(move |_, window, cx| {
                            w2.update(cx, |ws, cx| ws.open_folder_dialog(None, window, cx));
                        }),
                )
            })
            .into_any_element(),
        SidebarSection::Snippets => Button::new("sidebar-create-snippet")
            .tab_stop(false)
            .ghost()
            .xsmall()
            .compact()
            .icon(LucideIcon::Plus)
            .tooltip("新建命令片段")
            .on_click(move |_, window, cx| {
                ws_create_snippet.update(cx, |ws, cx| ws.open_snippet_dialog(None, window, cx));
            })
            .into_any_element(),
        SidebarSection::Keys => Button::new("sidebar-create-key")
            .tab_stop(false)
            .ghost()
            .xsmall()
            .compact()
            .icon(LucideIcon::Plus)
            .tooltip("新建密钥或身份凭据")
            .on_click(move |_, window, cx| {
                ws_create_key.update(cx, |ws, cx| ws.open_keychain_dialog(None, window, cx));
            })
            .into_any_element(),
    };

    h_flex()
        .px_2p5()
        .pt_2p5()
        .pb_2()
        .gap_1p5()
        .items_center()
        .justify_between()
        .child(
            macos_segmented_track(cx)
                .flex_1()
                .child(render_segment_pill(
                    "seg-hosts",
                    "主机",
                    section == SidebarSection::Hosts,
                    SidebarSection::Hosts,
                    workspace.clone(),
                    cx,
                ))
                .child(render_segment_pill(
                    "seg-snippets",
                    "片段",
                    section == SidebarSection::Snippets,
                    SidebarSection::Snippets,
                    workspace.clone(),
                    cx,
                ))
                .child(render_segment_pill(
                    "seg-keys",
                    "密钥",
                    section == SidebarSection::Keys,
                    SidebarSection::Keys,
                    workspace,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_0p5()
                .items_center()
                .child(create_control)
                .child(
                    Button::new("sidebar-collapse-btn")
                        .tab_stop(false)
                        .ghost()
                        .xsmall()
                        .compact()
                        .icon(LucideIcon::PanelLeftClose)
                        .tooltip(tooltip(
                            "隐藏左侧边栏",
                            ws_collapse.read(cx).settings.shortcuts.get(ShortcutAction::ToggleLeftSidebar).as_ref(),
                        ))
                        .on_click(move |_, window, cx| {
                            ws_collapse.update(cx, |ws, cx| ws.toggle_left_sidebar(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

impl RenderOnce for LeftSidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let header = render_sidebar_header(self.workspace.clone(), self.section, cx);
        let body = match self.section {
            SidebarSection::Hosts => render_hosts_list(
                self.workspace,
                self.host_rows,
                self.selected_id,
                cx,
            ),
            SidebarSection::Snippets => render_snippets_list(self.workspace, self.snippets, cx),
            SidebarSection::Keys => render_keys_list(self.workspace, self.keychain, cx),
        };

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(header)
            .child(
                div().px_2p5().pb_2().child(
                    Input::new(&self.search_input)
                        .xsmall()
                        .prefix(
                            Icon::new(LucideIcon::Search)
                                .xsmall()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .cleanable(true),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .px_2()
                    .pb_3()
                    .child(body),
            )
    }
}

#[inline(never)]
fn render_host_row(
    workspace: Entity<MicaWorkspace>,
    row: FlatHostRow,
    is_selected: bool,
    cx: &App,
) -> AnyElement {
    let id = row.id.clone();
    let indent = px((row.depth as f32) * 14.0 + 7.0);
    let is_folder = row.is_folder;

    let ws_click = workspace.clone();
    let id_click = id.clone();

    let ws_ctx_connect = workspace.clone();
    let ws_ctx_split = workspace.clone();
    let ws_ctx_split_v = workspace.clone();
    let ws_ctx_add_child = workspace.clone();
    let ws_ctx_edit = workspace.clone();
    let ws_ctx_clone = workspace.clone();
    let ws_ctx_del = workspace.clone();

    let id_ctx1 = id.clone();
    let id_ctx2 = id.clone();
    let id_ctx2v = id.clone();
    let id_ctx3 = id.clone();
    let id_ctx4 = id.clone();
    let id_ctx5 = id.clone();
    let id_ctx6 = id.clone();

    let leading_visual = if is_folder {
        let folder_tint = cx.theme().cyan;
        h_flex()
            .gap_1()
            .items_center()
            .child(
                Icon::new(if row.expanded {
                    LucideIcon::ChevronDown
                } else {
                    LucideIcon::ChevronRight
                })
                .xsmall()
                .text_color(cx.theme().muted_foreground),
            )
            .child(
                Icon::new(if row.expanded {
                    LucideIcon::FolderOpen
                } else {
                    LucideIcon::Folder
                })
                .small()
                .text_color(folder_tint),
            )
            .into_any_element()
    } else {
        Icon::new(LucideIcon::Server)
            .size(px(16.0))
            .text_color(cx.theme().sidebar_foreground)
            .into_any_element()
    };

    let env_pill = (!row.env_badge.is_empty()).then(|| {
        let (bg, fg) = macos_env_badge_colors(&row.env_badge, cx);
        div()
            .px_1p5()
            .rounded(px(4.5))
            .bg(bg)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(fg)
            .child(row.env_badge)
    });

    h_flex()
        .id(SharedString::from(format!("host-row-{id}")))
        .pl(indent)
        .pr_2()
        .py_1p5()
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(px(7.0))
        .cursor_pointer()
        .bg(if is_selected {
            cx.theme().list_active
        } else {
            cx.theme().transparent
        })
        .hover(|s| {
            if is_selected {
                s
            } else {
                s.bg(cx.theme().list_hover)
            }
        })
        .on_click(move |ev, window, cx| {
            ws_click.update(cx, |ws, cx| {
                ws.selected_sidebar_id = Some(id_click.clone());
                if is_folder {
                    ws.toggle_folder(&id_click, cx);
                } else if ev.click_count() >= 2 {
                    ws.connect_host_in_new_tab(&id_click, window, cx);
                } else {
                    cx.notify();
                }
            });
        })
        .context_menu(move |menu, _, _| {
            if is_folder {
                let w_add = ws_ctx_add_child.clone();
                let fid = id_ctx3.clone();
                let w_del = ws_ctx_del.clone();
                let fid_del = id_ctx6.clone();
                menu.item(
                    PopupMenuItem::new("在此文件夹新建主机")
                        .icon(LucideIcon::Plus)
                        .on_click(move |_, window, cx| {
                            w_add.update(cx, |ws, cx| {
                                ws.open_host_dialog(None, Some(fid.clone()), window, cx)
                            });
                        }),
                )
                .separator()
                .item(PopupMenuItem::new("删除文件夹").on_click(move |_, window, cx| {
                    w_del.update(cx, |ws, cx| {
                        ws.confirm_delete_host_node(&fid_del, window, cx)
                    });
                }))
            } else {
                let w_conn = ws_ctx_connect.clone();
                let hid1 = id_ctx1.clone();
                let w_split = ws_ctx_split.clone();
                let hid2 = id_ctx2.clone();
                let w_split_v = ws_ctx_split_v.clone();
                let hid2v = id_ctx2v.clone();
                let w_edit = ws_ctx_edit.clone();
                let hid4 = id_ctx4.clone();
                let w_clone = ws_ctx_clone.clone();
                let hid5 = id_ctx5.clone();
                let w_del = ws_ctx_del.clone();
                let hid6 = id_ctx6.clone();

                menu.item(
                    PopupMenuItem::new("打开 SSH 连接")
                        .icon(LucideIcon::Play)
                        .on_click(move |_, window, cx| {
                            w_conn.update(cx, |ws, cx| {
                                ws.connect_host_in_new_tab(&hid1, window, cx)
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("在水平分屏中连接")
                        .icon(LucideIcon::Columns2)
                        .on_click(move |_, window, cx| {
                            w_split.update(cx, |ws, cx| {
                                ws.connect_host_in_split(&hid2, true, window, cx)
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("在垂直分屏中连接")
                        .icon(LucideIcon::Rows2)
                        .on_click(move |_, window, cx| {
                            w_split_v.update(cx, |ws, cx| {
                                ws.connect_host_in_split(&hid2v, false, window, cx)
                            });
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("编辑主机")
                        .icon(LucideIcon::Pencil)
                        .on_click(move |_, window, cx| {
                            w_edit.update(cx, |ws, cx| {
                                ws.open_host_dialog(Some(hid4.clone()), None, window, cx)
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("克隆主机")
                        .icon(LucideIcon::Copy)
                        .on_click(move |_, _, cx| {
                            w_clone.update(cx, |ws, cx| ws.clone_host(&hid5, cx));
                        }),
                )
                .separator()
                .item(PopupMenuItem::new("删除主机").on_click(move |_, window, cx| {
                    w_del.update(cx, |ws, cx| {
                        ws.confirm_delete_host_node(&hid6, window, cx)
                    });
                }))
            }
        })
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(leading_visual)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .font_weight(if is_folder {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::MEDIUM
                                        })
                                        .text_color(cx.theme().foreground)
                                        .child(row.name),
                                )
                                .children(env_pill),
                        )
                        .children((!row.subtitle.is_empty()).then(|| {
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(row.subtitle)
                        })),
                ),
        )
        .children((!is_folder).then(|| {
            let ws_play = workspace;
            let hid = id.clone();
            Button::new(SharedString::from(format!("connect-btn-{id}")))
                .tab_stop(false)
                .ghost()
                .xsmall()
                .compact()
                .icon(LucideIcon::Play)
                .tooltip("双击或点击立即连接")
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    ws_play.update(cx, |ws, cx| ws.connect_host_in_new_tab(&hid, window, cx));
                })
        }))
        .into_any_element()
}

#[inline(never)]
fn render_hosts_list(
    workspace: Entity<MicaWorkspace>,
    rows: Vec<FlatHostRow>,
    selected_id: Option<String>,
    cx: &App,
) -> AnyElement {
    if rows.is_empty() {
        let ws = workspace;
        return v_flex()
            .py_10()
            .px_4()
            .gap_3()
            .items_center()
            .child(macos_squircle_badge(
                LucideIcon::Server,
                cx.theme().muted_foreground,
                36.0,
                9.0,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无主机资产"),
            )
            .child(
                Button::new("empty-add-ssh-host")
                    .tab_stop(false)
                    .secondary()
                    .xsmall()
                    .icon(LucideIcon::Plus)
                    .label("新建 SSH 主机")
                    .on_click(move |_, window, cx| {
                        ws.update(cx, |ws, cx| ws.open_host_dialog(None, None, window, cx));
                    }),
            )
            .into_any_element();
    }

    let items: Vec<AnyElement> = rows
        .into_iter()
        .map(|row| {
            let is_sel = selected_id.as_deref() == Some(row.id.as_str());
            render_host_row(workspace.clone(), row, is_sel, cx)
        })
        .collect();

    v_flex().gap_0p5().children(items).into_any_element()
}

#[inline(never)]
fn render_snippet_row(
    workspace: Entity<MicaWorkspace>,
    snip: SnippetItem,
    cx: &App,
) -> AnyElement {
    let sid = snip.id.clone();
    let cmd_preview = snip.command.clone();
    let cmd_copy = snip.command.clone();
    let ws_run = workspace.clone();
    let ws_ctx_run = workspace.clone();
    let ws_ctx_edit = workspace.clone();
    let ws_ctx_del = workspace;
    let sid1 = sid.clone();
    let sid2 = sid.clone();
    let sid3 = sid.clone();
    let sid4 = sid.clone();

    let purple = cx.theme().magenta;

    h_flex()
        .id(SharedString::from(format!("snippet-row-{sid}")))
        .px_2()
        .py_1p5()
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(px(7.0))
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().list_hover))
        .on_click({
            let ws_dbl = ws_run.clone();
            let sid_dbl = sid.clone();
            move |ev, window, cx| {
                if ev.click_count() >= 2 {
                    ws_dbl.update(cx, |ws, cx| ws.run_snippet(&sid_dbl, window, cx));
                }
            }
        })
        .context_menu(move |menu, _, _| {
            let w_run = ws_ctx_run.clone();
            let id_run = sid2.clone();
            let w_edit = ws_ctx_edit.clone();
            let id_edit = sid3.clone();
            let w_del = ws_ctx_del.clone();
            let id_del = sid4.clone();
            let c_copy = cmd_copy.clone();

            menu.item(
                PopupMenuItem::new("发送到当前终端")
                    .icon(LucideIcon::Play)
                    .on_click(move |_, window, cx| {
                        w_run.update(cx, |ws, cx| ws.run_snippet(&id_run, window, cx));
                    }),
            )
            .item(
                PopupMenuItem::new("复制命令")
                    .icon(LucideIcon::Copy)
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(c_copy.clone()));
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("编辑片段")
                    .icon(LucideIcon::Pencil)
                    .on_click(move |_, window, cx| {
                        w_edit.update(cx, |ws, cx| {
                            ws.open_snippet_dialog(Some(id_edit.clone()), window, cx)
                        });
                    }),
            )
            .item(PopupMenuItem::new("删除片段").on_click(move |_, window, cx| {
                w_del.update(cx, |ws, cx| ws.confirm_delete_snippet(&id_del, window, cx));
            }))
        })
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .gap_2()
                .items_center()
                .child(
                    Icon::new(LucideIcon::Code)
                        .size(px(16.0))
                        .text_color(purple),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(snip.title),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(cmd_preview),
                        ),
                ),
        )
        .child(
            Button::new(SharedString::from(format!("run-snip-{sid}")))
                .tab_stop(false)
                .ghost()
                .xsmall()
                .compact()
                .icon(LucideIcon::Play)
                .tooltip("发送到激活终端")
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    ws_run.update(cx, |ws, cx| ws.run_snippet(&sid1, window, cx));
                }),
        )
        .into_any_element()
}

#[inline(never)]
fn render_snippets_list(
    workspace: Entity<MicaWorkspace>,
    snippets: Vec<SnippetItem>,
    cx: &App,
) -> AnyElement {
    if snippets.is_empty() {
        return v_flex()
            .py_10()
            .px_4()
            .gap_2()
            .items_center()
            .child(macos_squircle_badge(
                LucideIcon::Code,
                cx.theme().muted_foreground,
                36.0,
                9.0,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无命令片段，点击右上角「+」添加常用脚本"),
            )
            .into_any_element();
    }

    let items: Vec<AnyElement> = snippets
        .into_iter()
        .map(|s| render_snippet_row(workspace.clone(), s, cx))
        .collect();
    v_flex().gap_0p5().children(items).into_any_element()
}

#[inline(never)]
fn render_key_row(
    workspace: Entity<MicaWorkspace>,
    entry: KeychainEntry,
    cx: &App,
) -> AnyElement {
    let kid = entry.id.clone();
    let kid_edit = kid.clone();
    let kid_del = kid.clone();
    let ws_edit = workspace.clone();
    let ws_del = workspace;
    let subtitle = if entry.username.is_empty() {
        entry.kind.label().to_string()
    } else {
        format!("{} · {}", entry.kind.label(), entry.username)
    };

    let (icon, tint) = match entry.kind {
        KeychainKind::PasswordIdentity => (LucideIcon::Key, cx.theme().warning),
        KeychainKind::SshKey => (LucideIcon::KeyRound, cx.theme().success),
    };

    h_flex()
        .id(SharedString::from(format!("keychain-row-{kid}")))
        .px_2()
        .py_1p5()
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(px(7.0))
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().list_hover))
        .on_click({
            let ws_dbl = ws_edit.clone();
            let id_dbl = kid.clone();
            move |ev, window, cx| {
                if ev.click_count() >= 2 {
                    ws_dbl.update(cx, |ws, cx| {
                        ws.open_keychain_dialog(Some(id_dbl.clone()), window, cx)
                    });
                }
            }
        })
        .context_menu(move |menu, _, _| {
            let w_e = ws_edit.clone();
            let id_e = kid_edit.clone();
            let w_d = ws_del.clone();
            let id_d = kid_del.clone();
            menu.item(
                PopupMenuItem::new("编辑凭据")
                    .icon(LucideIcon::Pencil)
                    .on_click(move |_, window, cx| {
                        w_e.update(cx, |ws, cx| {
                            ws.open_keychain_dialog(Some(id_e.clone()), window, cx)
                        });
                    }),
            )
            .separator()
            .item(PopupMenuItem::new("删除凭据").on_click(move |_, window, cx| {
                w_d.update(cx, |ws, cx| ws.confirm_delete_keychain(&id_d, window, cx));
            }))
        })
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).size(px(16.0)).text_color(tint))
                .child(
                    v_flex()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(entry.name),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(subtitle),
                        ),
                ),
        )
        .into_any_element()
}

#[inline(never)]
fn render_keys_list(
    workspace: Entity<MicaWorkspace>,
    entries: Vec<KeychainEntry>,
    cx: &App,
) -> AnyElement {
    if entries.is_empty() {
        return v_flex()
            .py_10()
            .px_4()
            .gap_2()
            .items_center()
            .child(macos_squircle_badge(
                LucideIcon::KeyRound,
                cx.theme().muted_foreground,
                36.0,
                9.0,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无密钥凭据，点击右上角「+」添加后可跨多台主机共享"),
            )
            .into_any_element();
    }

    let items: Vec<AnyElement> = entries
        .into_iter()
        .map(|e| render_key_row(workspace.clone(), e, cx))
        .collect();
    v_flex().gap_0p5().children(items).into_any_element()
}
