//! Clean GPUI Kit Dialog modals for SSH Hosts, Folders, Keychain, Snippets, SFTP Editor, and Settings.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
    h_flex,
    input::{Input, InputState, Textarea, TextareaState},
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, FontWeight, IntoElement, ParentElement as _,
    SharedString, Styled as _, Window, div, px,
};

use crate::domain::{
    HostNode, HostNodeKind, KeychainEntry, KeychainKind, SecretPayload, SnippetItem, SshAuthConfig,
    SshHostConfig, SshProxyConfig,
};
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
            .child(v_flex().gap_3().child(field_block(label, Input::new(&input).small(), cx)))
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

pub fn open_remote_editor_dialog(
    file_name: &str,
    remote_path: &str,
    initial_content: &str,
    window: &mut Window,
    cx: &mut App,
    on_save: impl Fn(String, &mut Window, &mut App) + 'static,
) {
    let text_init = initial_content.to_string();
    let editor = cx.new(|cx| {
        let mut st = TextareaState::new(window, cx);
        st.set_value(text_init, window, cx);
        st
    });
    let title_str = format!("编辑远程文件 · {file_name}");
    let path_str = remote_path.to_string();
    let save_cb = Rc::new(on_save);

    window.open_dialog(cx, move |dialog, _, cx| {
        let ed_save = editor.clone();
        let cb = save_cb.clone();

        dialog
            .title(title_str.clone())
            .w(px(680.0))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(path_str.clone()),
                    )
                    .child(
                        div()
                            .h(px(380.0))
                            .child(Textarea::new(&editor).h(px(372.0))),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("remote-edit-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("remote-edit-save")
                            .primary()
                            .small()
                            .label("保存到服务器")
                            .on_click(move |_, window, cx| {
                                let content = ed_save.read(cx).value().to_string();
                                cb(content, window, cx);
                                window.close_dialog(cx);
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

pub fn open_ssh_host_modal(
    workspace: Entity<MicaWorkspace>,
    editing_node: Option<HostNode>,
    existing_secrets: SecretPayload,
    default_parent_id: Option<String>,
    folders: Vec<(String, String)>,
    jump_candidates: Vec<(String, String)>,
    keychains: Vec<KeychainEntry>,
    window: &mut Window,
    cx: &mut App,
) {
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
        let mut s = InputState::new(window, cx).placeholder("~/.ssh/id_ed25519");
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
                    .children(folders.iter().cloned().map(|(fid, flabel)| {
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
                        h_flex().flex_wrap().gap_1().children(keychains.iter().map(|kc| {
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
                        .children(jump_candidates.iter().cloned().map(|(jid, jname)| {
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
                    DraftAuthTab::KeyPath => SshAuthConfig::PrivateKeyPath {
                        path: key_path_in.read(cx).value().trim().to_string(),
                    },
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
                    DraftProxyTab::Socks5 => SshProxyConfig::Socks5 {
                        host: px_host_in.read(cx).value().trim().to_string(),
                        port: px_port_in.read(cx).value().trim().parse().unwrap_or(1080),
                        username: Some(px_user_in.read(cx).value().trim().to_string())
                            .filter(|s| !s.is_empty()),
                    },
                    DraftProxyTab::Http => SshProxyConfig::Http {
                        host: px_host_in.read(cx).value().trim().to_string(),
                        port: px_port_in.read(cx).value().trim().parse().unwrap_or(8080),
                        username: Some(px_user_in.read(cx).value().trim().to_string())
                            .filter(|s| !s.is_empty()),
                    },
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
                    kind: HostNodeKind::Host(SshHostConfig {
                        host: host_val,
                        port: port_val,
                        user: user_val,
                        auth,
                        proxy,
                        default_remote_dir: dir_in.read(cx).value().trim().to_string(),
                        environment_tag: env_in.read(cx).value().trim().to_string(),
                        remark: String::new(),
                    }),
                };

                let saved_id = node.id.clone();
                window.close_dialog(cx);
                ws.update(cx, |ws, cx| {
                    ws.save_host_node(node, secrets, cx);
                    if connect_now {
                        ws.connect_host_in_new_tab(&saved_id, window, cx);
                    }
                });
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
            .w(px(520.0))
            .child(
                v_flex()
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
                    .children(
                        folder_selector.map(|fs| field_block("所属分组文件夹", fs, cx)),
                    )
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
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("auth-tab-pwd")
                                            .ghost()
                                            .xsmall()
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
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("px-tab-none")
                                            .ghost()
                                            .xsmall()
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
                    .child(field_block(
                        "SFTP 初始远程路径（选填）",
                        Input::new(&dir_in).small(),
                        cx,
                    )),
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
    let pwd_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("登录密码")
            .masked(true);
        if let Some(p) = existing_secrets.password {
            s.set_value(p, window, cx);
        }
        s
    });
    let pem_in = cx.new(|cx| {
        let mut s = TextareaState::new(window, cx);
        if let Some(pem) = existing_secrets.private_key_pem {
            s.set_value(pem, window, cx);
        }
        s
    });
    let pp_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx)
            .placeholder("私钥密码短语（选填）")
            .masked(true);
        if let Some(pp) = existing_secrets.passphrase {
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
                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("kc-kind-pwd")
                                    .ghost()
                                    .xsmall()
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
                    }),
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
                                    remark: String::new(),
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
                                ws.update(cx, |ws, cx| ws.save_keychain(entry, payload, cx));
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
                v_flex()
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

pub fn open_settings_modal(
    workspace: Entity<MicaWorkspace>,
    data_dir_display: String,
    window: &mut Window,
    cx: &mut App,
) {
    let init_settings = workspace.read(cx).settings.clone();
    let font_in = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder("DejaVu Sans Mono");
        s.set_value(init_settings.font_family.clone(), window, cx);
        s
    });

    window.open_dialog(cx, move |dialog, _, cx| {
        let cur = workspace.read(cx).settings.clone();
        let ws_dark = workspace.clone();
        let ws_fminus = workspace.clone();
        let ws_fplus = workspace.clone();
        let ws_follow = workspace.clone();
        let ws_apply_font = workspace.clone();
        let f_in = font_in.clone();

        dialog
            .title("设置与外观")
            .w(px(460.0))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("深色主题模式"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("同步切换应用外壳与终端配色方案"),
                                    ),
                            )
                            .child(
                                Switch::new("settings-dark-mode")
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
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().foreground)
                                    .child("默认终端字号"),
                            )
                            .child(
                                h_flex()
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
                    .child(field_block(
                        "终端等宽字体族",
                        Input::new(&font_in).small(),
                        cx,
                    ))
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("终端回滚缓存行数"),
                            )
                            .child(h_flex().gap_1p5().children(
                                [1500usize, 5000, 10000, 30000].into_iter().map(|lines| {
                                    let ws_sb = workspace.clone();
                                    Button::new(SharedString::from(format!("sb-{lines}")))
                                        .secondary()
                                        .xsmall()
                                        .selected(cur.scrollback_lines == lines)
                                        .label(format!("{lines} 行"))
                                        .on_click(move |_, window, cx| {
                                            ws_sb.update(cx, |ws, cx| {
                                                ws.set_scrollback_lines(lines, cx)
                                            });
                                            window.refresh();
                                        })
                                }),
                            )),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().foreground)
                                    .child("SFTP 默认跟随终端目录"),
                            )
                            .child(
                                Switch::new("settings-follow-cwd")
                                    .checked(cur.follow_sftp_cwd)
                                    .on_change(move |val, window, cx| {
                                        ws_follow.update(cx, |ws, cx| {
                                            ws.set_follow_sftp_cwd(*val, cx)
                                        });
                                        window.refresh();
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .p_2p5()
                            .gap_1()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().secondary.opacity(0.45))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Mica Term GPUI v0.2.0"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("本地数据目录：{data_dir_display}")),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("settings-done")
                        .primary()
                        .small()
                        .label("完成")
                        .on_click(move |_, window, cx| {
                            let fam = f_in.read(cx).value().trim().to_string();
                            ws_apply_font.update(cx, |ws, cx| ws.set_font_family(fam, cx));
                            window.close_dialog(cx);
                        }),
                ),
            )
    });
}
