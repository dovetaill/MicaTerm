//! Streamlined single-layer Left Sidebar: segmented `主机 / 片段 / 密钥` tabs, search, and lists.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    App, ClipboardItem, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

use crate::domain::{FlatHostRow, KeychainEntry, KeychainKind, SnippetItem};
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

impl RenderOnce for LeftSidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ws_hosts = self.workspace.clone();
        let ws_snippets = self.workspace.clone();
        let ws_keys = self.workspace.clone();
        let ws_create_host = self.workspace.clone();
        let ws_create_folder = self.workspace.clone();
        let ws_create_snippet = self.workspace.clone();
        let ws_create_key = self.workspace.clone();

        let create_control = match self.section {
            SidebarSection::Hosts => Button::new("sidebar-create-host-menu")
                .ghost()
                .xsmall()
                .compact()
                .icon(LucideIcon::Plus)
                .tooltip("新建主机或文件夹")
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
                        PopupMenuItem::new("新建文件夹")
                            .icon(LucideIcon::FolderPlus)
                            .on_click(move |_, window, cx| {
                                w2.update(cx, |ws, cx| ws.open_folder_dialog(None, window, cx));
                            }),
                    )
                })
                .into_any_element(),
            SidebarSection::Snippets => Button::new("sidebar-create-snippet")
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

        let body = match self.section {
            SidebarSection::Hosts => render_hosts_list(
                self.workspace.clone(),
                self.host_rows,
                self.selected_id.clone(),
                cx,
            ),
            SidebarSection::Snippets => {
                render_snippets_list(self.workspace.clone(), self.snippets, cx)
            }
            SidebarSection::Keys => render_keys_list(self.workspace.clone(), self.keychain, cx),
        };

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                // Top segmented switch + `+` button
                h_flex()
                    .px_2p5()
                    .pt_2p5()
                    .pb_1p5()
                    .gap_1p5()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .flex_1()
                            .p_0p5()
                            .gap_0p5()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().secondary.opacity(0.65))
                            .child(
                                Button::new("seg-hosts")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .selected(self.section == SidebarSection::Hosts)
                                    .label("主机")
                                    .on_click(move |_, _, cx| {
                                        ws_hosts.update(cx, |ws, cx| {
                                            ws.set_sidebar_section(SidebarSection::Hosts, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new("seg-snippets")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .selected(self.section == SidebarSection::Snippets)
                                    .label("片段")
                                    .on_click(move |_, _, cx| {
                                        ws_snippets.update(cx, |ws, cx| {
                                            ws.set_sidebar_section(SidebarSection::Snippets, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new("seg-keys")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .selected(self.section == SidebarSection::Keys)
                                    .label("密钥")
                                    .on_click(move |_, _, cx| {
                                        ws_keys.update(cx, |ws, cx| {
                                            ws.set_sidebar_section(SidebarSection::Keys, cx)
                                        });
                                    }),
                            ),
                    )
                    .child(create_control),
            )
            .child(
                // Search box
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

fn render_hosts_list(
    workspace: Entity<MicaWorkspace>,
    rows: Vec<FlatHostRow>,
    selected_id: Option<String>,
    cx: &App,
) -> gpui_kit::AnyElement {
    if rows.is_empty() {
        let ws = workspace.clone();
        return v_flex()
            .py_8()
            .px_3()
            .gap_3()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无主机资产"),
            )
            .child(
                Button::new("empty-add-ssh-host")
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

    v_flex()
        .gap_0p5()
        .children(rows.into_iter().map(|row| {
            let id = row.id.clone();
            let is_selected = selected_id.as_deref() == Some(id.as_str());
            let indent = px((row.depth as f32) * 14.0 + 8.0);
            let is_folder = row.is_folder;

            let ws_click = workspace.clone();
            let id_click = id.clone();

            let ws_ctx_connect = workspace.clone();
            let ws_ctx_split = workspace.clone();
            let ws_ctx_add_child = workspace.clone();
            let ws_ctx_edit = workspace.clone();
            let ws_ctx_clone = workspace.clone();
            let ws_ctx_del = workspace.clone();

            let id_ctx1 = id.clone();
            let id_ctx2 = id.clone();
            let id_ctx3 = id.clone();
            let id_ctx4 = id.clone();
            let id_ctx5 = id.clone();
            let id_ctx6 = id.clone();

            h_flex()
                .id(SharedString::from(format!("host-row-{id}")))
                .pl(indent)
                .pr_2()
                .py_1p5()
                .gap_2()
                .items_center()
                .justify_between()
                .rounded(cx.theme().radius)
                .cursor_pointer()
                .bg(if is_selected {
                    cx.theme().list_active
                } else {
                    cx.theme().transparent
                })
                .hover(|s| s.bg(cx.theme().list_hover))
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
                        .item(PopupMenuItem::new("删除文件夹").on_click(
                            move |_, _, cx| {
                                w_del.update(cx, |ws, cx| ws.delete_host_node(&fid_del, cx));
                            },
                        ))
                    } else {
                        let w_conn = ws_ctx_connect.clone();
                        let hid1 = id_ctx1.clone();
                        let w_split = ws_ctx_split.clone();
                        let hid2 = id_ctx2.clone();
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
                        .item(PopupMenuItem::new("删除主机").on_click(move |_, _, cx| {
                            w_del.update(cx, |ws, cx| ws.delete_host_node(&hid6, cx));
                        }))
                    }
                })
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .child(if is_folder {
                            Icon::new(if row.expanded {
                                LucideIcon::FolderOpen
                            } else {
                                LucideIcon::Folder
                            })
                            .small()
                            .text_color(cx.theme().primary)
                        } else {
                            Icon::new(LucideIcon::Server)
                                .small()
                                .text_color(cx.theme().muted_foreground)
                        })
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
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().foreground)
                                                .child(row.name),
                                        )
                                        .children((!row.env_badge.is_empty()).then(|| {
                                            div()
                                                .px_1()
                                                .rounded(cx.theme().radius)
                                                .bg(cx.theme().secondary)
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(row.env_badge)
                                        })),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(row.subtitle),
                                ),
                        ),
                )
                .children((!is_folder).then(|| {
                    let ws_play = workspace.clone();
                    let hid = id.clone();
                    Button::new(SharedString::from(format!("connect-btn-{id}")))
                        .ghost()
                        .xsmall()
                        .compact()
                        .icon(LucideIcon::Play)
                        .tooltip("立即连接")
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            ws_play.update(cx, |ws, cx| {
                                ws.connect_host_in_new_tab(&hid, window, cx)
                            });
                        })
                }))
        }))
        .into_any_element()
}

