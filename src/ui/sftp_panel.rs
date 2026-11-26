//! Context-aware Right SFTP Inspector Panel with directory follow, symlink directory traversal,
//! confirmation dialogs, streaming chunked transfers with live progress bars, and macOS Finder styling.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    notification::Notification,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, relative,
};
use tokio::sync::mpsc;

use crate::sftp::{
    RemoteEntry, RemoteEntryKind, SftpService, TransferDirection, TransferRecord, TransferStatus,
    format_bytes, join_remote_path, parent_remote_path,
};
use crate::ui::modals::{open_confirm_dialog, open_remote_editor_dialog, open_simple_name_dialog};
use crate::ui::theme::macos_squircle_badge;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SftpBindingKey {
    pub session_id: String,
    pub conn_epoch: u64,
}

#[derive(Clone)]
pub struct SftpTarget {
    pub binding: SftpBindingKey,
    pub dir: String,
    pub session_label: String,
}

pub enum SftpPanelEvent {
    Navigated {
        binding: SftpBindingKey,
        path: String,
    },
    FollowChanged(bool),
    CloseRequested,
}

pub struct SftpPanelView {
    target: Option<SftpTarget>,
    service: Option<SftpService>,
    nav_seq: u64,
    pub follow_cwd: bool,
    pub panel_visible: bool,
    pub loading: bool,
    pub error_message: Option<String>,
    pub entries: Vec<RemoteEntry>,
    pub selected_path: Option<String>,
    pub transfers: Vec<TransferRecord>,
    path_input: Option<Entity<InputState>>,
    editor_font_family: String,
    editor_font_size: f32,
    downloads_dir: PathBuf,
    tokio_handle: tokio::runtime::Handle,
}

impl EventEmitter<SftpPanelEvent> for SftpPanelView {}

