//! Clean Quick-Launch Welcome View shown when no session tabs are open.

use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

#[derive(Clone)]
pub struct QuickConnectItem {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub env_tag: String,
}

#[derive(IntoElement)]
pub struct WelcomeView {
    quick_hosts: Vec<QuickConnectItem>,
    on_open_local: Rc<dyn Fn(&mut Window, &mut App)>,
    on_new_ssh: Rc<dyn Fn(&mut Window, &mut App)>,
    on_connect_host: Rc<dyn Fn(&str, &mut Window, &mut App)>,
}

impl WelcomeView {
    pub fn new(
        quick_hosts: Vec<QuickConnectItem>,
        on_open_local: impl Fn(&mut Window, &mut App) + 'static,
        on_new_ssh: impl Fn(&mut Window, &mut App) + 'static,
        on_connect_host: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            quick_hosts,
            on_open_local: Rc::new(on_open_local),
            on_new_ssh: Rc::new(on_new_ssh),
            on_connect_host: Rc::new(on_connect_host),
        }
    }
}

impl RenderOnce for WelcomeView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_local = self.on_open_local.clone();
        let on_ssh = self.on_new_ssh.clone();
        let on_connect = self.on_connect_host.clone();

        div()
            .size_full()
            .overflow_y_scrollbar()
            .bg(cx.theme().background)
            .flex()
            .justify_center()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(720.0))
                    .px_8()
                    .py_12()
                    .gap_8()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_2p5()
                                    .items_center()
                                    .child(
                                        Icon::new(LucideIcon::Terminal)
                                            .large()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Mica Term"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("专注、轻量、GPU 加速的本地终端与远程服务器工作空间"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .child(
                                div()
                                    .id("welcome-open-local-card")
                                    .flex_1()
                                    .p_4()
                                    .rounded(cx.theme().radius_lg)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary.opacity(0.45))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(cx.theme().primary).bg(cx.theme().secondary))
                                    .on_click(move |_, window, cx| on_local(window, cx))
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(LucideIcon::Terminal)
                                                            .text_color(cx.theme().primary),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child("打开本地终端"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("启动本机默认 Shell (PTY)，支持多标签与分屏"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("welcome-new-ssh-card")
                                    .flex_1()
                                    .p_4()
                                    .rounded(cx.theme().radius_lg)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary.opacity(0.45))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(cx.theme().primary).bg(cx.theme().secondary))
                                    .on_click(move |_, window, cx| on_ssh(window, cx))
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(LucideIcon::Server)
                                                            .text_color(cx.theme().primary),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child("新建 SSH 连接"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("支持密码、私钥、跳板机与联动 SFTP 文件浏览"),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("快速连接主机"),
                            )
                            .child(if self.quick_hosts.is_empty() {
                                div()
                                    .p_5()
                                    .rounded(cx.theme().radius_lg)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("暂无已保存的 SSH 主机。点击上方「新建 SSH 连接」或左侧栏「+」添加你的第一台服务器。")
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .gap_2()
                                    .children(self.quick_hosts.into_iter().map(|item| {
                                        let host_id = item.id.clone();
                                        let cb = on_connect.clone();
                                        h_flex()
                                            .id(SharedString::from(format!("quick-host-{}", item.id)))
                                            .px_3p5()
                                            .py_2p5()
                                            .justify_between()
                                            .items_center()
                                            .rounded(cx.theme().radius)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(cx.theme().list_hover))
                                            .on_click(move |_, window, cx| cb(&host_id, window, cx))
                                            .child(
                                                h_flex()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(LucideIcon::Server)
                                                            .small()
                                                            .text_color(cx.theme().muted_foreground),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .gap_0p5()
                                                            .child(
                                                                h_flex()
                                                                    .gap_2()
                                                                    .items_center()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(cx.theme().foreground)
                                                                            .child(item.name),
                                                                    )
                                                                    .children((!item.env_tag.is_empty()).then(|| {
                                                                        div()
                                                                            .px_1p5()
                                                                            .py_0p5()
                                                                            .rounded(cx.theme().radius)
                                                                            .bg(cx.theme().secondary)
                                                                            .text_xs()
                                                                            .text_color(cx.theme().secondary_foreground)
                                                                            .child(item.env_tag)
                                                                    })),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child(item.endpoint),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Button::new(SharedString::from(format!(
                                                    "quick-btn-{}",
                                                    item.id
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .label("连接"),
                                            )
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
    }
}