fn render_snippets_list(
    workspace: Entity<MicaWorkspace>,
    snippets: Vec<SnippetItem>,
    cx: &App,
) -> gpui_kit::AnyElement {
    if snippets.is_empty() {
        return div()
            .py_8()
            .px_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("暂无命令片段，点击右上角「+」添加常用脚本。")
            .into_any_element();
    }

    v_flex()
        .gap_1p5()
        .children(snippets.into_iter().map(|snip| {
            let sid = snip.id.clone();
            let cmd_preview = snip.command.clone();
            let cmd_copy = snip.command.clone();
            let ws_run = workspace.clone();
            let ws_ctx_run = workspace.clone();
            let ws_ctx_edit = workspace.clone();
            let ws_ctx_del = workspace.clone();
            let sid1 = sid.clone();
            let sid2 = sid.clone();
            let sid3 = sid.clone();
            let sid4 = sid.clone();

            v_flex()
                .id(SharedString::from(format!("snippet-row-{sid}")))
                .p_2()
                .gap_1()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border.opacity(0.6))
                .hover(|s| s.bg(cx.theme().list_hover))
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
                            .on_click(move |_, _, cx| {
                                w_run.update(cx, |ws, cx| ws.run_snippet(&id_run, cx));
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
                    .item(PopupMenuItem::new("删除片段").on_click(move |_, _, cx| {
                        w_del.update(cx, |ws, cx| ws.delete_snippet(&id_del, cx));
                    }))
                })
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_1()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    Icon::new(LucideIcon::Code)
                                        .xsmall()
                                        .text_color(cx.theme().primary),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(cx.theme().foreground)
                                        .child(snip.title),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("run-snip-{sid}")))
                                .ghost()
                                .xsmall()
                                .compact()
                                .icon(LucideIcon::Play)
                                .tooltip("发送到激活终端")
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    ws_run.update(cx, |ws, cx| ws.run_snippet(&sid1, cx));
                                }),
                        ),
                )
                .child(
                    div()
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(cmd_preview),
                )
        }))
        .into_any_element()
}

fn render_keys_list(
    workspace: Entity<MicaWorkspace>,
    entries: Vec<KeychainEntry>,
    cx: &App,
) -> gpui_kit::AnyElement {
    if entries.is_empty() {
        return div()
            .py_8()
            .px_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("暂无复用的密钥与身份凭据，点击「+」添加后可在多台 SSH 主机中共享。")
            .into_any_element();
    }

    v_flex()
        .gap_1()
        .children(entries.into_iter().map(|entry| {
            let kid = entry.id.clone();
            let kid_edit = kid.clone();
            let kid_del = kid.clone();
            let ws_edit = workspace.clone();
            let ws_del = workspace.clone();
            let subtitle = if entry.username.is_empty() {
                entry.kind.label().to_string()
            } else {
                format!("{} · {}", entry.kind.label(), entry.username)
            };

            h_flex()
                .id(SharedString::from(format!("keychain-row-{kid}")))
                .px_2p5()
                .py_2()
                .gap_2()
                .items_center()
                .justify_between()
                .rounded(cx.theme().radius)
                .hover(|s| s.bg(cx.theme().list_hover))
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
                    .item(PopupMenuItem::new("删除凭据").on_click(move |_, _, cx| {
                        w_d.update(cx, |ws, cx| ws.delete_keychain(&id_d, cx));
                    }))
                })
                .child(
                    h_flex()
                        .min_w_0()
                        .flex_1()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(match entry.kind {
                                KeychainKind::PasswordIdentity => LucideIcon::Key,
                                KeychainKind::SshKey => LucideIcon::KeyRound,
                            })
                            .small()
                            .text_color(cx.theme().primary),
                        )
                        .child(
                            v_flex()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
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
        }))
        .into_any_element()
}
