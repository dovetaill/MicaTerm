//! Clean GPUI Kit Dialog modals for SSH Hosts, Folders, Keychain, Snippets, SFTP Editor,
//! Destructive Confirmations, and Settings.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
    notification::Notification,
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState, Textarea, TextareaState},
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, Focusable as _,
    FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, TextAlign, TextRun, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};

use crate::domain::{
    BUNDLED_SARASA_FONT_FAMILY, HostNode, HostNodeKind, KeychainEntry,
    KeychainKind, RightClickAction, SecretPayload, ShortcutAction, ShortcutCategory, ShortcutChord,
    ShortcutConfig, SnippetItem, SshAuthConfig, SshHostConfig, SshProxyConfig, format_chord,
    system_monospace_font_family,
};
use crate::logging::{open_directory_in_file_manager, read_recent_logs};
use crate::storage::AppPaths;
use crate::syntax::SyntaxRegistry;
use crate::terminal::{TerminalPalette, colors::rgb_to_hsla};
use crate::ui::fonts::{TERMINAL_FONT_PRESETS, TerminalFont};
use crate::ui::theme::{macos_hex, macos_inset_group, macos_segmented_track};
use crate::ui::theme_catalog::{ThemeCategory, ThemeRegistry, ThemeSpec};
use crate::ui::workspace::MicaWorkspace;

fn field_block(label: &'static str, control: impl IntoElement, cx: &App) -> gpui_kit::Div {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(control)
}

/// Reserves vertical room inside `Dialog`'s `overflow_hidden` / `overflow_y_scrollbar` body
/// (`pl/pr = 16px`, `pt/pb = 0px`) so `gpui-component`'s 3px outer focus ring is not clipped.
fn dialog_body() -> gpui_kit::Div {
    v_flex().w_full().min_w_0().py_1()
}

pub fn open_confirm_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    confirm_label: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let message = message.into();
    let confirm_label = confirm_label.into();
    let confirm_cb = Rc::new(on_confirm);

    window.open_dialog(cx, move |dialog, _, cx| {
        let cb = confirm_cb.clone();
        dialog
            .title(title.clone())
            .w(px(400.0))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(message.clone()),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("confirm-dialog-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("confirm-dialog-ok")
                            .danger()
                            .small()
                            .label(confirm_label.clone())
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                cb(window, cx);
                            }),
                    ),
            )
    });
}

pub fn open_multiline_paste_confirm_dialog(
    text: String,
    window: &mut Window,
    cx: &mut App,
    on_confirm: impl Fn(&str, &mut App) + 'static,
) {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let line_vec: Vec<&str> = normalized.lines().collect();
    let line_count = line_vec.len().max(1);
    let title = if line_count == 1 {
        "粘贴含换行的命令（将立即执行）？".to_string()
    } else {
        format!("粘贴 {line_count} 行文本？")
    };
    let preview = line_vec.into_iter().take(6).collect::<Vec<_>>().join("\n");
    let confirm = Rc::new(move |cx: &mut App| on_confirm(&text, cx));
    window.open_alert_dialog(cx, move |dialog, _, cx| {
        let confirm = confirm.clone();
        dialog
            .confirm()
            .title(title.clone())
            .ok_text("确认粘贴")
            .cancel_text("取消")
            .child(
                div()
                    .text_sm()
                    .child("当前终端未开启括号粘贴保护，换行可能立即执行命令。"),
            )
            .child(
                div()
                    .font_family(BUNDLED_SARASA_FONT_FAMILY)
                    .text_xs()
                    .p_2()
                    .overflow_hidden()
                    .max_h(px(144.))
                    .bg(cx.theme().muted)
                    .child(preview.clone()),
            )
            .on_ok(move |_, _, cx| {
                confirm(cx);
                true
            })
    });
}

pub fn open_simple_name_dialog(
    title: &'static str,
    label: &'static str,
    initial_value: &str,
    window: &mut Window,
    cx: &mut App,
    on_submit: impl Fn(String, &mut Window, &mut App) + 'static,
) {
    let initial = initial_value.to_string();
    let input = cx.new(|cx| {
        let mut st = InputState::new(window, cx).placeholder("请输入名称");
        if !initial.is_empty() {
            st.set_value(initial, window, cx);
        }
        st
    });
    let first_focus = Cell::new(true);
    let submit_cb = Rc::new(on_submit);

    window.open_dialog(cx, move |dialog, window, cx| {
        if first_focus.replace(false) {
            input.focus_handle(cx).focus(window, cx);
        }
        let inp_ok = input.clone();
        let cb = submit_cb.clone();

        dialog
            .title(title)
            .w(px(400.0))
            .child(dialog_body().gap_3().child(field_block(label, Input::new(&input).small(), cx)))
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("simple-dialog-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("simple-dialog-ok")
                            .primary()
                            .small()
                            .label("确定")
                            .on_click(move |_, window, cx| {
                                let val = inp_ok.read(cx).value().trim().to_string();
                                if !val.is_empty() {
                                    cb(val, window, cx);
                                    window.close_dialog(cx);
                                }
                            }),
                    ),
            )
    });
}

const COMMON_EDITOR_LANGUAGES: &[(&str, &str)] = &[
    ("Bourne Again Shell (bash)", "Shell / Bash"),
    ("JSON", "JSON"),
    ("YAML", "YAML"),
    ("TOML", "TOML"),
    ("INI", "INI / Conf"),
    ("nginx", "Nginx"),
    ("Dockerfile", "Dockerfile"),
    ("Makefile", "Makefile"),
    ("Python", "Python"),
    ("Rust", "Rust"),
    ("Go", "Go"),
    ("JavaScript", "JavaScript"),
    ("TypeScript", "TypeScript"),
    ("SQL", "SQL"),
    ("HTML", "HTML"),
    ("CSS", "CSS"),
    ("XML", "XML"),
    ("Markdown", "Markdown"),
    ("Diff", "Diff / Patch"),
    ("C", "C"),
    ("C++", "C++"),
];

