//! Context-aware Right SFTP Panel with directory follow, file operations, and inline transfer status.

use std::path::PathBuf;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AppContext as _, ClipboardItem, Context, Entity, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::sftp::{
    RemoteEntry, SftpService, TransferDirection, TransferRecord, TransferStatus, format_bytes,
    join_remote_path, parent_remote_path,
};
use crate::ui::modals::{open_remote_editor_dialog, open_simple_name_dialog};

pub struct SftpPanelView {
    service: Option<SftpService>,
    bound_session_label: String,
    pub current_path: String,
    pub follow_cwd: bool,
    pub loading: bool,
    pub error_message: Option<String>,
    pub entries: Vec<RemoteEntry>,
    pub selected_path: Option<String>,
    pub transfers: Vec<TransferRecord>,
    path_input: Option<Entity<InputState>>,
    downloads_dir: PathBuf,
    tokio_handle: tokio::runtime::Handle,
}

impl SftpPanelView {
    pub fn new(
        follow_cwd: bool,
        downloads_dir: PathBuf,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            service: None,
            bound_session_label: String::new(),
            current_path: "/".to_string(),
            follow_cwd,
            loading: false,
            error_message: None,
            entries: Vec::new(),
            selected_path: None,
            transfers: Vec::new(),
            path_input: None,
            downloads_dir,
            tokio_handle,
        }
    }

    pub fn has_active_transfers(&self) -> bool {
        self.transfers
            .iter()
            .any(|t| matches!(t.status, TransferStatus::Running))
    }

    pub fn bind_session(
        &mut self,
        label: String,
        service: Option<SftpService>,
        initial_dir: Option<String>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.bound_session_label = label;
        self.service = service;
        self.error_message = None;
        self.selected_path = None;
        if self.service.is_none() {
            self.entries.clear();
            self.loading = false;
            cx.notify();
            return;
        }
        let dir = initial_dir
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/".to_string());
        self.navigate_to(dir, window, cx);
    }

    pub fn on_terminal_cwd_changed(
        &mut self,
        new_cwd: String,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.follow_cwd && self.service.is_some() && new_cwd != self.current_path {
            self.navigate_to(new_cwd, window, cx);
        }
    }

    pub fn navigate_to(
        &mut self,
        path: String,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let normalized = if path.trim().is_empty() {
            "/".to_string()
        } else {
            path.trim().to_string()
        };
        self.current_path = normalized.clone();
        self.loading = true;
        self.error_message = None;

        if let (Some(win), Some(input)) = (window, self.path_input.clone()) {
            let p = normalized.clone();
            input.update(cx, |st, cx| st.set_value(p, win, cx));
        }

        let rt = self.tokio_handle.clone();
        cx.spawn(async move |weak, cx| {
            let res = rt
                .spawn(async move { service.list_dir(&normalized).await })
                .await;
            let _ = weak.update(cx, |this, cx| {
                this.loading = false;
                match res {
                    Ok(Ok(items)) => {
                        this.entries = items;
                        this.error_message = None;
                    }
                    Ok(Err(err)) => {
                        this.error_message = Some(format!("{err:#}"));
                    }
                    Err(join_err) => {
                        this.error_message = Some(join_err.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        self.navigate_to(path, None, cx);
    }

    pub fn go_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = parent_remote_path(&self.current_path) {
            self.navigate_to(parent, Some(window), cx);
        }
    }

    fn ensure_path_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(existing) = &self.path_input {
            if existing.read(cx).value().as_ref() != self.current_path
                && !existing.read(cx).focus_handle(cx).is_focused(window)
            {
                let cur = self.current_path.clone();
                existing.update(cx, |st, cx| st.set_value(cur, window, cx));
            }
            return existing.clone();
        }
        let initial = self.current_path.clone();
        let input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).placeholder("/远程路径");
            st.set_value(initial, window, cx);
            st
        });
        cx.subscribe_in(&input, window, |this, state, ev, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                let target = state.read(cx).value().to_string();
                this.navigate_to(target, Some(window), cx);
            }
        })
        .detach();
        self.path_input = Some(input.clone());
        input
    }

    fn trigger_upload(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let target_dir = self.current_path.clone();
        let rt = self.tokio_handle.clone();

        cx.spawn(async move |weak, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("选择要上传到远程目录的文件")
                .pick_files()
                .await;
            let Some(files) = picked else {
                return;
            };

            for handle in files {
                let local_path = handle.path().to_path_buf();
                let file_name = handle.file_name();
                let remote_path = join_remote_path(&target_dir, &file_name);
                let tid = uuid::Uuid::new_v4().to_string();

                let _ = weak.update(cx, |this, cx| {
                    this.transfers.insert(
                        0,
                        TransferRecord {
                            id: tid.clone(),
                            direction: TransferDirection::Upload,
                            file_name: file_name.clone(),
                            bytes_done: 0,
                            bytes_total: None,
                            status: TransferStatus::Running,
                        },
                    );
                    cx.notify();
                });

                let svc = service.clone();
                let res = rt
                    .spawn(async move { svc.upload_local_file(&local_path, &remote_path).await })
                    .await;

                let _ = weak.update(cx, |this, cx| {
                    if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid) {
                        match res {
                            Ok(Ok(bytes)) => {
                                rec.bytes_done = bytes;
                                rec.bytes_total = Some(bytes);
                                rec.status = TransferStatus::Completed;
                            }
                            Ok(Err(err)) => {
                                rec.status = TransferStatus::Failed(err.to_string());
                            }
                            Err(err) => {
                                rec.status = TransferStatus::Failed(err.to_string());
                            }
                        }
                    }
                    this.refresh(cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn trigger_download(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let default_dir = self.downloads_dir.clone();
        let rt = self.tokio_handle.clone();

        cx.spawn(async move |weak, cx| {
            let save_target = rfd::AsyncFileDialog::new()
                .set_title("保存远程文件到本地")
                .set_directory(&default_dir)
                .set_file_name(&entry.name)
                .save_file()
                .await;
            let Some(file_handle) = save_target else {
                return;
            };
            let local_path = file_handle.path().to_path_buf();
            let tid = uuid::Uuid::new_v4().to_string();
            let fname = entry.name.clone();
            let expected = entry.size_bytes;

            let _ = weak.update(cx, |this, cx| {
                this.transfers.insert(
                    0,
                    TransferRecord {
                        id: tid.clone(),
                        direction: TransferDirection::Download,
                        file_name: fname,
                        bytes_done: 0,
                        bytes_total: expected,
                        status: TransferStatus::Running,
                    },
                );
                cx.notify();
            });

            let remote_path = entry.path.clone();
            let res = rt
                .spawn(async move { service.download_to_local(&remote_path, &local_path).await })
                .await;

            let _ = weak.update(cx, |this, cx| {
                if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid) {
                    match res {
                        Ok(Ok(bytes)) => {
                            rec.bytes_done = bytes;
                            rec.bytes_total = Some(bytes);
                            rec.status = TransferStatus::Completed;
                        }
                        Ok(Err(err)) => {
                            rec.status = TransferStatus::Failed(err.to_string());
                        }
                        Err(err) => {
                            rec.status = TransferStatus::Failed(err.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_mkdir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let base_dir = self.current_path.clone();
        let rt = self.tokio_handle.clone();
        let panel = cx.entity().clone();

        open_simple_name_dialog(
            "新建远程文件夹",
            "文件夹名称",
            "",
            window,
            cx,
            move |folder_name, _, cx| {
                let full_path = join_remote_path(&base_dir, &folder_name);
                let svc = service.clone();
                let rt_inner = rt.clone();
                panel.update(cx, |_, cx| {
                    cx.spawn(async move |weak, cx| {
                        let res = rt_inner.spawn(async move { svc.mkdir(&full_path).await }).await;
                        let _ = weak.update(cx, |this, cx| {
                            if let Ok(Err(err)) = res {
                                this.error_message = Some(format!("{err:#}"));
                            }
                            this.refresh(cx);
                            cx.notify();
                        });
                    })
                    .detach();
                });
            },
        );
    }

    fn prompt_rename(&mut self, entry: RemoteEntry, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let parent_dir = parent_remote_path(&entry.path).unwrap_or_else(|| "/".to_string());
        let old_path = entry.path.clone();
        let rt = self.tokio_handle.clone();
        let panel = cx.entity().clone();

        open_simple_name_dialog(
            "重命名远程条目",
            "新名称",
            &entry.name,
            window,
            cx,
            move |new_name, _, cx| {
                let new_path = join_remote_path(&parent_dir, &new_name);
                let svc = service.clone();
                let from = old_path.clone();
                let rt_inner = rt.clone();
                panel.update(cx, |_, cx| {
                    cx.spawn(async move |weak, cx| {
                        let res = rt_inner
                            .spawn(async move { svc.rename(&from, &new_path).await })
                            .await;
                        let _ = weak.update(cx, |this, cx| {
                            if let Ok(Err(err)) = res {
                                this.error_message = Some(format!("{err:#}"));
                            }
                            this.refresh(cx);
                            cx.notify();
                        });
                    })
                    .detach();
                });
            },
        );
    }

    fn delete_entry(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let rt = self.tokio_handle.clone();
        let path = entry.path.clone();
        let is_dir = entry.kind.is_dir();
        cx.spawn(async move |weak, cx| {
            let res = rt
                .spawn(async move { service.remove(&path, is_dir).await })
                .await;
            let _ = weak.update(cx, |this, cx| {
                if let Ok(Err(err)) = res {
                    this.error_message = Some(format!("{err:#}"));
                }
                this.refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn open_remote_file_editor(
        &mut self,
        entry: RemoteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let rt = self.tokio_handle.clone();
        let path = entry.path.clone();
        let file_name = entry.name.clone();
        let win_handle = window.window_handle();

        cx.spawn(async move |weak, cx| {
            let svc_read = service.clone();
            let path_read = path.clone();
            let res = rt
                .spawn(async move { svc_read.read_text(&path_read).await })
                .await;

            match res {
                Ok(Ok(content)) => {
                    let _ = cx.update_window(win_handle, |_, window, cx| {
                        let svc_save = service.clone();
                        let path_save = path.clone();
                        let rt_save = rt.clone();
                        let weak_panel = weak.clone();
                        open_remote_editor_dialog(
                            &file_name,
                            &path,
                            &content,
                            window,
                            cx,
                            move |updated_text, _, cx| {
                                let s = svc_save.clone();
                                let p = path_save.clone();
                                let r = rt_save.clone();
                                if let Some(panel_ent) = weak_panel.upgrade() {
                                    panel_ent.update(cx, |_, cx| {
                                        cx.spawn(async move |w, cx| {
                                            let write_res = r
                                                .spawn(async move { s.write_text(&p, updated_text).await })
                                                .await;
                                            let _ = w.update(cx, |this, cx| {
                                                if let Ok(Err(e)) = write_res {
                                                    this.error_message = Some(format!("{e:#}"));
                                                }
                                                this.refresh(cx);
                                                cx.notify();
                                            });
                                        })
                                        .detach();
                                    });
                                }
                            },
                        );
                    });
                }
                Ok(Err(err)) => {
                    let _ = weak.update(cx, |this, cx| {
                        this.error_message = Some(format!("{err:#}"));
                        cx.notify();
                    });
                }
                Err(err) => {
                    let _ = weak.update(cx, |this, cx| {
                        this.error_message = Some(err.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

impl Render for SftpPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.service.is_some();
        let can_up = connected && parent_remote_path(&self.current_path).is_some();
        let path_input = self.ensure_path_input(window, cx);
        let follow = self.follow_cwd;
        let session_badge = if connected {
            self.bound_session_label.clone()
        } else {
            "未绑定 SSH 会话".to_string()
        };

        let view_follow = cx.entity().clone();
        let view_up = cx.entity().clone();
        let view_refresh = cx.entity().clone();
        let view_upload = cx.entity().clone();
        let view_mkdir = cx.entity().clone();
        let view_clear_tx = cx.entity().clone();

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .border_l_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                // Header: bound session + follow toggle + quick actions
                v_flex()
                    .px_3()
                    .pt_2p5()
                    .pb_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
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
                                        Icon::new(LucideIcon::FolderOpen)
                                            .small()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!("SFTP · {session_badge}")),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("sftp-follow-toggle")
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .selected(follow)
                                            .icon(LucideIcon::Link)
                                            .tooltip("跟随终端目录 (OSC 7)")
                                            .on_click(move |_, _, cx| {
                                                view_follow.update(cx, |this, cx| {
                                                    this.follow_cwd = !this.follow_cwd;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("sftp-up")
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .disabled(!can_up)
                                            .icon(LucideIcon::ArrowUp)
                                            .tooltip("上级目录")
                                            .on_click(move |_, window, cx| {
                                                view_up.update(cx, |this, cx| this.go_up(window, cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("sftp-refresh")
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .disabled(!connected)
                                            .icon(LucideIcon::RefreshCw)
                                            .tooltip("刷新目录")
                                            .on_click(move |_, _, cx| {
                                                view_refresh.update(cx, |this, cx| this.refresh(cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("sftp-upload")
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .disabled(!connected)
                                            .icon(LucideIcon::Upload)
                                            .tooltip("上传本地文件")
                                            .on_click(move |_, _, cx| {
                                                view_upload.update(cx, |this, cx| {
                                                    this.trigger_upload(cx)
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("sftp-mkdir")
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .disabled(!connected)
                                            .icon(LucideIcon::FolderPlus)
                                            .tooltip("新建远程文件夹")
                                            .on_click(move |_, window, cx| {
                                                view_mkdir.update(cx, |this, cx| {
                                                    this.prompt_mkdir(window, cx)
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .child(Input::new(&path_input).xsmall().disabled(!connected)),
            )
            .children(self.error_message.as_ref().map(|err| {
                div()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().danger.opacity(0.14))
                    .border_b_1()
                    .border_color(cx.theme().danger.opacity(0.3))
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(err.clone())
            }))
            .child(
                // File table
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .px_2()
                    .py_1p5()
                    .child(if !connected {
                        v_flex()
                            .py_10()
                            .px_4()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(LucideIcon::Server)
                                    .large()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("切换到已连接的 SSH 终端标签，即可自动浏览并传输远程文件。"),
                            )
                            .into_any_element()
                    } else if self.loading && self.entries.is_empty() {
                        div()
                            .py_8()
                            .px_4()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("正在读取远程目录...")
                            .into_any_element()
                    } else if self.entries.is_empty() {
                        div()
                            .py_8()
                            .px_4()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("当前远程目录为空")
                            .into_any_element()
                    } else {
                        let selected = self.selected_path.clone();
                        let panel_ent = cx.entity().clone();
                        v_flex()
                            .gap_0p5()
                            .children(self.entries.iter().cloned().map(|entry| {
                                let row_path = entry.path.clone();
                                let is_sel = selected.as_deref() == Some(row_path.as_str());
                                let is_dir = entry.kind.is_dir();
                                let size_label = entry.formatted_size();
                                let time_label = entry.formatted_mtime();

                                let p_click = panel_ent.clone();
                                let entry_click = entry.clone();

                                let p_ctx_open = panel_ent.clone();
                                let p_ctx_dl = panel_ent.clone();
                                let p_ctx_rename = panel_ent.clone();
                                let p_ctx_del = panel_ent.clone();
                                let e1 = entry.clone();
                                let e2 = entry.clone();
                                let e3 = entry.clone();
                                let e4 = entry.clone();
                                let copy_path = entry.path.clone();

                                h_flex()
                                    .id(SharedString::from(format!("sftp-row-{}", entry.path)))
                                    .px_2()
                                    .py_1p5()
                                    .gap_2()
                                    .items_center()
                                    .justify_between()
                                    .rounded(cx.theme().radius)
                                    .cursor_pointer()
                                    .bg(if is_sel {
                                        cx.theme().list_active
                                    } else {
                                        cx.theme().transparent
                                    })
                                    .hover(|s| s.bg(cx.theme().list_hover))
                                    .on_click(move |ev, window, cx| {
                                        p_click.update(cx, |this, cx| {
                                            this.selected_path = Some(entry_click.path.clone());
                                            if ev.click_count() >= 2 {
                                                if entry_click.kind.is_dir() {
                                                    this.navigate_to(
                                                        entry_click.path.clone(),
                                                        Some(window),
                                                        cx,
                                                    );
                                                } else {
                                                    this.open_remote_file_editor(
                                                        entry_click.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            } else {
                                                cx.notify();
                                            }
                                        });
                                    })
                                    .context_menu(move |menu, _, _| {
                                        let po = p_ctx_open.clone();
                                        let eo = e1.clone();
                                        let pd = p_ctx_dl.clone();
                                        let ed = e2.clone();
                                        let pr = p_ctx_rename.clone();
                                        let er = e3.clone();
                                        let pdel = p_ctx_del.clone();
                                        let edel = e4.clone();
                                        let cp = copy_path.clone();

                                        let menu = if is_dir {
                                            menu.item(
                                                PopupMenuItem::new("进入目录")
                                                    .icon(LucideIcon::FolderOpen)
                                                    .on_click(move |_, window, cx| {
                                                        po.update(cx, |this, cx| {
                                                            this.navigate_to(
                                                                eo.path.clone(),
                                                                Some(window),
                                                                cx,
                                                            )
                                                        });
                                                    }),
                                            )
                                        } else {
                                            menu.item(
                                                PopupMenuItem::new("在线编辑文本")
                                                    .icon(LucideIcon::FileText)
                                                    .on_click(move |_, window, cx| {
                                                        po.update(cx, |this, cx| {
                                                            this.open_remote_file_editor(
                                                                eo.clone(),
                                                                window,
                                                                cx,
                                                            )
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("下载到本地")
                                                    .icon(LucideIcon::Download)
                                                    .on_click(move |_, _, cx| {
                                                        pd.update(cx, |this, cx| {
                                                            this.trigger_download(ed.clone(), cx)
                                                        });
                                                    }),
                                            )
                                        };

                                        menu.item(
                                            PopupMenuItem::new("复制远程路径")
                                                .icon(LucideIcon::Copy)
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(cp.clone()),
                                                    );
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new("重命名")
                                                .icon(LucideIcon::Pencil)
                                                .on_click(move |_, window, cx| {
                                                    pr.update(cx, |this, cx| {
                                                        this.prompt_rename(er.clone(), window, cx)
                                                    });
                                                }),
                                        )
                                        .separator()
                                        .item(PopupMenuItem::new("删除").on_click(
                                            move |_, _, cx| {
                                                pdel.update(cx, |this, cx| {
                                                    this.delete_entry(edel.clone(), cx)
                                                });
                                            },
                                        ))
                                    })
                                    .child(
                                        h_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .gap_2()
                                            .items_center()
                                            .child(if is_dir {
                                                Icon::new(LucideIcon::Folder)
                                                    .small()
                                                    .text_color(cx.theme().primary)
                                            } else {
                                                Icon::new(LucideIcon::File)
                                                    .small()
                                                    .text_color(cx.theme().muted_foreground)
                                            })
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .truncate()
                                                            .text_xs()
                                                            .text_color(cx.theme().foreground)
                                                            .child(entry.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .truncate()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(format!(
                                                                "{size_label} · {time_label}"
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .children((!is_dir).then(|| {
                                        let pdl = panel_ent.clone();
                                        let edl = entry.clone();
                                        Button::new(SharedString::from(format!(
                                            "dl-btn-{}",
                                            entry.path
                                        )))
                                        .ghost()
                                        .xsmall()
                                        .compact()
                                        .icon(LucideIcon::Download)
                                        .tooltip("下载文件")
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            pdl.update(cx, |this, cx| {
                                                this.trigger_download(edl.clone(), cx)
                                            });
                                        })
                                    }))
                            }))
                            .into_any_element()
                    }),
            )
            .children((!self.transfers.is_empty()).then(|| {
                // Lightweight embedded bottom transfer status strip
                v_flex()
                    .max_h(px(148.0))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.35))
                    .px_3()
                    .py_2()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(format!("传输记录 ({})", self.transfers.len())),
                            )
                            .child(
                                Button::new("sftp-clear-transfers")
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .label("清空")
                                    .on_click(move |_, _, cx| {
                                        view_clear_tx.update(cx, |this, cx| {
                                            this.transfers.retain(|t| {
                                                matches!(t.status, TransferStatus::Running)
                                            });
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .overflow_y_scrollbar()
                            .child(v_flex().gap_1().children(self.transfers.iter().take(6).map(
                                |tr| {
                                    let status_text = match &tr.status {
                                        TransferStatus::Running => "传输中...".to_string(),
                                        TransferStatus::Completed => {
                                            format!("已完成 · {}", format_bytes(tr.bytes_done))
                                        }
                                        TransferStatus::Failed(e) => format!("失败: {e}"),
                                    };
                                    let dir_icon = match tr.direction {
                                        TransferDirection::Upload => LucideIcon::Upload,
                                        TransferDirection::Download => LucideIcon::Download,
                                    };
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
                                                    Icon::new(dir_icon)
                                                        .xsmall()
                                                        .text_color(cx.theme().primary),
                                                )
                                                .child(
                                                    div()
                                                        .truncate()
                                                        .text_xs()
                                                        .text_color(cx.theme().foreground)
                                                        .child(tr.file_name.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(status_text),
                                        )
                                },
                            ))),
                    )
            }))
    }
}