impl SftpPanelView {
    pub fn new(
        follow_cwd: bool,
        panel_visible: bool,
        editor_font_family: String,
        editor_font_size: f32,
        downloads_dir: PathBuf,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            target: None,
            nav_seq: 0,
            service: None,
            follow_cwd,
            panel_visible,
            loading: false,
            error_message: None,
            entries: Vec::new(),
            selected_path: None,
            transfers: Vec::new(),
            path_input: None,
            editor_font_family,
            editor_font_size,
            downloads_dir,
            tokio_handle,
        }
    }

    pub fn set_editor_typography(&mut self, family: String, size: f32) {
        self.editor_font_family = family;
        self.editor_font_size = size;
    }

    pub fn active_transfer_count(&self) -> usize {
        self.transfers
            .iter()
            .filter(|t| matches!(t.status, TransferStatus::Running))
            .count()
    }

    pub fn set_panel_visible(
        &mut self,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_visible = visible;
        if visible && self.service.is_some() {
            self.load_directory(Some(window), cx);
        } else {
            cx.notify();
        }
    }

    pub fn bind_session(
        &mut self,
        binding: Option<(SftpService, SftpTarget)>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let (Some(cur), Some((_, next))) = (&mut self.target, &binding)
            && cur.binding == next.binding
        {
            if cur.session_label != next.session_label {
                cur.session_label = next.session_label.clone();
                cx.notify();
            }
            return;
        }
        (self.service, self.target) = binding.unzip();
        self.load_directory(window, cx);
    }

    pub fn current_path(&self) -> &str {
        self.target.as_ref().map_or("", |t| &t.dir)
    }

    fn operation_target(&self) -> Option<(SftpService, SftpTarget)> {
        self.service.clone().zip(self.target.clone())
    }

    fn shows_target(&self, target: &SftpTarget) -> bool {
        self.target
            .as_ref()
            .is_some_and(|t| t.binding == target.binding && t.dir == target.dir)
    }

    fn begin_listing(&mut self) {
        self.nav_seq += 1;
        self.entries.clear();
        self.selected_path = None;
        self.error_message = None;
        self.loading = self.panel_visible && self.target.is_some();
    }

    pub fn navigate_to(
        &mut self,
        path: String,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(target) = &mut self.target {
            target.dir = path.clone();
            let binding = target.binding.clone();
            cx.emit(SftpPanelEvent::Navigated { binding, path });
            self.load_directory(window, cx);
        }
    }

    fn load_directory(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.begin_listing();
        if let Some(window) = window {
            if let Some(input) = &self.path_input {
                input.update(cx, |input, cx| {
                    input.set_value(self.current_path().to_owned(), window, cx)
                });
            }
        } else {
            self.path_input = None;
        }
        if self.panel_visible
            && let Some((service, target)) = self.operation_target()
        {
            let seq = self.nav_seq;
            let listing = self
                .tokio_handle
                .spawn(async move { service.list_dir(&target.dir).await });
            cx.spawn(async move |weak, cx| {
                let result = listing.await.map_err(anyhow::Error::from).and_then(|r| r);
                let _ = weak.update(cx, |this, cx| {
                    if let Some(event) = this.finish_listing(target.binding, seq, result) {
                        cx.emit(event);
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn finish_listing(
        &mut self,
        binding: SftpBindingKey,
        seq: u64,
        result: anyhow::Result<Vec<RemoteEntry>>,
    ) -> Option<SftpPanelEvent> {
        if self.target.as_ref().map(|t| &t.binding) != Some(&binding) || self.nav_seq != seq {
            return None;
        }
        self.loading = false;
        match result {
            Ok(entries) => {
                self.entries = entries;
                Some(SftpPanelEvent::Navigated {
                    binding,
                    path: self.current_path().to_owned(),
                })
            }
            Err(err) => {
                self.error_message = Some(format!("{err:#}"));
                None
            }
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_directory(None, cx);
    }

    pub fn go_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = parent_remote_path(self.current_path()) {
            self.navigate_to(parent, Some(window), cx);
        }
    }

    fn ensure_path_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(existing) = &self.path_input {
            return existing.clone();
        }
        let initial = self.current_path().to_owned();
        let input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).placeholder("/远程路径");
            st.set_value(initial, window, cx);
            st
        });
        cx.subscribe_in(&input, window, |this, state, ev, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                let value = state.read(cx).value();
                let target = value.trim();
                this.navigate_to(
                    if target.is_empty() {
                        "/".into()
                    } else {
                        target.into()
                    },
                    Some(window),
                    cx,
                );
            }
        })
        .detach();
        self.path_input = Some(input.clone());
        input
    }

    fn trigger_upload(&mut self, cx: &mut Context<Self>) {
        let Some((service, target)) = self.operation_target() else {
            return;
        };
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
                let remote_path = join_remote_path(&target.dir, &file_name);
                let tid = uuid::Uuid::new_v4().to_string();

                let _ = weak.update(cx, |this, cx| {
                    this.transfers.insert(
                        0,
                        TransferRecord {
                            id: tid.clone(),
                            session_label: target.session_label.clone(),
                            direction: TransferDirection::Upload,
                            file_name: file_name.clone(),
                            bytes_done: 0,
                            bytes_total: None,
                            status: TransferStatus::Running,
                        },
                    );
                    cx.notify();
                });

                let (prog_tx, mut prog_rx) = mpsc::unbounded_channel::<(u64, Option<u64>)>();
                let tid_prog = tid.clone();
                let weak_prog = weak.clone();
                let prog_task = cx.spawn(async move |cx| {
                    while let Some((done, total)) = prog_rx.recv().await {
                        let ok = weak_prog.update(cx, |this, cx| {
                            if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid_prog)
                            {
                                rec.bytes_done = done;
                                rec.bytes_total = total;
                                cx.notify();
                            }
                        });
                        if ok.is_err() {
                            break;
                        }
                    }
                });

                let svc = service.clone();
                let res = rt
                    .spawn(async move {
                        let mut last_emit = Instant::now() - Duration::from_secs(1);
                        svc.upload_local_file_with_progress(
                            &local_path,
                            &remote_path,
                            |done, total| {
                                let now = Instant::now();
                                if done == 0
                                    || done >= total
                                    || now.duration_since(last_emit) >= Duration::from_millis(45)
                                {
                                    last_emit = now;
                                    let _ = prog_tx.send((done, Some(total)));
                                }
                            },
                        )
                        .await
                    })
                    .await;
                let _ = prog_task.await;

                let _ = weak.update(cx, |this, cx| {
                    let succeeded = matches!(res, Ok(Ok(_)));
                    if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid) {
                        match res {
                            Ok(Ok(bytes)) => {
                                rec.bytes_done = bytes;
                                rec.bytes_total = Some(bytes);
                                rec.status = TransferStatus::Completed;
                            }
                            Ok(Err(err)) => {
                                rec.status = TransferStatus::Failed(format!("{err:#}"));
                            }
                            Err(err) => {
                                rec.status = TransferStatus::Failed(format!("{err:#}"));
                            }
                        }
                    }
                    if succeeded && this.shows_target(&target) {
                        this.refresh(cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn trigger_download(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let Some((service, target)) = self.operation_target() else {
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
                        session_label: target.session_label.clone(),
                        direction: TransferDirection::Download,
                        file_name: fname,
                        bytes_done: 0,
                        bytes_total: expected,
                        status: TransferStatus::Running,
                    },
                );
                cx.notify();
            });

            let (prog_tx, mut prog_rx) = mpsc::unbounded_channel::<(u64, Option<u64>)>();
            let tid_prog = tid.clone();
            let weak_prog = weak.clone();
            let prog_task = cx.spawn(async move |cx| {
                while let Some((done, total)) = prog_rx.recv().await {
                    let ok = weak_prog.update(cx, |this, cx| {
                        if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid_prog) {
                            rec.bytes_done = done;
                            if total.is_some() {
                                rec.bytes_total = total;
                            }
                            cx.notify();
                        }
                    });
                    if ok.is_err() {
                        break;
                    }
                }
            });

            let remote_path = entry.path.clone();
            let res = rt
                .spawn(async move {
                    let mut last_emit = Instant::now() - Duration::from_secs(1);
                    service
                        .download_to_local_with_progress(&remote_path, &local_path, |done, total| {
                            let now = Instant::now();
                            let finished = total.is_some_and(|t| done >= t);
                            if done == 0
                                || finished
                                || now.duration_since(last_emit) >= Duration::from_millis(45)
                            {
                                last_emit = now;
                                let _ = prog_tx.send((done, total));
                            }
                        })
                        .await
                })
                .await;
            let _ = prog_task.await;

            let _ = weak.update(cx, |this, cx| {
                if let Some(rec) = this.transfers.iter_mut().find(|r| r.id == tid) {
                    match res {
                        Ok(Ok(bytes)) => {
                            rec.bytes_done = bytes;
                            rec.bytes_total = Some(bytes);
                            rec.status = TransferStatus::Completed;
                        }
                        Ok(Err(err)) => {
                            rec.status = TransferStatus::Failed(format!("{err:#}"));
                        }
                        Err(err) => {
                            rec.status = TransferStatus::Failed(format!("{err:#}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_file_operation(
        &mut self,
        target: SftpTarget,
        op: impl Future<Output = anyhow::Result<()>> + Send + 'static,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let task = self.tokio_handle.spawn(op);
        cx.spawn_in(window, async move |weak, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            let _ = weak.update_in(cx, |this, window, cx| {
                this.finish_file_operation(&target, result, window, cx);
            });
        })
        .detach();
    }

    fn finish_file_operation(
        &mut self,
        target: &SftpTarget,
        result: anyhow::Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) if self.shows_target(target) => self.load_directory(Some(window), cx),
            Ok(()) => {}
            Err(err) if self.panel_visible && self.shows_target(target) => {
                self.error_message = Some(format!("{err:#}"));
                cx.notify();
            }
            Err(err) => window.push_notification(
                Notification::error(format!(
                    "{} · {}：{err:#}",
                    target.session_label, target.dir
                )),
                cx,
            ),
        }
    }

    fn prompt_mkdir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((service, target)) = self.operation_target() else {
            return;
        };
        let panel = cx.entity();
        open_simple_name_dialog(
            "新建远程文件夹",
            "文件夹名称",
            "",
            window,
            cx,
            move |name, window, cx| {
                let path = join_remote_path(&target.dir, &name);
                let service = service.clone();
                panel.update(cx, |this, cx| {
                    this.run_file_operation(
                        target.clone(),
                        async move { service.mkdir(&path).await },
                        window,
                        cx,
                    )
                });
            },
        );
    }

    fn prompt_rename(&mut self, entry: RemoteEntry, window: &mut Window, cx: &mut Context<Self>) {
        let Some((service, target)) = self.operation_target() else {
            return;
        };
        let panel = cx.entity();
        open_simple_name_dialog(
            "重命名远程条目",
            "新名称",
            &entry.name,
            window,
            cx,
            move |name, window, cx| {
                let from = entry.path.clone();
                let to = join_remote_path(&target.dir, &name);
                let service = service.clone();
                panel.update(cx, |this, cx| {
                    this.run_file_operation(
                        target.clone(),
                        async move { service.rename(&from, &to).await },
                        window,
                        cx,
                    )
                });
            },
        );
    }

    fn confirm_delete_entry(
        &mut self,
        entry: RemoteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((service, target)) = self.operation_target() else {
            return;
        };
        let panel = cx.entity();
        let kind_label = if entry.kind.is_dir() {
            "远程目录"
        } else if entry.kind == RemoteEntryKind::Symlink {
            "软链接"
        } else {
            "远程文件"
        };
        let msg = format!(
            "确定要永久删除 {} 上的{kind_label} `{}` 吗？此操作无法撤销。",
            target.session_label, entry.path
        );
        open_confirm_dialog(
            format!("删除{kind_label}"),
            msg,
            "确认删除",
            window,
            cx,
            move |window, cx| {
                let service = service.clone();
                let path = entry.path.clone();
                let is_dir = entry.kind.is_dir();
                panel.update(cx, |this, cx| {
                    this.run_file_operation(
                        target.clone(),
                        async move { service.remove(&path, is_dir).await },
                        window,
                        cx,
                    )
                });
            },
        );
    }

    fn open_remote_file_editor(
        &mut self,
        entry: RemoteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((service, target)) = self.operation_target() else {
            return;
        };
        let read_path = entry.path.clone();
        let rt = self.tokio_handle.clone();
        let read_service = service.clone();
        let read = rt.spawn(async move { read_service.read_text(&read_path).await });
        let family = self.editor_font_family.clone();
        let size = self.editor_font_size;
        cx.spawn_in(window, async move |weak, cx| {
            let result = read.await.map_err(anyhow::Error::from).and_then(|r| r);
            let _ = weak.update_in(cx, |this, window, cx| {
                let content = match result {
                    Ok(content) if this.shows_target(&target) => content,
                    Ok(_) => return,
                    Err(err) => return this.finish_file_operation(&target, Err(err), window, cx),
                };
                let panel = cx.weak_entity();
                open_remote_editor_dialog(
                    &entry.name,
                    &format!("{} · {}", target.session_label, entry.path),
                    &content,
                    &family,
                    size,
                    window,
                    cx,
                    move |text, cx| {
                        let target = target.clone();
                        let service = service.clone();
                        let path = entry.path.clone();
                        let write = rt.spawn(async move { service.write_text(&path, text).await });
                        let panel = panel.clone();
                        cx.spawn(async move |cx| {
                            write.await??;
                            let _ = panel.update(cx, |this, cx| {
                                if this.shows_target(&target) {
                                    this.refresh(cx);
                                }
                            });
                            Ok(())
                        })
                    },
                );
            });
        })
        .detach();
    }
}

#[inline(never)]
fn render_sftp_entry_row(
    panel_ent: Entity<SftpPanelView>,
    entry: RemoteEntry,
    nav_seq: u64,
    is_sel: bool,
    cx: &App,
) -> AnyElement {
    let nav_dir = entry.is_navigable_dir();
    let is_symlink = entry.kind == RemoteEntryKind::Symlink;
    let size_label = entry.formatted_size();
    let time_label = entry.formatted_mtime();
    let display_name = if is_symlink {
        format!("{} ↗", entry.name)
    } else {
        entry.name.clone()
    };

    let folder_tint = cx.theme().cyan;

    // A context menu can outlive the directory rows that opened it.
    let with_row = {
        let entry = entry.clone();
        move |window: &mut Window,
              cx: &mut App,
              action: fn(
            &mut SftpPanelView,
            RemoteEntry,
            &mut Window,
            &mut Context<SftpPanelView>,
        )| {
            panel_ent.update(cx, |panel, cx| {
                if panel.nav_seq == nav_seq {
                    action(panel, entry.clone(), window, cx);
                }
            });
        }
    };
    let enter =
        |p: &mut SftpPanelView, e: RemoteEntry, w: &mut Window, cx: &mut Context<SftpPanelView>| {
            p.navigate_to(e.path, Some(w), cx)
        };
    let download = |p: &mut SftpPanelView, e, _: &mut Window, cx: &mut Context<SftpPanelView>| {
        p.trigger_download(e, cx)
    };
    let click_row = with_row.clone();
    let menu_row = with_row.clone();
    let copy_path = entry.path.clone();

    h_flex()
        .id(SharedString::from(format!("sftp-row-{}", entry.path)))
        .px_2()
        .py_1p5()
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(px(7.0))
        .cursor_pointer()
        .bg(if is_sel {
            cx.theme().list_active
        } else {
            cx.theme().transparent
        })
        .hover(|s| {
            if is_sel {
                s
            } else {
                s.bg(cx.theme().list_hover)
            }
        })
        .on_click(move |ev, window, cx| {
            if ev.click_count() >= 2 {
                click_row(
                    window,
                    cx,
                    if nav_dir {
                        enter
                    } else {
                        SftpPanelView::open_remote_file_editor
                    },
                );
            } else {
                click_row(window, cx, |panel, entry, _, cx| {
                    panel.selected_path = Some(entry.path);
                    cx.notify();
                });
            }
        })
        .context_menu(move |menu, _, _| {
            let open = menu_row.clone();
            let dl = menu_row.clone();
            let rename = menu_row.clone();
            let delete = menu_row.clone();
            let path = copy_path.clone();
            let menu = if nav_dir {
                menu.item(
                    PopupMenuItem::new("进入目录")
                        .icon(LucideIcon::FolderOpen)
                        .on_click(move |_, window, cx| open(window, cx, enter)),
                )
            } else {
                menu.item(
                    PopupMenuItem::new("在线编辑文本")
                        .icon(LucideIcon::FileText)
                        .on_click(move |_, window, cx| {
                            open(window, cx, SftpPanelView::open_remote_file_editor)
                        }),
                )
                .item(
                    PopupMenuItem::new("下载到本地")
                        .icon(LucideIcon::Download)
                        .on_click(move |_, window, cx| dl(window, cx, download)),
                )
            };
            menu.item(
                PopupMenuItem::new("复制远程路径")
                    .icon(LucideIcon::Copy)
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                    }),
            )
            .item(
                PopupMenuItem::new("重命名")
                    .icon(LucideIcon::Pencil)
                    .on_click(move |_, window, cx| {
                        rename(window, cx, SftpPanelView::prompt_rename)
                    }),
            )
            .separator()
            .item(PopupMenuItem::new("删除").on_click(move |_, window, cx| {
                delete(window, cx, SftpPanelView::confirm_delete_entry)
            }))
        })
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .gap_2()
                .items_center()
                .child(if nav_dir {
                    Icon::new(LucideIcon::Folder)
                        .small()
                        .text_color(folder_tint)
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
                                .font_weight(if nav_dir {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(cx.theme().foreground)
                                .child(display_name),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{size_label} · {time_label}")),
                        ),
                ),
        )
        .children((!nav_dir).then(|| {
            Button::new(SharedString::from(format!("dl-btn-{}", entry.path)))
                .tab_stop(false)
                .ghost()
                .xsmall()
                .compact()
                .icon(LucideIcon::Download)
                .tooltip("下载文件")
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    with_row(window, cx, download);
                })
        }))
        .into_any_element()
}