pub fn open_remote_editor_dialog(
    file_name: &str,
    remote_path: &str,
    initial_content: &str,
    font_family: &str,
    font_size: f32,
    window: &mut Window,
    cx: &mut App,
    on_save: impl Fn(String, &mut App) -> gpui_kit::Task<anyhow::Result<()>> + 'static,
) {
    let pack = SyntaxRegistry::global().current();
    let first_line = initial_content.lines().next();
    let detected_lang = pack
        .find_syntax(Some(file_name), first_line)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Plain Text".to_string());

    let text_init = initial_content.to_string();
    let init_lang_for_editor = detected_lang.clone();
    let editor = cx.new(|cx| {
        let mut st = EditorState::new(window, cx)
            .language(init_lang_for_editor)
            .default_value(text_init);
        st.set_highlighter_factory(SyntaxRegistry::editor_highlighter_factory(), cx);
        st
    });

    let active_lang = Rc::new(RefCell::new(detected_lang.clone()));
    let custom_langs: Vec<String> = pack
        .custom_entries
        .iter()
        .filter_map(|e| e.syntax_name.clone())
        .collect();
    let editor_font = TerminalFont::resolve(font_family, cx.theme().mode.is_dark(), || {
        cx.text_system().all_font_names()
    }).font.family;
    let editor_px = font_size.clamp(11.0, 24.0);
    let title_str = format!("编辑远程文件 · {file_name}");
    let path_str = remote_path.to_string();
    let save_cb = Rc::new(on_save);
    let saving = Rc::new(Cell::new(false));

    window.open_dialog(cx, move |dialog, _, cx| {
        let saving = saving.clone();
        let ed_save = editor.clone();
        let ed_lang = editor.clone();
        let cb = save_cb.clone();
        let cur_lang = active_lang.borrow().clone();
        let lang_state_menu = active_lang.clone();
        let detected_for_menu = detected_lang.clone();
        let custom_for_menu = custom_langs.clone();

        let lang_badge_label = if cur_lang == "Plain Text" {
            "纯文本 (Plain Text)".to_string()
        } else {
            COMMON_EDITOR_LANGUAGES
                .iter()
                .find(|(id, _)| *id == cur_lang)
                .map(|(_, label)| (*label).to_string())
                .unwrap_or_else(|| cur_lang.clone())
        };

        dialog
            .title(title_str.clone())
            .w(px(760.0))
            .child(
                dialog_body()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(path_str.clone()),
                            )
                            .child(
                                Button::new("remote-editor-lang-picker")
                                    .flex_shrink_0()
                                    .secondary()
                                    .xsmall()
                                    .icon(LucideIcon::Code)
                                    .label(format!("语法: {lang_badge_label}"))
                                    .dropdown_menu(move |mut menu, _, _| {
                                        let mut seen = std::collections::HashSet::new();

                                        // 1. Plain Text option
                                        {
                                            let ed = ed_lang.clone();
                                            let st = lang_state_menu.clone();
                                            seen.insert("Plain Text".to_string());
                                            menu = menu.item(
                                                PopupMenuItem::new("纯文本 (Plain Text)")
                                                    .checked(cur_lang == "Plain Text")
                                                    .on_click(move |_, window, cx| {
                                                        *st.borrow_mut() = "Plain Text".to_string();
                                                        ed.update(cx, |s, cx| {
                                                            s.set_highlighter("Plain Text", cx);
                                                            s.apply_highlighter_fold_candidates(Vec::new(), cx);
                                                        });
                                                        window.refresh();
                                                    }),
                                            );
                                        }

                                        // 2. Auto-detected syntax (if outside curated presets)
                                        if detected_for_menu != "Plain Text"
                                            && !COMMON_EDITOR_LANGUAGES
                                                .iter()
                                                .any(|(id, _)| *id == detected_for_menu)
                                            && seen.insert(detected_for_menu.clone())
                                        {
                                            let lang_id = detected_for_menu.clone();
                                            let ed = ed_lang.clone();
                                            let st = lang_state_menu.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(format!(
                                                    "{lang_id} (自动识别)"
                                                ))
                                                .checked(cur_lang == lang_id)
                                                .on_click(move |_, window, cx| {
                                                    *st.borrow_mut() = lang_id.clone();
                                                    ed.update(cx, |s, cx| {
                                                        s.set_highlighter(lang_id.clone(), cx);
                                                    });
                                                    window.refresh();
                                                }),
                                            );
                                        }

                                        // 3. User-installed custom extensions
                                        if !custom_for_menu.is_empty() {
                                            menu = menu.separator();
                                            for cust in &custom_for_menu {
                                                if !seen.insert(cust.clone()) {
                                                    continue;
                                                }
                                                let lang_id = cust.clone();
                                                let ed = ed_lang.clone();
                                                let st = lang_state_menu.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(format!(
                                                        "{lang_id} (拓展)"
                                                    ))
                                                    .checked(cur_lang == lang_id)
                                                    .on_click(move |_, window, cx| {
                                                        *st.borrow_mut() = lang_id.clone();
                                                        ed.update(cx, |s, cx| {
                                                            s.set_highlighter(lang_id.clone(), cx);
                                                        });
                                                        window.refresh();
                                                    }),
                                                );
                                            }
                                        }

                                        // 4. Common server & developer languages
                                        menu = menu.separator();
                                        for &(syn_id, label) in COMMON_EDITOR_LANGUAGES {
                                            if !seen.insert(syn_id.to_string()) {
                                                continue;
                                            }
                                            let lang_id = syn_id.to_string();
                                            let ed = ed_lang.clone();
                                            let st = lang_state_menu.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(label)
                                                    .checked(cur_lang == syn_id)
                                                    .on_click(move |_, window, cx| {
                                                        *st.borrow_mut() = lang_id.clone();
                                                        ed.update(cx, |s, cx| {
                                                            s.set_highlighter(lang_id.clone(), cx);
                                                        });
                                                        window.refresh();
                                                    }),
                                            );
                                        }
                                        menu
                                    }),
                            ),
                    )
                    .child(
                        Editor::new(&editor)
                            .h(px(412.0))
                            .readonly(saving.get())
                            .font_family(editor_font.clone())
                            .text_size(px(editor_px))
                            .text_color(
                                cx.theme()
                                    .highlight_theme
                                    .style
                                    .editor_foreground
                                    .unwrap_or(cx.theme().foreground),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("remote-edit-cancel")
                            .small()
                            .disabled(saving.get())
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("remote-edit-save")
                            .primary()
                            .small()
                            .disabled(saving.get())
                            .label(if saving.get() { "保存中…" } else { "保存到服务器" })
                            .on_click(move |_, window, cx| {
                                if saving.replace(true) {
                                    return;
                                }
                                let content = ed_save.read(cx).text().to_string();
                                let write = cb(content, cx);
                                let saving = saving.clone();
                                window.refresh();
                                window
                                    .spawn(cx, async move |cx| {
                                        let result = write.await;
                                        let _ = cx.update(|window, cx| {
                                            saving.set(false);
                                            match result {
                                                Ok(()) => window.close_dialog(cx),
                                                Err(err) => {
                                                    window.push_notification(
                                                        Notification::error(format!(
                                                            "保存失败：{err:#}"
                                                        )),
                                                        cx,
                                                    );
                                                    window.refresh();
                                                }
                                            }
                                        });
                                    })
                                    .detach();
                            }),
                    ),
            )
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DraftAuthTab {
    Password,
    KeyPath,
    KeyInline,
    Keychain,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DraftProxyTab {
    None,
    Socks5,
    Http,
    JumpHost,
}

pub struct SshHostModalInit {
    pub editing_node: Option<HostNode>,
    pub existing_secrets: SecretPayload,
    pub default_parent_id: Option<String>,
    pub folders: Vec<(String, String)>,
    pub jump_candidates: Vec<(String, String)>,
    pub keychains: Vec<KeychainEntry>,
}

pub fn open_ssh_host_modal(
    workspace: Entity<MicaWorkspace>,
    init: SshHostModalInit,
    window: &mut Window,
    cx: &mut App,
) {
    let SshHostModalInit {
        editing_node,
        existing_secrets,
        default_parent_id,
        folders,
        jump_candidates,
        keychains,
    } = init;

    let is_edit = editing_node.is_some();
    let node_id = editing_node
        .as_ref()
        .map(|n| n.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let init_parent = editing_node
        .as_ref()
        .and_then(|n| n.parent_id.clone())
        .or(default_parent_id);
    let init_name = editing_node
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_default();
    let cfg = editing_node
        .as_ref()
        .and_then(|n| n.as_host().cloned())
        .unwrap_or_default();

    let (init_auth_tab, init_key_path, init_kc_id) = match &cfg.auth {
        SshAuthConfig::Password => (DraftAuthTab::Password, String::new(), String::new()),
        SshAuthConfig::PrivateKeyPath { path } => {
            (DraftAuthTab::KeyPath, path.clone(), String::new())
        }
        SshAuthConfig::PrivateKeyInline => (DraftAuthTab::KeyInline, String::new(), String::new()),
        SshAuthConfig::Keychain { keychain_id } => {
            (DraftAuthTab::Keychain, String::new(), keychain_id.clone())
        }
    };

    let (init_proxy_tab, init_px_host, init_px_port, init_px_user, init_jump_id) = match &cfg.proxy
    {
        SshProxyConfig::None => (
            DraftProxyTab::None,
            String::new(),
            "1080".to_string(),
            String::new(),
            String::new(),
        ),
        SshProxyConfig::Socks5 {
            host,
            port,
            username,
        } => (
            DraftProxyTab::Socks5,
            host.clone(),
            port.to_string(),
            username.clone().unwrap_or_default(),
            String::new(),
        ),
        SshProxyConfig::Http {
            host,
            port,
            username,
        } => (
            DraftProxyTab::Http,
            host.clone(),
            port.to_string(),
            username.clone().unwrap_or_default(),
            String::new(),
        ),
        SshProxyConfig::JumpHost { host_id } => (
            DraftProxyTab::JumpHost,
            String::new(),
            "1080".to_string(),
            String::new(),
            host_id.clone(),
        ),
    };

    let name_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("例如：生产环境 API-01");
        s.set_value(init_name, window, cx);
        s
    });
    let host_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("IP 或域名，如 192.168.1.10");
        s.set_value(cfg.host.clone(), window, cx);
        s
    });
    let port_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("22");
        s.set_value(cfg.port.to_string(), window, cx);
        s
    });
    let user_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("root");
        s.set_value(cfg.user.clone(), window, cx);
        s
    });
    let env_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("选填，如 PROD / DEV");
        s.set_value(cfg.environment_tag.clone(), window, cx);
        s
    });
    let dir_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("选填，如 /var/www");
        s.set_value(cfg.default_remote_dir.clone(), window, cx);
        s
    });
    let remark_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("选填备注，支持左侧栏搜索");
        s.set_value(cfg.remark.clone(), window, cx);
        s
    });
    let pass_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("SSH 登录密码")
            .masked(true);
        if let Some(p) = existing_secrets.password.clone() {
            s.set_value(p, window, cx);
        }
        s
    });
    let key_path_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("~/.ssh/id_ed25519（自动展开 ~）");
        s.set_value(init_key_path, window, cx);
        s
    });
    let key_pem_in = cx.new(|cx| {
        let mut s = TextareaState::new(window, cx);
        if let Some(pem) = existing_secrets.private_key_pem.clone() {
            s.set_value(pem, window, cx);
        }
        s
    });
    let passphrase_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("私钥密码短语（无则留空）")
            .masked(true);
        if let Some(pp) = existing_secrets.passphrase.clone() {
            s.set_value(pp, window, cx);
        }
        s
    });
    let px_host_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("127.0.0.1");
        s.set_value(init_px_host, window, cx);
        s
    });
    let px_port_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("1080");
        s.set_value(init_px_port, window, cx);
        s
    });
    let px_user_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("代理用户名（选填）");
        s.set_value(init_px_user, window, cx);
        s
    });
    let px_pass_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("代理密码（选填）")
            .masked(true);
        if let Some(pp) = existing_secrets.proxy_password.clone() {
            s.set_value(pp, window, cx);
        }
        s
    });

    let auth_tab = Rc::new(Cell::new(init_auth_tab));
    let proxy_tab = Rc::new(Cell::new(init_proxy_tab));
    let selected_parent = Rc::new(RefCell::new(init_parent));
    let selected_kc = Rc::new(RefCell::new(if init_kc_id.is_empty() {
        keychains.first().map(|k| k.id.clone()).unwrap_or_default()
    } else {
        init_kc_id
    }));
    let selected_jump = Rc::new(RefCell::new(if init_jump_id.is_empty() {
        jump_candidates
            .first()
            .map(|(id, _)| id.clone())
            .unwrap_or_default()
    } else {
        init_jump_id
    }));
    let error_msg = Rc::new(RefCell::new(Option::<String>::None));
    let first_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if first_focus.replace(false) {
            name_in.focus_handle(cx).focus(window, cx);
        }

        let cur_auth = auth_tab.get();
        let cur_proxy = proxy_tab.get();
        let cur_parent = selected_parent.borrow().clone();
        let cur_kc = selected_kc.borrow().clone();
        let cur_jump = selected_jump.borrow().clone();
        let cur_err = error_msg.borrow().clone();

        let at1 = auth_tab.clone();
        let at2 = auth_tab.clone();
        let at3 = auth_tab.clone();
        let at4 = auth_tab.clone();

        let pt1 = proxy_tab.clone();
        let pt2 = proxy_tab.clone();
        let pt3 = proxy_tab.clone();
        let pt4 = proxy_tab.clone();

        let folder_selector = if folders.is_empty() {
            None
        } else {
            let sp_none = selected_parent.clone();
            Some(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("folder-opt-root")
                            .ghost()
                            .xsmall()
                            .selected(cur_parent.is_none())
                            .label("根目录")
                            .on_click(move |_, window, _| {
                                *sp_none.borrow_mut() = None;
                                window.refresh();
                            }),
                    )
                    .children(folders.iter().map(|(fid, flabel)| {
                        let fid = fid.clone();
                        let flabel = flabel.clone();
                        let sp = selected_parent.clone();
                        let active = cur_parent.as_deref() == Some(fid.as_str());
                        Button::new(SharedString::from(format!("folder-opt-{fid}")))
                            .ghost()
                            .xsmall()
                            .selected(active)
                            .label(flabel)
                            .on_click(move |_, window, _| {
                                *sp.borrow_mut() = Some(fid.clone());
                                window.refresh();
                            })
                    })),
            )
        };

        let auth_section = match cur_auth {
            DraftAuthTab::Password => field_block("登录密码", Input::new(&pass_in).small(), cx),
            DraftAuthTab::KeyPath => v_flex()
                .gap_2()
                .child(field_block(
                    "本机私钥文件路径",
                    Input::new(&key_path_in).small(),
                    cx,
                ))
                .child(field_block(
                    "私钥密码短语（选填）",
                    Input::new(&passphrase_in).small(),
                    cx,
                )),
            DraftAuthTab::KeyInline => v_flex()
                .gap_2()
                .child(field_block(
                    "PEM / OpenSSH 私钥内容",
                    Textarea::new(&key_pem_in).h(px(96.0)),
                    cx,
                ))
                .child(field_block(
                    "私钥密码短语（选填）",
                    Input::new(&passphrase_in).small(),
                    cx,
                )),
            DraftAuthTab::Keychain => {
                if keychains.is_empty() {
                    v_flex().child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("左侧「密钥」栏暂无凭据，请先创建密钥凭据或改用密码/私钥方式。"),
                    )
                } else {
                    v_flex().gap_1p5().child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .children(keychains.iter().map(|kc| {
                                let kid = kc.id.clone();
                                let skc = selected_kc.clone();
                                let active = cur_kc == kid;
                                Button::new(SharedString::from(format!("pick-kc-{kid}")))
                                    .secondary()
                                    .xsmall()
                                    .selected(active)
                                    .label(format!("{} ({})", kc.name, kc.kind.label()))
                                    .on_click(move |_, window, _| {
                                        *skc.borrow_mut() = kid.clone();
                                        window.refresh();
                                    })
                            })),
                    )
                }
            }
        };

        let proxy_section = match cur_proxy {
            DraftProxyTab::None => div(),
            DraftProxyTab::Socks5 | DraftProxyTab::Http => v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .child(field_block("代理地址", Input::new(&px_host_in).small(), cx)),
                        )
                        .child(
                            div()
                                .w(px(100.0))
                                .child(field_block("端口", Input::new(&px_port_in).small(), cx)),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div().flex_1().child(field_block(
                                "代理账号",
                                Input::new(&px_user_in).small(),
                                cx,
                            )),
                        )
                        .child(
                            div().flex_1().child(field_block(
                                "代理密码",
                                Input::new(&px_pass_in).small(),
                                cx,
                            )),
                        ),
                ),
            DraftProxyTab::JumpHost => {
                if jump_candidates.is_empty() {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("暂无其他已保存主机可作为跳板机")
                } else {
                    h_flex()
                        .flex_wrap()
                        .gap_1()
                        .children(jump_candidates.iter().map(|(jid, jname)| {
                            let jid = jid.clone();
                            let jname = jname.clone();
                            let sj = selected_jump.clone();
                            let active = cur_jump == jid;
                            Button::new(SharedString::from(format!("pick-jump-{jid}")))
                                .secondary()
                                .xsmall()
                                .selected(active)
                                .label(jname)
                                .on_click(move |_, window, _| {
                                    *sj.borrow_mut() = jid.clone();
                                    window.refresh();
                                })
                        }))
                }
            }
        };

        let build_save = {
            let ws = workspace.clone();
            let nid = node_id.clone();
            let name_in = name_in.clone();
            let host_in = host_in.clone();
            let port_in = port_in.clone();
            let user_in = user_in.clone();
            let env_in = env_in.clone();
            let dir_in = dir_in.clone();
            let remark_in = remark_in.clone();
            let pass_in = pass_in.clone();
            let key_path_in = key_path_in.clone();
            let key_pem_in = key_pem_in.clone();
            let passphrase_in = passphrase_in.clone();
            let px_host_in = px_host_in.clone();
            let px_port_in = px_port_in.clone();
            let px_user_in = px_user_in.clone();
            let px_pass_in = px_pass_in.clone();
            let auth_tab = auth_tab.clone();
            let proxy_tab = proxy_tab.clone();
            let selected_parent = selected_parent.clone();
            let selected_kc = selected_kc.clone();
            let selected_jump = selected_jump.clone();
            let error_msg = error_msg.clone();

            Rc::new(move |connect_now: bool, window: &mut Window, cx: &mut App| {
                let host_val = host_in.read(cx).value().trim().to_string();
                if host_val.is_empty() {
                    *error_msg.borrow_mut() = Some("请填写服务器主机地址 (IP 或域名)".into());
                    window.refresh();
                    return;
                }
                let mut name_val = name_in.read(cx).value().trim().to_string();
                if name_val.is_empty() {
                    name_val = host_val.clone();
                }
                let port_val: u16 = port_in.read(cx).value().trim().parse().unwrap_or(22);
                let mut user_val = user_in.read(cx).value().trim().to_string();
                if user_val.is_empty() {
                    user_val = "root".to_string();
                }

                let auth = match auth_tab.get() {
                    DraftAuthTab::Password => SshAuthConfig::Password,
                    DraftAuthTab::KeyPath => {
                        let p = key_path_in.read(cx).value().trim().to_string();
                        if p.is_empty() {
                            *error_msg.borrow_mut() = Some("请填写私钥文件路径（如 ~/.ssh/id_ed25519）".into());
                            window.refresh();
                            return;
                        }
                        SshAuthConfig::PrivateKeyPath { path: p }
                    }
                    DraftAuthTab::KeyInline => SshAuthConfig::PrivateKeyInline,
                    DraftAuthTab::Keychain => {
                        let kid = selected_kc.borrow().clone();
                        if kid.is_empty() {
                            *error_msg.borrow_mut() = Some("请选择一个密钥库凭据".into());
                            window.refresh();
                            return;
                        }
                        SshAuthConfig::Keychain { keychain_id: kid }
                    }
                };

                let proxy = match proxy_tab.get() {
                    DraftProxyTab::None => SshProxyConfig::None,
                    DraftProxyTab::Socks5 => {
                        let px_h = px_host_in.read(cx).value().trim().to_string();
                        if px_h.is_empty() {
                            *error_msg.borrow_mut() = Some("请填写 SOCKS5 代理地址".into());
                            window.refresh();
                            return;
                        }
                        SshProxyConfig::Socks5 {
                            host: px_h,
                            port: px_port_in.read(cx).value().trim().parse().unwrap_or(1080),
                            username: Some(px_user_in.read(cx).value().trim().to_string())
                                .filter(|s| !s.is_empty()),
                        }
                    }
                    DraftProxyTab::Http => {
                        let px_h = px_host_in.read(cx).value().trim().to_string();
                        if px_h.is_empty() {
                            *error_msg.borrow_mut() = Some("请填写 HTTP 代理地址".into());
                            window.refresh();
                            return;
                        }
                        SshProxyConfig::Http {
                            host: px_h,
                            port: px_port_in.read(cx).value().trim().parse().unwrap_or(8080),
                            username: Some(px_user_in.read(cx).value().trim().to_string())
                                .filter(|s| !s.is_empty()),
                        }
                    }
                    DraftProxyTab::JumpHost => {
                        let jid = selected_jump.borrow().clone();
                        if jid.is_empty() {
                            SshProxyConfig::None
                        } else {
                            SshProxyConfig::JumpHost { host_id: jid }
                        }
                    }
                };

                let secrets = SecretPayload {
                    password: Some(pass_in.read(cx).value().to_string()).filter(|s| !s.is_empty()),
                    private_key_pem: Some(key_pem_in.read(cx).value().to_string())
                        .filter(|s| !s.trim().is_empty()),
                    passphrase: Some(passphrase_in.read(cx).value().to_string())
                        .filter(|s| !s.is_empty()),
                    proxy_password: Some(px_pass_in.read(cx).value().to_string())
                        .filter(|s| !s.is_empty()),
                };

                let node = HostNode {
                    id: nid.clone(),
                    parent_id: selected_parent.borrow().clone(),
                    name: name_val,
                    kind: HostNodeKind::Host(Box::new(SshHostConfig {
                        host: host_val,
                        port: port_val,
                        user: user_val,
                        auth,
                        proxy,
                        default_remote_dir: dir_in.read(cx).value().trim().to_string(),
                        environment_tag: env_in.read(cx).value().trim().to_string(),
                        remark: remark_in.read(cx).value().trim().to_string(),
                    })),
                };

                let saved_id = node.id.clone();
                let ws_after_close = ws.clone();
                window.close_dialog(cx);
                ws.update(cx, |ws, cx| {
                    ws.save_host_node(node, secrets, window, cx);
                });
                if connect_now {
                    window.defer(cx, move |window, cx| {
                        ws_after_close.update(cx, |ws, cx| {
                            ws.connect_host_in_new_tab(&saved_id, window, cx);
                        });
                    });
                }
            })
        };

        let save_only = build_save.clone();
        let save_and_connect = build_save;

        dialog
            .title(if is_edit {
                "编辑 SSH 主机"
            } else {
                "新建 SSH 主机"
            })
            .w(px(540.0))
            .child(
                dialog_body()
                    .gap_3()
                    .children(cur_err.map(|msg| {
                        div()
                            .px_3()
                            .py_1p5()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().danger.opacity(0.15))
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(msg)
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(field_block("显示名称", Input::new(&name_in).small(), cx)),
                            )
                            .child(
                                div()
                                    .w(px(120.0))
                                    .child(field_block("环境标签", Input::new(&env_in).small(), cx)),
                            ),
                    )
                    .children(folder_selector.map(|fs| field_block("所属分组文件夹", fs, cx)))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(field_block("主机地址 *", Input::new(&host_in).small(), cx)),
                            )
                            .child(
                                div()
                                    .w(px(86.0))
                                    .child(field_block("端口", Input::new(&port_in).small(), cx)),
                            )
                            .child(
                                div()
                                    .w(px(120.0))
                                    .child(field_block("用户名", Input::new(&user_in).small(), cx)),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("认证方式"),
                            )
                            .child(
                                macos_segmented_track(cx)
                                    .child(
                                        Button::new("auth-tab-pwd")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_auth == DraftAuthTab::Password)
                                            .label("密码")
                                            .on_click(move |_, w, _| {
                                                at1.set(DraftAuthTab::Password);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("auth-tab-keypath")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_auth == DraftAuthTab::KeyPath)
                                            .label("私钥文件")
                                            .on_click(move |_, w, _| {
                                                at2.set(DraftAuthTab::KeyPath);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("auth-tab-keyinline")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_auth == DraftAuthTab::KeyInline)
                                            .label("粘贴私钥")
                                            .on_click(move |_, w, _| {
                                                at3.set(DraftAuthTab::KeyInline);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("auth-tab-kc")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_auth == DraftAuthTab::Keychain)
                                            .label("密钥库凭据")
                                            .on_click(move |_, w, _| {
                                                at4.set(DraftAuthTab::Keychain);
                                                w.refresh();
                                            }),
                                    ),
                            )
                            .child(auth_section),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("网络代理 / 跳板机"),
                            )
                            .child(
                                macos_segmented_track(cx)
                                    .child(
                                        Button::new("px-tab-none")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_proxy == DraftProxyTab::None)
                                            .label("直连")
                                            .on_click(move |_, w, _| {
                                                pt1.set(DraftProxyTab::None);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("px-tab-socks5")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_proxy == DraftProxyTab::Socks5)
                                            .label("SOCKS5")
                                            .on_click(move |_, w, _| {
                                                pt2.set(DraftProxyTab::Socks5);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("px-tab-http")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_proxy == DraftProxyTab::Http)
                                            .label("HTTP")
                                            .on_click(move |_, w, _| {
                                                pt3.set(DraftProxyTab::Http);
                                                w.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("px-tab-jump")
                                            .ghost()
                                            .xsmall()
                                            .flex_1()
                                            .selected(cur_proxy == DraftProxyTab::JumpHost)
                                            .label("SSH 跳板机")
                                            .on_click(move |_, w, _| {
                                                pt4.set(DraftProxyTab::JumpHost);
                                                w.refresh();
                                            }),
                                    ),
                            )
                            .child(proxy_section),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div().flex_1().child(field_block(
                                    "SFTP 初始远程路径（选填）",
                                    Input::new(&dir_in).small(),
                                    cx,
                                )),
                            )
                            .child(
                                div().flex_1().child(field_block(
                                    "备注（选填）",
                                    Input::new(&remark_in).small(),
                                    cx,
                                )),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("ssh-modal-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ssh-modal-save")
                            .secondary()
                            .small()
                            .label("仅保存")
                            .on_click(move |_, window, cx| save_only(false, window, cx)),
                    )
                    .child(
                        Button::new("ssh-modal-connect")
                            .primary()
                            .small()
                            .label("保存并连接")
                            .on_click(move |_, window, cx| save_and_connect(true, window, cx)),
                    ),
            )
    });
}

pub fn open_keychain_modal(
    workspace: Entity<MicaWorkspace>,
    editing: Option<KeychainEntry>,
    existing_secrets: SecretPayload,
    window: &mut Window,
    cx: &mut App,
) {
    let is_edit = editing.is_some();
    let entry_id = editing
        .as_ref()
        .map(|e| e.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let init_kind = editing
        .as_ref()
        .map(|e| e.kind)
        .unwrap_or(KeychainKind::PasswordIdentity);
    let init_remark = editing
        .as_ref()
        .map(|e| e.remark.clone())
        .unwrap_or_default();

    let name_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("例如：集群统一运维账号");
        if let Some(e) = &editing {
            s.set_value(e.name.clone(), window, cx);
        }
        s
    });
    let user_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("默认用户名（选填，如 ubuntu）");
        if let Some(e) = &editing {
            s.set_value(e.username.clone(), window, cx);
        }
        s
    });
    let remark_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("备注（选填）");
        if !init_remark.is_empty() {
            s.set_value(init_remark, window, cx);
        }
        s
    });
    let pwd_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("登录密码")
            .masked(true);
        if let Some(p) = existing_secrets.password.clone() {
            s.set_value(p, window, cx);
        }
        s
    });
    let pem_in = cx.new(|cx| {
        let mut s = TextareaState::new(window, cx);
        if let Some(pem) = existing_secrets.private_key_pem.clone() {
            s.set_value(pem, window, cx);
        }
        s
    });
    let pp_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("私钥密码短语（选填）")
            .masked(true);
        if let Some(pp) = existing_secrets.passphrase.clone() {
            s.set_value(pp, window, cx);
        }
        s
    });

    let kind_cell = Rc::new(Cell::new(init_kind));

    window.open_dialog(cx, move |dialog, _, cx| {
        let k = kind_cell.get();
        let kc1 = kind_cell.clone();
        let kc2 = kind_cell.clone();

        let ws = workspace.clone();
        let eid = entry_id.clone();
        let n_in = name_in.clone();
        let u_in = user_in.clone();
        let r_in = remark_in.clone();
        let pw_in = pwd_in.clone();
        let pm_in = pem_in.clone();
        let ph_in = pp_in.clone();
        let kc_save = kind_cell.clone();

        dialog
            .title(if is_edit {
                "编辑密钥凭据"
            } else {
                "新建密钥凭据"
            })
            .w(px(460.0))
            .child(
                dialog_body()
                    .gap_3()
                    .child(
                        macos_segmented_track(cx)
                            .child(
                                Button::new("kc-kind-pwd")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .selected(k == KeychainKind::PasswordIdentity)
                                    .label("账号密码")
                                    .on_click(move |_, w, _| {
                                        kc1.set(KeychainKind::PasswordIdentity);
                                        w.refresh();
                                    }),
                            )
                            .child(
                                Button::new("kc-kind-ssh")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .selected(k == KeychainKind::SshKey)
                                    .label("SSH 私钥")
                                    .on_click(move |_, w, _| {
                                        kc2.set(KeychainKind::SshKey);
                                        w.refresh();
                                    }),
                            ),
                    )
                    .child(field_block("凭据名称", Input::new(&name_in).small(), cx))
                    .child(field_block("关联用户名", Input::new(&user_in).small(), cx))
                    .child(match k {
                        KeychainKind::PasswordIdentity => {
                            field_block("密码", Input::new(&pwd_in).small(), cx)
                        }
                        KeychainKind::SshKey => v_flex()
                            .gap_2()
                            .child(field_block(
                                "私钥内容 (PEM / OpenSSH)",
                                Textarea::new(&pem_in).h(px(120.0)),
                                cx,
                            ))
                            .child(field_block(
                                "密码短语（选填）",
                                Input::new(&pp_in).small(),
                                cx,
                            )),
                    })
                    .child(field_block("备注", Input::new(&remark_in).small(), cx)),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("kc-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("kc-save")
                            .primary()
                            .small()
                            .label("保存凭据")
                            .on_click(move |_, window, cx| {
                                let name = n_in.read(cx).value().trim().to_string();
                                if name.is_empty() {
                                    return;
                                }
                                let entry = KeychainEntry {
                                    id: eid.clone(),
                                    name,
                                    kind: kc_save.get(),
                                    username: u_in.read(cx).value().trim().to_string(),
                                    remark: r_in.read(cx).value().trim().to_string(),
                                };
                                let payload = SecretPayload {
                                    password: Some(pw_in.read(cx).value().to_string())
                                        .filter(|s| !s.is_empty()),
                                    private_key_pem: Some(pm_in.read(cx).value().to_string())
                                        .filter(|s| !s.trim().is_empty()),
                                    passphrase: Some(ph_in.read(cx).value().to_string())
                                        .filter(|s| !s.is_empty()),
                                    proxy_password: None,
                                };
                                window.close_dialog(cx);
                                ws.update(cx, |ws, cx| ws.save_keychain(entry, payload, window, cx));
                            }),
                    ),
            )
    });
}