impl Render for SftpPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.service.is_some();
        let can_up = connected && parent_remote_path(self.current_path()).is_some();
        let path_input = self.ensure_path_input(window, cx);
        let follow = self.follow_cwd;
        let session_badge = self
            .target
            .as_ref()
            .map_or("未绑定 SSH 会话", |t| &t.session_label);

        let view_up = cx.entity().clone();
        let view_refresh = cx.entity().clone();
        let view_upload = cx.entity().clone();
        let view_mkdir = cx.entity().clone();
        let view_clear_tx = cx.entity().clone();

        let list_body = if !connected {
            v_flex()
                .py_12()
                .px_4()
                .gap_3()
                .items_center()
                .child(macos_squircle_badge(
                    LucideIcon::FolderOpen,
                    cx.theme().muted_foreground,
                    38.0,
                    9.5,
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("切换到已连接的 SSH 标签即可自动浏览并传输远程文件"),
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
        } else if self.error_message.is_some() && self.entries.is_empty() {
            div().into_any_element()
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
            let rows: Vec<AnyElement> = self
                .entries
                .iter()
                .cloned()
                .map(|entry| {
                    let is_sel = selected.as_deref() == Some(entry.path.as_str());
                    render_sftp_entry_row(panel_ent.clone(), entry, self.nav_seq, is_sel, cx)
                })
                .collect();
            v_flex().gap_0p5().children(rows).into_any_element()
        };

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(
                v_flex()
                    .px_3()
                    .pt_2p5()
                    .pb_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .w(px(7.0))
                                            .h(px(7.0))
                                            .rounded_full()
                                            .bg(if connected {
                                                cx.theme().success
                                            } else {
                                                cx.theme().muted_foreground.opacity(0.45)
                                            }),
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
                                    .gap_0p5()
                                    .items_center()
                                    .child(
                                        Button::new("sftp-follow-toggle")
                                            .tab_stop(false)
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .selected(follow)
                                            .icon(LucideIcon::Link)
                                            .tooltip("跟随终端当前目录")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                cx.emit(SftpPanelEvent::FollowChanged(
                                                    !this.follow_cwd,
                                                ));
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sftp-up")
                                            .tab_stop(false)
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .disabled(!can_up)
                                            .icon(LucideIcon::ArrowUp)
                                            .tooltip("上级目录")
                                            .on_click(move |_, window, cx| {
                                                view_up
                                                    .update(cx, |this, cx| this.go_up(window, cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("sftp-refresh")
                                            .tab_stop(false)
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
                                            .tab_stop(false)
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
                                            .tab_stop(false)
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
                                    )
                                    .child(
                                        Button::new("sftp-close-panel-btn")
                                            .tab_stop(false)
                                            .ghost()
                                            .xsmall()
                                            .compact()
                                            .icon(LucideIcon::PanelRightClose)
                                            .tooltip("隐藏 SFTP 面板")
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                cx.emit(SftpPanelEvent::CloseRequested);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        Input::new(&path_input)
                            .xsmall()
                            .prefix(
                                Icon::new(LucideIcon::Folder)
                                    .xsmall()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .disabled(!connected),
                    ),
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
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .px_2()
                    .py_1p5()
                    .child(list_body),
            )
            .children((!self.transfers.is_empty()).then(|| {
                v_flex()
                    .max_h(px(172.0))
                    .border_t_1()
                    .border_color(cx.theme().sidebar_border)
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
                                    .child(format!("传输队列 ({})", self.transfers.len())),
                            )
                            .child(
                                Button::new("sftp-clear-transfers")
                                    .tab_stop(false)
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
                        div().w_full().min_w_0().overflow_y_scrollbar().child(
                            v_flex()
                                .gap_1p5()
                                .children(self.transfers.iter().take(6).map(|tr| {
                                    let ratio = tr.progress_ratio();
                                    let is_running = matches!(tr.status, TransferStatus::Running);
                                    let status_text = match &tr.status {
                                        TransferStatus::Running => {
                                            if let Some(total) = tr.bytes_total {
                                                let pct =
                                                    (ratio.unwrap_or(0.0) * 100.0).round() as u32;
                                                format!(
                                                    "{pct}% · {} / {}",
                                                    format_bytes(tr.bytes_done),
                                                    format_bytes(total)
                                                )
                                            } else {
                                                format!("传输中 · {}", format_bytes(tr.bytes_done))
                                            }
                                        }
                                        TransferStatus::Completed => {
                                            format!("已完成 · {}", format_bytes(tr.bytes_done))
                                        }
                                        TransferStatus::Failed(e) => format!("失败: {e}"),
                                    };
                                    let dir_icon = match tr.direction {
                                        TransferDirection::Upload => LucideIcon::Upload,
                                        TransferDirection::Download => LucideIcon::Download,
                                    };
                                    v_flex()
                                        .gap_1()
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
                                                            Icon::new(dir_icon)
                                                                .xsmall()
                                                                .text_color(cx.theme().primary),
                                                        )
                                                        .child(
                                                            div()
                                                                .truncate()
                                                                .text_xs()
                                                                .text_color(cx.theme().foreground)
                                                                .child(format!(
                                                                    "{} · {}",
                                                                    tr.session_label, tr.file_name
                                                                )),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .max_w(px(180.0))
                                                        .truncate()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(status_text),
                                                ),
                                        )
                                        .children(is_running.then(|| {
                                            let fill_pct = ratio.unwrap_or(0.15).clamp(0.03, 1.0);
                                            div()
                                                .w_full()
                                                .h(px(3.0))
                                                .rounded_full()
                                                .bg(cx.theme().border)
                                                .child(
                                                    div()
                                                        .w(relative(fill_pct))
                                                        .h_full()
                                                        .rounded_full()
                                                        .bg(cx.theme().primary),
                                                )
                                        }))
                                })),
                        ),
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> SftpPanelView {
        SftpPanelView::new(
            true,
            true,
            String::new(),
            14.,
            PathBuf::new(),
            tokio::runtime::Handle::current(),
        )
    }

    fn target(id: &str, epoch: u64, path: &str) -> SftpTarget {
        SftpTarget {
            binding: SftpBindingKey {
                session_id: id.into(),
                conn_epoch: epoch,
            },
            dir: path.into(),
            session_label: "同名 SSH".into(),
        }
    }

    fn file(name: &str) -> RemoteEntry {
        RemoteEntry {
            name: name.into(),
            path: format!("/{name}"),
            kind: RemoteEntryKind::File,
            points_to_dir: false,
            size_bytes: None,
            modified_unix: None,
            permissions: String::new(),
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn detached_path_inputs_and_file_errors_follow_the_originating_directory() {
        gpui_kit::platform::headless()
            .with_assets(gpui_kit::assets::AllAssets)
            .run(|cx| {
                gpui_kit::init(cx);
                let (window, view) =
                    gpui_kit::open_window(Default::default(), cx, |_, cx| cx.new(|_| panel()))
                        .unwrap();
                window
                    .update(cx, |_, window, cx| {
                        view.update(cx, |p, cx| {
                            let a = target("A", 1, "/documents");
                            p.target = Some(a.clone());
                            p.load_directory(Some(window), cx);
                            let seq = p.nav_seq;
                            let input = p.ensure_path_input(window, cx);
                            assert_eq!(input.read(cx).value().as_str(), "/documents");
                            p.finish_file_operation(
                                &a,
                                Err(anyhow::anyhow!("mkdir: permission denied")),
                                window,
                                cx,
                            );
                            assert_eq!(p.nav_seq, seq);
                            p.finish_listing(a.binding.clone(), seq, Ok(vec![]));
                            assert_eq!(
                                p.error_message.as_deref(),
                                Some("mkdir: permission denied")
                            );

                            p.navigate_to("/elsewhere".into(), None, cx);
                            assert!(p.path_input.is_none());
                            let new_input = p.ensure_path_input(window, cx);
                            assert_ne!(new_input, input);
                            assert_eq!(new_input.read(cx).value().as_str(), "/elsewhere");
                            let seq = p.nav_seq;
                            p.finish_file_operation(&a, Ok(()), window, cx);
                            assert_eq!(p.nav_seq, seq);
                            p.bind_session(None, Some(window), cx);
                            assert!(new_input.read(cx).value().is_empty());
                            assert!(p.current_path().is_empty() && !p.loading);
                        })
                    })
                    .unwrap();
                cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
            });
    }

    #[tokio::test]
    async fn slow_listings_cannot_replace_another_tab_directory_or_connection() {
        let mut p = panel();
        let a = target("A", 1, "/home");
        p.target = Some(a.clone());
        p.begin_listing();
        let slow = p.nav_seq;
        p.entries.push(file("old"));
        p.selected_path = Some("/old".into());
        let b = target("B", 1, "/home");
        p.target = Some(b.clone());
        p.begin_listing();
        assert!(p.entries.is_empty() && p.selected_path.is_none() && p.loading);
        assert!(!p.shows_target(&a));
        let ready = p.finish_listing(b.binding.clone(), p.nav_seq, Ok(vec![file("B-only")]));
        assert!(
            matches!(ready, Some(SftpPanelEvent::Navigated { binding, path }) if binding == b.binding && path == "/home")
        );
        assert!(
            p.finish_listing(a.binding.clone(), slow, Ok(vec![file("late-A")]))
                .is_none()
        );
        assert_eq!(p.entries, [file("B-only")]);

        let denied = target("A", 2, "/private");
        p.target = Some(denied.clone());
        p.begin_listing();
        let pending = p.nav_seq;
        assert!(p.finish_listing(a.binding, pending, Ok(vec![])).is_none());
        assert!(p.loading);
        assert!(
            p.finish_listing(
                denied.binding.clone(),
                pending,
                Err(anyhow::anyhow!("permission denied"))
            )
            .is_none()
        );
        assert_eq!(p.error_message.as_deref(), Some("permission denied"));
        assert!(
            p.finish_listing(denied.binding.clone(), slow, Ok(vec![file("late")]))
                .is_none()
        );
        assert_eq!(p.error_message.as_deref(), Some("permission denied"));
        assert!(!p.loading && p.entries.is_empty());

        p.target = None;
        p.begin_listing();
        assert_eq!(p.current_path(), "");
        assert!(
            p.finish_listing(denied.binding, pending, Ok(vec![file("late")]))
                .is_none()
        );
        assert!(!p.loading && p.entries.is_empty() && p.error_message.is_none());
    }
}