pub fn open_snippet_modal(
    workspace: Entity<MicaWorkspace>,
    editing: Option<SnippetItem>,
    window: &mut Window,
    cx: &mut App,
) {
    let is_edit = editing.is_some();
    let sid = editing
        .as_ref()
        .map(|s| s.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let init_auto = editing.as_ref().map(|s| s.auto_enter).unwrap_or(true);

    let title_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("例如：查看 Nginx 错误日志");
        if let Some(it) = &editing {
            s.set_value(it.title.clone(), window, cx);
        }
        s
    });
    let group_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("分类（选填，如 运维常用）");
        if let Some(it) = &editing {
            s.set_value(it.group.clone(), window, cx);
        }
        s
    });
    let cmd_in = cx.new(|cx| {
        let mut s = TextareaState::new(window, cx);
        if let Some(it) = &editing {
            s.set_value(it.command.clone(), window, cx);
        }
        s
    });
    let auto_enter = Rc::new(Cell::new(init_auto));

    window.open_dialog(cx, move |dialog, _, cx| {
        let checked = auto_enter.get();
        let ae_toggle = auto_enter.clone();
        let ae_save = auto_enter.clone();
        let ws = workspace.clone();
        let id_save = sid.clone();
        let t_in = title_in.clone();
        let g_in = group_in.clone();
        let c_in = cmd_in.clone();

        dialog
            .title(if is_edit {
                "编辑命令片段"
            } else {
                "新建命令片段"
            })
            .w(px(460.0))
            .child(
                dialog_body()
                    .gap_3()
                    .child(field_block("片段名称", Input::new(&title_in).small(), cx))
                    .child(field_block("分组标签", Input::new(&group_in).small(), cx))
                    .child(field_block(
                        "Shell 命令内容",
                        Textarea::new(&cmd_in).h(px(110.0)),
                        cx,
                    ))
                    .child(
                        Switch::new("snippet-auto-enter")
                            .small()
                            .checked(checked)
                            .label("发送后自动附加回车立即执行")
                            .on_change(move |val, window, _| {
                                ae_toggle.set(*val);
                                window.refresh();
                            }),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("snip-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("snip-save")
                            .primary()
                            .small()
                            .label("保存片段")
                            .on_click(move |_, window, cx| {
                                let title = t_in.read(cx).value().trim().to_string();
                                let command = c_in.read(cx).value().trim().to_string();
                                if title.is_empty() || command.is_empty() {
                                    return;
                                }
                                let item = SnippetItem {
                                    id: id_save.clone(),
                                    title,
                                    group: g_in.read(cx).value().trim().to_string(),
                                    command,
                                    auto_enter: ae_save.get(),
                                };
                                window.close_dialog(cx);
                                ws.update(cx, |ws, cx| ws.save_snippet(item, cx));
                            }),
                    ),
            )
    });
}

struct ShortcutRecorder {
    workspace: Entity<MicaWorkspace>,
    recorder_focus: FocusHandle,
    recording_action: Option<ShortcutAction>,
    status: SharedString,
}

impl ShortcutRecorder {
    fn assign(
        &mut self,
        action: ShortcutAction,
        chord: Option<ShortcutChord>,
        cx: &mut Context<Self>,
    ) {
        let label = action.label();
        let binding = chord.as_ref().map(format_chord);
        let conflict = self
            .workspace
            .update(cx, |ws, cx| ws.set_shortcut(action, chord, cx));
        self.status = match binding {
            Some(keys) => {
                let mut message = format!("「{label}」已设为 {keys}");
                if let Some(other) = conflict {
                    message.push_str(&format!("，已解除「{}」的绑定", other.label()));
                }
                message
            }
            None => format!("「{label}」已设为未设置"),
        }
        .into();
        self.recording_action = None;
        cx.notify();
    }

    fn handle_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.recording_action else {
            return;
        };
        let ks = &ev.keystroke;
        cx.stop_propagation();
        window.prevent_default();
        match ks.key.as_str() {
            "escape" => {
                self.recording_action = None;
                self.status = "已取消录制".into();
            }
            "backspace" | "delete" if !ks.modifiers.modified() => {
                return self.assign(action, None, cx);
            }
            _ => {
                if ShortcutChord::from_keystroke(ks).is_none() {
                    return;
                }
                match ShortcutChord::try_record(ks) {
                    Ok(chord) => return self.assign(action, Some(chord), cx),
                    Err(message) => self.status = message.into(),
                }
            }
        }
        cx.notify();
    }

    fn row(
        &self,
        action: ShortcutAction,
        shortcuts: &ShortcutConfig,
        cx: &Context<Self>,
    ) -> gpui_kit::Div {
        let chord = shortcuts.get(action);
        let recording = self.recording_action == Some(action);
        let badge = if recording {
            "请按下组合键… (Esc 取消)".into()
        } else {
            chord
                .as_ref()
                .map(format_chord)
                .unwrap_or_else(|| "未设置".into())
        };
        h_flex()
            .px_3()
            .py_1p5()
            .gap_1()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().min_w_0().flex_1().text_sm().child(action.label()))
            .child(
                Button::new(("record-shortcut", action as usize))
                    .flex_shrink_0()
                    .small()
                    .secondary()
                    .tab_stop(false)
                    .when(recording, |b| b.primary())
                    .when(!recording && chord.is_none(), |b| {
                        b.text_color(cx.theme().muted_foreground)
                    })
                    .label(badge)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.recording_action = Some(action);
                        this.recorder_focus.focus(window, cx);
                        cx.notify();
                    })),
            )
            .when(chord.is_some() && !recording, |row| {
                row.child(
                    Button::new(("clear-shortcut", action as usize))
                        .flex_shrink_0()
                        .ghost()
                        .xsmall()
                        .compact()
                        .tab_stop(false)
                        .icon(LucideIcon::X)
                        .tooltip("设为空（不绑定快捷键）")
                        .on_click(cx.listener(move |this, _, _, cx| this.assign(action, None, cx))),
                )
            })
            .when(shortcuts.is_customized(action), |row| {
                row.child(
                    Button::new(("reset-shortcut", action as usize))
                        .flex_shrink_0()
                        .ghost()
                        .xsmall()
                        .compact()
                        .tab_stop(false)
                        .icon(LucideIcon::RotateCcw)
                        .tooltip("恢复默认快捷键")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.workspace
                                .update(cx, |ws, cx| ws.reset_shortcut(action, cx));
                            this.recording_action = None;
                            this.status =
                                format!("已恢复「{}」的默认快捷键", action.label()).into();
                            cx.notify();
                        })),
                )
            })
    }
}

impl Render for ShortcutRecorder {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let shortcuts = &ws.settings.shortcuts;
        let customized = ShortcutAction::ALL
            .iter()
            .any(|&a| shortcuts.is_customized(a));
        let status = self.status.clone();
        v_flex()
            .gap_2()
            .track_focus(&self.recorder_focus)
            .when(self.recording_action.is_some(), |container| {
                container
                    .key_context("ShortcutRecorder")
                    .capture_key_down(cx.listener(Self::handle_key_down))
            })
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    )
                    .child(
                        Button::new("reset-all-shortcuts")
                            .flex_shrink_0()
                            .secondary()
                            .small()
                            .tab_stop(false)
                            .label("全部恢复默认")
                            .disabled(!customized)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workspace
                                    .update(cx, |ws, cx| ws.reset_all_shortcuts(cx));
                                this.recording_action = None;
                                this.status = "已恢复全部默认快捷键".into();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .max_h(px(420.0))
                    .w_full()
                    .min_w_0()
                    .pr_1()
                    .overflow_y_scrollbar()
                    .children(ShortcutCategory::ALL.into_iter().map(|category| {
                        macos_inset_group(cx)
                            .flex_shrink_0()
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(category.label()),
                            )
                            .children(
                                ShortcutAction::ALL
                                    .iter()
                                    .copied()
                                    .filter(|a| a.category() == category)
                                    .map(|action| self.row(action, shortcuts, cx)),
                            )
                    })),
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Themes,
    General,
    Shortcuts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeModeFilter {
    All,
    Dark,
    Light,
}

#[inline(never)]
fn render_theme_preview_card(
    spec: &ThemeSpec,
    is_active_now: bool,
    is_saved_dark: bool,
    is_saved_light: bool,
    workspace: Entity<MicaWorkspace>,
    cx: &App,
) -> AnyElement {
    let tid = spec.id.clone();
    let titlebar_c = macos_hex(spec.titlebar_bg, 1.0);
    let sidebar_c = macos_hex(spec.sidebar_bg, 1.0);
    let term_bg_c = macos_hex(spec.term_bg, 1.0);
    let term_fg_c = macos_hex(spec.term_fg, 1.0);
    let surface_c = macos_hex(spec.surface_bg, 1.0);
    let muted_c = macos_hex(spec.muted_fg, 1.0);
    let accent_c = macos_hex(spec.accent, 1.0);
    let cursor_c = macos_hex(spec.cursor, 1.0);
    let divider_c = macos_hex(if spec.dark { 0xFFFFFF } else { 0x000000 }, 0.09);

    let dot_red = macos_hex(spec.ansi[1], 1.0);
    let dot_yellow = macos_hex(spec.ansi[3], 1.0);
    let dot_green = macos_hex(spec.ansi[2], 1.0);
    let prompt_cyan = macos_hex(spec.ansi[6], 1.0);
    let cmd_blue = macos_hex(spec.ansi[4], 1.0);

    let swatch_colors = [
        spec.ansi[1],
        spec.ansi[2],
        spec.ansi[3],
        spec.ansi[4],
        spec.ansi[5],
        spec.ansi[6],
        spec.accent,
        spec.ansi[13],
    ];

    let status_pill = if is_active_now {
        Some(("当前生效", cx.theme().primary.opacity(0.18), cx.theme().primary))
    } else if is_saved_dark {
        Some((
            "深色预设",
            cx.theme().secondary,
            cx.theme().muted_foreground,
        ))
    } else if is_saved_light {
        Some((
            "浅色预设",
            cx.theme().secondary,
            cx.theme().muted_foreground,
        ))
    } else {
        None
    };

    let tid_key = tid.clone();
    let ws_key = workspace.clone();

    v_flex()
        .id(SharedString::from(format!("theme-card-{}", spec.id)))
        .tab_stop(true)
        .flex_1()
        .min_w(px(0.0))
        .rounded(px(9.0))
        .overflow_hidden()
        .cursor_pointer()
        .border_1()
        .border_color(if is_active_now {
            cx.theme().primary
        } else {
            cx.theme().border
        })
        .bg(if is_active_now {
            cx.theme().list_active
        } else {
            cx.theme().group_box
        })
        .hover(|s| {
            if is_active_now {
                s
            } else {
                s.border_color(cx.theme().primary.opacity(0.55))
            }
        })
        .on_key_down(move |ev: &KeyDownEvent, window, cx| {
            if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                ws_key.update(cx, |ws, cx| ws.select_theme(&tid_key, window, cx));
                window.refresh();
            }
        })
        .on_click(move |_, window, cx| {
            workspace.update(cx, |ws, cx| ws.select_theme(&tid, window, cx));
            window.refresh();
        })
        .child(
            // Mini Rex/Termius window + terminal canvas preview
            v_flex()
                .h(px(62.0))
                .w_full()
                .border_b_1()
                .border_color(divider_c)
                .child(
                    h_flex()
                        .h(px(15.0))
                        .px_2()
                        .gap_1()
                        .items_center()
                        .bg(titlebar_c)
                        .border_b_1()
                        .border_color(divider_c)
                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(dot_red))
                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(dot_yellow))
                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(dot_green))
                        .child(
                            div()
                                .ml_1p5()
                                .w(px(36.0))
                                .h(px(8.0))
                                .rounded(px(3.0))
                                .bg(surface_c),
                        ),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .w_full()
                        .child(
                            v_flex()
                                .w(px(46.0))
                                .h_full()
                                .px_1p5()
                                .py_1p5()
                                .gap_1()
                                .bg(sidebar_c)
                                .border_r_1()
                                .border_color(divider_c)
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(6.0))
                                        .rounded(px(2.5))
                                        .bg(accent_c.opacity(0.35)),
                                )
                                .child(
                                    div()
                                        .w(px(26.0))
                                        .h(px(5.0))
                                        .rounded(px(2.5))
                                        .bg(muted_c.opacity(0.32)),
                                )
                                .child(
                                    div()
                                        .w(px(30.0))
                                        .h(px(5.0))
                                        .rounded(px(2.5))
                                        .bg(muted_c.opacity(0.22)),
                                ),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .h_full()
                                .px_2()
                                .py_1()
                                .justify_between()
                                .bg(term_bg_c)
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(div().text_color(prompt_cyan).child("~"))
                                        .child(div().text_color(accent_c).child("❯"))
                                        .child(div().text_color(cmd_blue).child("ssh"))
                                        .child(div().text_color(term_fg_c).child("prod-01"))
                                        .child(
                                            div()
                                                .w(px(5.5))
                                                .h(px(10.5))
                                                .rounded(px(1.5))
                                                .bg(cursor_c),
                                        ),
                                )
                                .child(
                                    h_flex().gap_1().items_center().children(
                                        swatch_colors.into_iter().map(|hex| {
                                            div()
                                                .w(px(11.0))
                                                .h(px(5.5))
                                                .rounded(px(2.0))
                                                .bg(macos_hex(hex, 1.0))
                                        }),
                                    ),
                                ),
                        ),
                ),
        )
        .child(
            v_flex()
                .px_2p5()
                .py_2()
                .gap_0p5()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .gap_1p5()
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_1()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child(spec.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .px_1p5()
                                        .rounded(px(4.0))
                                        .bg(cx.theme().secondary)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(spec.badge.clone()),
                                ),
                        )
                        .children(status_pill.map(|(txt, bg, fg)| {
                            div()
                                .flex_shrink_0()
                                .px_1p5()
                                .rounded(px(4.0))
                                .bg(bg)
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(fg)
                                .child(txt)
                        })),
                )
                .child(
                    div()
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} · {}",
                            if spec.dark { "深色" } else { "浅色" },
                            spec.subtitle
                        )),
                ),
        )
        .into_any_element()
}

pub fn open_settings_modal(
    workspace: Entity<MicaWorkspace>,
    paths: AppPaths,
    window: &mut Window,
    cx: &mut App,
) {
    open_settings_modal_at(workspace, paths, SettingsTab::Themes, window, cx);
}

pub fn open_settings_modal_at(
    workspace: Entity<MicaWorkspace>,
    paths: AppPaths,
    initial_tab: SettingsTab,
    window: &mut Window,
    cx: &mut App,
) {
    let data_dir_display = paths.root_dir.display().to_string();
    let logs_dir_display = paths.logs_dir.display().to_string();
    let init_settings = workspace.read(cx).settings.clone();
    let active_page = Rc::new(Cell::new(initial_tab));
    let mode_filter = Rc::new(Cell::new(ThemeModeFilter::All));
    let category_filter: Rc<Cell<Option<ThemeCategory>>> = Rc::new(Cell::new(None));
    let theme_query = Rc::new(RefCell::new(String::new()));
    let tabs_focus = cx.focus_handle();
    let recorder = cx.new(|cx| ShortcutRecorder {
        workspace: workspace.clone(),
        recorder_focus: cx.focus_handle(),
        recording_action: None,
        status: "点击按键即可录制新组合键；录制中按 Backspace 置空，按 Esc 取消".into(),
    });
    let theme_search_in = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("搜索主题：Rex、Termius、Catppuccin、Tokyo Night、Dracula、Flexoki...")
    });
    {
        let q_ref = theme_query.clone();
        workspace.update(cx, |_, cx| {
            cx.subscribe_in(&theme_search_in, window, move |_, state, ev, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    *q_ref.borrow_mut() = state.read(cx).value().to_string();
                    window.refresh();
                }
            })
            .detach();
        });
    }
    let font_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder(BUNDLED_SARASA_FONT_FAMILY);
        s.set_value(init_settings.font_family.clone(), window, cx);
        s
    });
    workspace.update(cx, |_, cx| {
        cx.subscribe_in(&font_in, window, |ws, state, ev, window, cx| {
            match ev {
                InputEvent::Change => window.refresh(),
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    ws.set_font_family(state.read(cx).value().to_string(), cx);
                }
                _ => {}
            }
        })
        .detach();
    });

    window.open_dialog(cx, move |dialog, window, cx| {
        let ws_apply_font = workspace.clone();
        let f_in = font_in.clone();
        let tabs = macos_segmented_track(cx).track_focus(&tabs_focus).children(
            [
                ("settings-themes-tab", "外观与主题", SettingsTab::Themes),
                ("settings-general-tab", "通用与终端", SettingsTab::General),
                ("settings-shortcuts-tab", "快捷键", SettingsTab::Shortcuts),
            ]
            .into_iter()
            .map(|(id, label, tab_kind)| {
                let page = active_page.clone();
                let focus = tabs_focus.clone();
                let recorder = recorder.clone();
                Button::new(id)
                    .ghost()
                    .small()
                    .flex_1()
                    .label(label)
                    .selected(page.get() == tab_kind)
                    .on_click(move |_, window, cx| {
                        page.set(tab_kind);
                        recorder.update(cx, |r, cx| {
                            r.recording_action = None;
                            cx.notify();
                        });
                        focus.focus(window, cx);
                        window.refresh();
                    })
            }),
        );
        let body = dialog_body().gap_2p5().child(tabs);
        let dialog = dialog.title("偏好设置").w(px(690.0)).footer(
            DialogFooter::new().child(
                Button::new("settings-done")
                    .primary()
                    .small()
                    .label("完成")
                    .on_click(move |_, window, cx| {
                        let fam = f_in.read(cx).value().to_string();
                        ws_apply_font.update(cx, |ws, cx| ws.set_font_family(fam, cx));
                        window.close_dialog(cx);
                    }),
            ),
        );
        match active_page.get() {
            SettingsTab::Shortcuts => return dialog.child(body.child(recorder.clone())),
            SettingsTab::Themes => {
                let cur = workspace.read(cx).settings.clone();
                let pack = ThemeRegistry::global().current();
                let dark_spec = pack.resolve(&cur.dark_theme, true);
                let light_spec = pack.resolve(&cur.light_theme, false);
                let active_id = if cur.dark_mode {
                    dark_spec.id.clone()
                } else {
                    light_spec.id.clone()
                };

                let short_theme_name = |name: &str| match name.char_indices().nth(12) {
                    Some((end, _)) => format!("{}…", &name[..end]),
                    None => name.to_string(),
                };
                let total_cnt = pack.themes.len();
                let dark_cnt = pack.themes.iter().filter(|t| t.dark).count();
                let light_cnt = total_cnt.saturating_sub(dark_cnt);
                let has_custom = pack
                    .themes
                    .iter()
                    .any(|t| t.category == ThemeCategory::Custom);

                let q = theme_query.borrow().trim().to_lowercase();
                let cur_mode_f = mode_filter.get();
                let cur_cat_f = category_filter.get();

                let filtered: Vec<&ThemeSpec> = pack
                    .themes
                    .iter()
                    .filter(|t| match cur_mode_f {
                        ThemeModeFilter::All => true,
                        ThemeModeFilter::Dark => t.dark,
                        ThemeModeFilter::Light => !t.dark,
                    })
                    .filter(|t| cur_cat_f.is_none_or(|cat| t.category == cat))
                    .filter(|t| {
                        if q.is_empty() {
                            return true;
                        }
                        t.name.to_lowercase().contains(&q)
                            || t.id.to_lowercase().contains(&q)
                            || t.subtitle.to_lowercase().contains(&q)
                            || t.badge.to_lowercase().contains(&q)
                            || t.category.label().to_lowercase().contains(&q)
                    })
                    .collect();

                let mut grid_rows: Vec<AnyElement> = Vec::new();
                for pair in filtered.chunks(2) {
                    let first = render_theme_preview_card(
                        pair[0],
                        pair[0].id == active_id,
                        pair[0].id == dark_spec.id,
                        pair[0].id == light_spec.id,
                        workspace.clone(),
                        cx,
                    );
                    let second = if let Some(sec) = pair.get(1) {
                        render_theme_preview_card(
                            sec,
                            sec.id == active_id,
                            sec.id == dark_spec.id,
                            sec.id == light_spec.id,
                            workspace.clone(),
                            cx,
                        )
                    } else {
                        div().flex_1().min_w(px(0.0)).into_any_element()
                    };
                    grid_rows.push(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap_2p5()
                            .child(first)
                            .child(second)
                            .into_any_element(),
                    );
                }

                let ws_mode_dark = workspace.clone();
                let ws_mode_light = workspace.clone();
                let ws_tint = workspace.clone();
                let ws_import_theme = workspace.clone();
                let ws_reload_theme = workspace.clone();
                let themes_dir_open = paths.themes_dir.clone();

                let mut cat_options: Vec<(String, Option<ThemeCategory>, String)> = vec![(
                    "cat-all".to_string(),
                    None,
                    "全部分类".to_string(),
                )];
                for cat in ThemeCategory::BUILTIN_FILTERS {
                    cat_options.push((
                        format!("cat-{:?}", cat),
                        Some(cat),
                        cat.label().to_string(),
                    ));
                }
                if has_custom {
                    cat_options.push((
                        "cat-custom".to_string(),
                        Some(ThemeCategory::Custom),
                        ThemeCategory::Custom.label().to_string(),
                    ));
                }

                return dialog.child(
                    body.child(
                        v_flex()
                            .gap_2p5()
                            .max_h(px(480.0))
                            .w_full()
                            .min_w_0()
                            .pr_1()
                            .overflow_y_scrollbar()
                            .child(
                                macos_inset_group(cx)
                                    .flex_shrink_0()
                                    .child(
                                        h_flex()
                                            .px_3p5()
                                            .py_2p5()
                                            .justify_between()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(cx.theme().foreground)
                                                            .child("深浅双模式独立主题预设"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .child(
                                                                "分别记住深色与浅色主题，点击顶栏太阳/月亮一键无缝切换",
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        Button::new("slot-dark-btn")
                                                            .secondary()
                                                            .xsmall()
                                                            .icon(LucideIcon::Moon)
                                                            .selected(cur.dark_mode)
                                                            .label(format!(
                                                                "深色 · {}",
                                                                short_theme_name(&dark_spec.name)
                                                            ))
                                                            .on_click(move |_, window, cx| {
                                                                ws_mode_dark.update(
                                                                    cx,
                                                                    |ws, cx| {
                                                                        ws.set_dark_mode(
                                                                            true, window, cx,
                                                                        )
                                                                    },
                                                                );
                                                                window.refresh();
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("slot-light-btn")
                                                            .secondary()
                                                            .xsmall()
                                                            .icon(LucideIcon::Sun)
                                                            .selected(!cur.dark_mode)
                                                            .label(format!(
                                                                "浅色 · {}",
                                                                short_theme_name(&light_spec.name)
                                                            ))
                                                            .on_click(move |_, window, cx| {
                                                                ws_mode_light.update(
                                                                    cx,
                                                                    |ws, cx| {
                                                                        ws.set_dark_mode(
                                                                            false, window, cx,
                                                                        )
                                                                    },
                                                                );
                                                                window.refresh();
                                                            }),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .px_3p5()
                                            .py_2p5()
                                            .border_t_1()
                                            .border_color(cx.theme().border)
                                            .justify_between()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(cx.theme().foreground)
                                                            .child("界面材质随主题同步染色"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .child(
                                                                "Superlogical Rex 风格：顶栏、侧边栏、卡片与编辑器与终端浑然一体",
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("settings-tint-ui-theme").flex_shrink_0()
                                                    .checked(cur.tint_ui_with_theme)
                                                    .on_change(move |val, window, cx| {
                                                        ws_tint.update(cx, |ws, cx| {
                                                            ws.set_tint_ui_with_theme(
                                                                *val, window, cx,
                                                            );
                                                        });
                                                        window.refresh();
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_shrink_0()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div().min_w_0().flex_1().child(
                                                    Input::new(&theme_search_in)
                                                        .small()
                                                        .prefix(
                                                            Icon::new(LucideIcon::Search)
                                                                .xsmall()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                ),
                                                        )
                                                        .cleanable(true),
                                                ),
                                            )
                                            .child(
                                                h_flex().flex_shrink_0().gap_1().children(
                                                    [
                                                        (
                                                            "tm-all",
                                                            ThemeModeFilter::All,
                                                            format!("全部 ({total_cnt})"),
                                                        ),
                                                        (
                                                            "tm-dark",
                                                            ThemeModeFilter::Dark,
                                                            format!("深色 ({dark_cnt})"),
                                                        ),
                                                        (
                                                            "tm-light",
                                                            ThemeModeFilter::Light,
                                                            format!("浅色 ({light_cnt})"),
                                                        ),
                                                    ]
                                                    .into_iter()
                                                    .map(|(id, mf, lbl)| {
                                                        let mf_cell = mode_filter.clone();
                                                        Button::new(id)
                                                            .secondary()
                                                            .xsmall()
                                                            .tab_stop(false)
                                                            .selected(cur_mode_f == mf)
                                                            .label(lbl)
                                                            .on_click(move |_, window, _| {
                                                                mf_cell.set(mf);
                                                                window.refresh();
                                                            })
                                                    }),
                                                ),
                                            ),
                                    )
                                    .child(
                                        h_flex().flex_wrap().gap_1p5().children(
                                            cat_options.into_iter().map(|(id, cat, lbl)| {
                                                let cat_cell = category_filter.clone();
                                                Button::new(SharedString::from(id))
                                                    .secondary()
                                                    .xsmall()
                                                    .tab_stop(false)
                                                    .selected(cur_cat_f == cat)
                                                    .label(lbl)
                                                    .on_click(move |_, window, _| {
                                                        cat_cell.set(cat);
                                                        window.refresh();
                                                    })
                                            }),
                                        ),
                                    ),
                            )
                            .child(if grid_rows.is_empty() {
                                macos_inset_group(cx)
                                    .flex_shrink_0()
                                    .py_8()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("没有找到匹配的主题，请尝试更换搜索词或筛选条件"),
                                    )
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .flex_shrink_0()
                                    .gap_2p5()
                                    .children(grid_rows)
                                    .into_any_element()
                            })
                            .child(
                                macos_inset_group(cx)
                                    .flex_shrink_0()
                                    .px_3p5()
                                    .py_2p5()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(cx.theme().foreground)
                                                            .child("导入外部配色方案 (Ghostty / iTerm2)"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .child(format!(
                                                                "内置 {} 款精选主题 · 已导入 {} 个自定义主题（兼容 .ghostty 与 .itermcolors）",
                                                                pack.builtin_count,
                                                                pack.loaded_custom_count()
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        Button::new("settings-import-theme")
                                                            .secondary()
                                                            .xsmall()
                                                            .label("导入主题…")
                                                            .on_click(move |_, window, cx| {
                                                                ws_import_theme.update(
                                                                    cx,
                                                                    |ws, cx| {
                                                                        ws.import_custom_themes(
                                                                            window, cx,
                                                                        )
                                                                    },
                                                                );
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("settings-open-themes-dir")
                                                            .secondary()
                                                            .xsmall()
                                                            .label("打开目录")
                                                            .on_click(move |_, window, cx| {
                                                                if let Err(err) =
                                                                    open_directory_in_file_manager(
                                                                        &themes_dir_open,
                                                                    )
                                                                {
                                                                    window.push_notification(
                                                                        Notification::error(err),
                                                                        cx,
                                                                    );
                                                                }
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("settings-reload-themes")
                                                            .secondary()
                                                            .xsmall()
                                                            .label("重载")
                                                            .on_click(move |_, window, cx| {
                                                                ws_reload_theme.update(
                                                                    cx,
                                                                    |ws, cx| {
                                                                        ws.reload_custom_themes(
                                                                            window, cx,
                                                                        )
                                                                    },
                                                                );
                                                            }),
                                                    ),
                                            ),
                                    )
                                    .children((!pack.custom_entries.is_empty()).then(|| {
                                        v_flex()
                                            .pt_1()
                                            .gap_1p5()
                                            .border_t_1()
                                            .border_color(cx.theme().border)
                                            .children(pack.custom_entries.iter().map(|entry| {
                                                let fname = entry.file_name.clone();
                                                let ws_rm = workspace.clone();
                                                let has_err = entry.error.is_some();
                                                let subtitle = if let Some(ref err) = entry.error {
                                                    err.clone()
                                                } else {
                                                    let tname = entry
                                                        .theme_name
                                                        .as_deref()
                                                        .unwrap_or("自定义主题");
                                                    let mode_lbl = if entry.dark.unwrap_or(true) {
                                                        "深色"
                                                    } else {
                                                        "浅色"
                                                    };
                                                    format!("{tname} ({mode_lbl})")
                                                };
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        v_flex()
                                                            .min_w_0()
                                                            .flex_1()
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        FontWeight::MEDIUM,
                                                                    )
                                                                    .text_color(
                                                                        cx.theme().foreground,
                                                                    )
                                                                    .child(
                                                                        entry.file_name.clone(),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .text_xs()
                                                                    .text_color(if has_err {
                                                                        cx.theme().danger
                                                                    } else {
                                                                        cx.theme().muted_foreground
                                                                    })
                                                                    .child(subtitle),
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new(SharedString::from(format!(
                                                            "rm-theme-{}",
                                                            entry.file_name
                                                        )))
                                                        .flex_shrink_0()
                                                        .ghost()
                                                        .xsmall()
                                                        .compact()
                                                        .icon(LucideIcon::Trash)
                                                        .tooltip("移除该主题文件")
                                                        .on_click(move |_, window, cx| {
                                                            ws_rm.update(cx, |ws, cx| {
                                                                ws.remove_custom_theme(
                                                                    &fname, window, cx,
                                                                );
                                                            });
                                                        }),
                                                    )
                                            }))
                                    })),
                            ),
                    ),
                );
            }
            SettingsTab::General => {}
        }
        let cur = workspace.read(cx).settings.clone();
        let ws_dark = workspace.clone();
        let ws_fminus = workspace.clone();
        let ws_fplus = workspace.clone();
        let ws_follow = workspace.clone();
        let ws_copy_on_select = workspace.clone();
        let ws_cmd_blocks = workspace.clone();
        let ws_sem_hl = workspace.clone();
        let ws_code_hl = workspace.clone();
        let ws_overview = workspace.clone();
        let ws_syn_import = workspace.clone();
        let ws_syn_reload = workspace.clone();
        let syn_pack = SyntaxRegistry::global().current();
        let sys_mono = system_monospace_font_family();
        let sys_preset_family = if TERMINAL_FONT_PRESETS
            .iter()
            .any(|p| p.family.eq_ignore_ascii_case(sys_mono))
        {
            "Consolas"
        } else {
            sys_mono
        };
        let palette = TerminalPalette::for_theme_id(cur.active_theme_id(), cur.dark_mode);
        let preview_font = TerminalFont::resolve(&font_in.read(cx).value(), palette.dark, || {
            cx.text_system().all_font_names()
        });
        let preview = terminal_font_preview(&preview_font, cur.font_size, &palette, window, cx);
        let font_presets = TERMINAL_FONT_PRESETS
            .iter()
            .map(|p| (p.family, SharedString::from(p.label)))
            .chain(std::iter::once((
                sys_preset_family,
                format!("系统等宽 ({sys_preset_family})").into(),
            )))
            .enumerate()
            .map(|(index, (family, label))| {
                let input = font_in.clone();
                let ws = workspace.clone();
                Button::new(("font-preset", index))
                    .font_family(family)
                    .secondary()
                    .xsmall()
                    .selected(preview_font.font.family == family)
                    .label(label)
                    .on_click(move |_, window, cx| {
                        input.update(cx, |st, cx| st.set_value(family, window, cx));
                        ws.update(cx, |ws, cx| ws.set_font_family(family.to_string(), cx));
                        window.refresh();
                    })
            });

        dialog.child(body.child(
            v_flex()
                .gap_3()
                .max_h(px(440.0))
                .w_full()
                .min_w_0()
                .pr_1()
                .overflow_y_scrollbar()
                .child(
                    macos_inset_group(cx)
                        .flex_shrink_0()
                        .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("深色外观模式"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("同步切换 macOS 窗口材质与终端配色"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("settings-dark-mode").flex_shrink_0()
                                            .checked(cur.dark_mode)
                                            .on_change(move |val, window, cx| {
                                                ws_dark.update(cx, |ws, cx| {
                                                    ws.set_dark_mode(*val, window, cx);
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("默认终端字号"),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_shrink_0()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                Button::new("settings-font-dec")
                                                    .secondary()
                                                    .xsmall()
                                                    .label("-")
                                                    .on_click(move |_, window, cx| {
                                                        ws_fminus.update(cx, |ws, cx| {
                                                            ws.adjust_default_font_size(-1.0, cx)
                                                        });
                                                        window.refresh();
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .w(px(48.0))
                                                    .text_xs()
                                                    .text_color(cx.theme().foreground)
                                                    .child(format!("{} px", cur.font_size as i32)),
                                            )
                                            .child(
                                                Button::new("settings-font-inc")
                                                    .secondary()
                                                    .xsmall()
                                                    .label("+")
                                                    .on_click(move |_, window, cx| {
                                                        ws_fplus.update(cx, |ws, cx| {
                                                            ws.adjust_default_font_size(1.0, cx)
                                                        });
                                                        window.refresh();
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .gap_2()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child(field_block(
                                        "终端等宽字体族（中文与图标自动回退内置更纱黑体）",
                                        Input::new(&font_in).small(),
                                        cx,
                                    ))
                                    .child(h_flex().min_w_0().flex_wrap().gap_1p5().children(font_presets))
                                    .child(preview),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("回滚缓存行数"),
                                    )
                                    .child(h_flex().flex_shrink_0().gap_1().children(
                                        [1500usize, 5000, 10000, 30000].into_iter().map(|lines| {
                                            let ws_sb = workspace.clone();
                                            Button::new(SharedString::from(format!("sb-{lines}")))
                                                .secondary()
                                                .xsmall()
                                                .selected(cur.scrollback_lines == lines)
                                                .label(format!("{lines}"))
                                                .on_click(move |_, window, cx| {
                                                    ws_sb.update(cx, |ws, cx| {
                                                        ws.set_scrollback_lines(lines, cx)
                                                    });
                                                    window.refresh();
                                                })
                                        }),
                                    )),
                            ),
                    )
                    .child(
                        macos_inset_group(cx)
                            .flex_shrink_0()
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("OSC 133 命令块分割"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("命令间显示极淡分隔细线，报错/执行中在起始行标注状态"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("settings-command-blocks").flex_shrink_0()
                                            .checked(cur.command_blocks)
                                            .on_change(move |val, window, cx| {
                                                ws_cmd_blocks.update(cx, |ws, cx| {
                                                    ws.set_command_blocks(*val, cx)
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .child(
                                h_flex().px_3p5().py_2p5().border_t_1().border_color(cx.theme().border)
                                    .justify_between().items_center().gap_2()
                                    .child(div().min_w_0().flex_1().text_sm().font_weight(FontWeight::MEDIUM).child("终端右键动作"))
                                    .child(h_flex().flex_shrink_0().gap_1().children([
                                        ("right-click-menu", RightClickAction::ContextMenu, "弹出菜单"),
                                        ("right-click-smart", RightClickAction::SmartCopyPaste, "智能复制粘贴"),
                                    ].into_iter().map(|(id, action, label)| {
                                        let ws = workspace.clone();
                                        Button::new(id).secondary().xsmall().tab_stop(false)
                                            .selected(cur.right_click_action == action).label(label)
                                            .on_click(move |_, window, cx| {
                                                ws.update(cx, |ws, cx| ws.set_right_click_action(action, cx));
                                                window.refresh();
                                            })
                                    }))),
                            )
                            .child(
                                h_flex().px_3p5().py_2p5().border_t_1().border_color(cx.theme().border)
                                    .justify_between().items_center().gap_2()
                                    .child(div().min_w_0().flex_1().text_sm().font_weight(FontWeight::MEDIUM).child("选中文本自动复制"))
                                    .child(Switch::new("copy-on-select").flex_shrink_0().checked(cur.copy_on_select)
                                        .on_change(move |enabled, window, cx| {
                                            ws_copy_on_select.update(cx, |ws, cx| ws.set_copy_on_select(*enabled, cx));
                                            window.refresh();
                                        })),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("语义输出与命令行高亮"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("高亮路径、URL (Ctrl+点击打开) 与命令参数"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("settings-semantic-hl").flex_shrink_0()
                                            .checked(cur.semantic_highlighting)
                                            .on_change(move |val, window, cx| {
                                                ws_sem_hl.update(cx, |ws, cx| {
                                                    ws.set_semantic_highlighting(*val, cx)
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("终端输出代码语法高亮"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("对 cat / head / tail / diff 等纯文本输出自动多语言着色"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("settings-code-hl").flex_shrink_0()
                                            .checked(cur.code_highlighting)
                                            .on_change(move |val, window, cx| {
                                                ws_code_hl.update(cx, |ws, cx| {
                                                    ws.set_code_highlighting(*val, cx)
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("滚动条全局概览标记"),
                                    )
                                    .child(
                                        Switch::new("settings-overview-markers").flex_shrink_0()
                                            .checked(cur.overview_markers)
                                            .on_change(move |val, window, cx| {
                                                ws_overview.update(cx, |ws, cx| {
                                                    ws.set_overview_markers(*val, cx)
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("SFTP 默认跟随终端目录"),
                                    )
                                    .child(
                                        Switch::new("settings-follow-cwd").flex_shrink_0()
                                            .checked(cur.follow_sftp_cwd)
                                            .on_change(move |val, window, cx| {
                                                ws_follow.update(cx, |ws, cx| {
                                                    ws.set_follow_sftp_cwd(*val, cx)
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        macos_inset_group(cx)
                            .flex_shrink_0()
                            .px_3p5()
                            .py_2p5()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("多语言语法拓展库"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!(
                                                        "内置 {} 种语言 · 已生效 {} 个外部拓展（支持 .sublime-syntax / 简版 .tmLanguage.json）",
                                                        syn_pack.builtin_count,
                                                        syn_pack.loaded_custom_count()
                                                    )),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_shrink_0()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                Button::new("settings-import-syntax")
                                                    .secondary()
                                                    .xsmall()
                                                    .label("导入语法…")
                                                    .on_click(move |_, window, cx| {
                                                        ws_syn_import.update(cx, |ws, cx| {
                                                            ws.import_syntax_extensions(window, cx)
                                                        });
                                                    }),
                                            )
                                            .child({
                                                let syn_dir_open = paths.syntaxes_dir.clone();
                                                Button::new("settings-open-syntaxes-dir")
                                                    .secondary()
                                                    .xsmall()
                                                    .label("打开目录")
                                                    .on_click(move |_, window, cx| {
                                                        if let Err(err) = open_directory_in_file_manager(&syn_dir_open) {
                                                            window.push_notification(Notification::error(err), cx);
                                                        }
                                                    })
                                            })
                                            .child(
                                                Button::new("settings-reload-syntaxes")
                                                    .secondary()
                                                    .xsmall()
                                                    .label("重载")
                                                    .on_click(move |_, window, cx| {
                                                        ws_syn_reload.update(cx, |ws, cx| {
                                                            ws.reload_syntax_extensions(window, cx)
                                                        });
                                                    }),
                                            ),
                                    ),
                            )
                            .children(
                                (!syn_pack.custom_entries.is_empty()).then(|| {
                                    v_flex()
                                        .pt_1()
                                        .gap_1p5()
                                        .border_t_1()
                                        .border_color(cx.theme().border)
                                        .children(syn_pack.custom_entries.iter().map(|entry| {
                                            let fname = entry.file_name.clone();
                                            let ws_rm = workspace.clone();
                                            let subtitle = if let Some(ref err) = entry.error {
                                                err.clone()
                                            } else {
                                                let syn_label = entry
                                                    .syntax_name
                                                    .as_deref()
                                                    .unwrap_or("未知语法");
                                                let exts = if entry.extensions.is_empty() {
                                                    "按首行匹配".to_string()
                                                } else {
                                                    entry
                                                        .extensions
                                                        .iter()
                                                        .map(|e| format!(".{e}"))
                                                        .collect::<Vec<_>>()
                                                        .join(", ")
                                                };
                                                format!("{syn_label} ({exts})")
                                            };
                                            let has_err = entry.error.is_some();
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .gap_2()
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
                                                                .child(entry.file_name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .truncate()
                                                                .text_xs()
                                                                .text_color(if has_err {
                                                                    cx.theme().danger
                                                                } else {
                                                                    cx.theme().muted_foreground
                                                                })
                                                                .child(subtitle),
                                                        ),
                                                )
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "rm-syn-{}",
                                                        entry.file_name
                                                    )))
                                                    .flex_shrink_0()
                                                    .ghost()
                                                    .xsmall()
                                                    .compact()
                                                    .icon(LucideIcon::Trash)
                                                    .tooltip("移除该语法文件")
                                                    .on_click(move |_, window, cx| {
                                                        ws_rm.update(cx, |ws, cx| {
                                                            ws.remove_syntax_extension(
                                                                &fname, window, cx,
                                                            );
                                                        });
                                                    }),
                                                )
                                        }))
                                }),
                            ),
                    )
                    .child(
                        macos_inset_group(cx)
                            .flex_shrink_0()
                            .child(
                                h_flex()
                                    .px_3p5()
                                    .py_2p5()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("开启调试日志"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("默认关闭；开启后在顶栏显示「日志」按钮，并展开运行与诊断日志面板"),
                                            ),
                                    )
                                    .child({
                                        let ws = workspace.clone();
                                        Switch::new("settings-debug-logging")
                                            .flex_shrink_0()
                                            .accessibility_label("开启调试日志")
                                            .checked(cur.debug_logging)
                                            .on_change(move |enabled, window, cx| {
                                                if let Err(err) = ws.update(cx, |ws, cx| ws.set_debug_logging(*enabled, cx)) {
                                                    window.push_notification(Notification::error(format!("更新调试日志设置失败：{err:#}")), cx);
                                                }
                                                window.refresh();
                                            })
                                    }),
                            )
                            .children(cur.debug_logging.then(|| {
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .px_3p5()
                                    .py_2p5()
                                    .gap_1p5()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .justify_between()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .truncate()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Mica Term GPUI v0.2.0 · 运行与诊断日志"),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child({
                                                        let paths_open = paths.clone();
                                                        Button::new("settings-open-logs-dir")
                                                            .secondary()
                                                            .xsmall()
                                                            .label("打开日志目录")
                                                            .on_click(move |_, window, cx| {
                                                                if let Err(err) = open_directory_in_file_manager(&paths_open.logs_dir) {
                                                                    window.push_notification(Notification::error(err), cx);
                                                                }
                                                            })
                                                    })
                                                    .child({
                                                        let paths_view = paths.clone();
                                                        Button::new("settings-view-logs")
                                                            .secondary()
                                                            .xsmall()
                                                            .label("查看运行日志")
                                                            .on_click(move |_, window, cx| {
                                                                window.close_dialog(cx);
                                                                open_logs_modal(
                                                                    paths_view.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            })
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("数据与凭据目录：{data_dir_display}")),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("运行与崩溃日志：{logs_dir_display}")),
                                    )
                            })),
                    ),
        ))
    });
}

fn terminal_font_preview(
    font: &TerminalFont,
    size: f32,
    palette: &TerminalPalette,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let metrics = font.measure(size, window.text_system());
    let height = metrics.cell_h * 2.0;
    let regular_font = font.font.clone();
    let mut bold_font = regular_font.clone();
    bold_font.weight = font.tuning.bold_weight;
    let fg = rgb_to_hsla(palette.foreground, 1.0);
    let prompt = rgb_to_hsla(palette.ansi[2], 1.0);
    let path_fg = rgb_to_hsla(palette.ansi[6], 1.0);
    div()
        .w_full()
        .min_w_0()
        .px_2p5()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .bg(rgb_to_hsla(palette.background, 1.0))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    for (row, col, sample, wide, bold, color) in [
                        (0, 0, "mica@prod", false, true, prompt),
                        (0, 10, "~", false, false, path_fg),
                        (0, 12, "| 0O 1lI {} [] != ->", false, false, fg),
                        (1, 0, "中文等宽对齐", true, false, fg),
                        (1, 12, "| 200 OK  1234567890", false, false, fg),
                    ] {
                        let run = TextRun {
                            len: sample.len(),
                            font: if bold {
                                bold_font.clone()
                            } else {
                                regular_font.clone()
                            },
                            color,
                            ..Default::default()
                        };
                        let line = window.text_system().shape_line(
                            sample.into(),
                            metrics.font_size,
                            &[run],
                            Some(metrics.advance(wide)),
                        );
                        let origin = metrics.text_origin(
                            bounds.origin.x + metrics.cell_w * col as f32,
                            bounds.origin.y + metrics.cell_h * row as f32,
                            wide,
                        );
                        let _ =
                            line.paint(origin, metrics.cell_h, TextAlign::Left, None, window, cx);
                    }
                },
            )
            .w_full()
            .h(height),
        )
}

pub fn open_logs_modal(paths: AppPaths, window: &mut Window, cx: &mut App) {
    let initial_text = read_recent_logs(&paths, 48 * 1024);
    let log_area = cx.new(|cx| {
        let mut s = TextareaState::new(window, cx);
        s.set_value(initial_text, window, cx);
        s
    });
    let logs_dir_str = paths.logs_dir.display().to_string();

    window.open_dialog(cx, move |dialog, _, cx| {
        let area_view = log_area.clone();
        let area_refresh = log_area.clone();
        let area_copy = log_area.clone();
        let paths_refresh = paths.clone();
        let paths_dir = paths.clone();

        dialog
            .title("运行与崩溃诊断日志")
            .w(px(680.0))
            .child(
                dialog_body()
                    .gap_2p5()
                    .child(
                        macos_inset_group(cx)
                            .px_3()
                            .py_2()
                            .gap_0p5()
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().foreground)
                                    .child(format!("日志文件夹：{logs_dir_str}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "提示：mica-term.log 记录实时运行与 SSH 握手日志；crash.log 记录异常与底层崩溃堆栈。命令行附加 --console 可实时输出。",
                                    ),
                            ),
                    )
                    .child(Textarea::new(&area_view).h(px(340.0))),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("logs-refresh-btn")
                            .secondary()
                            .small()
                            .label("刷新")
                            .on_click(move |_, window, cx| {
                                let fresh = read_recent_logs(&paths_refresh, 48 * 1024);
                                area_refresh.update(cx, |st, cx| st.set_value(fresh, window, cx));
                                window.refresh();
                            }),
                    )
                    .child(
                        Button::new("logs-copy-btn")
                            .secondary()
                            .small()
                            .label("复制日志")
                            .on_click(move |_, _, cx| {
                                let content = area_copy.read(cx).value().to_string();
                                cx.write_to_clipboard(ClipboardItem::new_string(content));
                            }),
                    )
                    .child(
                        Button::new("logs-open-dir-btn")
                            .secondary()
                            .small()
                            .label("打开日志目录")
                            .on_click(move |_, window, cx| {
                                if let Err(err) = open_directory_in_file_manager(&paths_dir.logs_dir) {
                                    window.push_notification(Notification::error(err), cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("logs-close-btn")
                            .primary()
                            .small()
                            .label("关闭")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}
