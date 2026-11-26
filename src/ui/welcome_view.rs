//! macOS Sonoma / Sequoia Quick-Launch Welcome View:
//! Squircle hero header, Cupertino action cards, and System Settings Inset Grouped host list.

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
    AnyElement, App, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::ui::brand::brand_app_icon_element;
use crate::ui::theme::{macos_env_badge_colors, macos_inset_group};

type WindowActionFn = Rc<dyn Fn(&mut Window, &mut App)>;
type HostConnectFn = Rc<dyn Fn(&str, &mut Window, &mut App)>;

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
    on_open_local: WindowActionFn,
    on_new_ssh: WindowActionFn,
    on_connect_host: HostConnectFn,
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

#[inline(never)]
fn render_action_card(
    id: &'static str,
    icon: LucideIcon,
    tint: gpui_kit::Hsla,
    title: &'static str,
    subtitle: &'static str,
    action: WindowActionFn,
    cx: &App,
) -> AnyElement {
    macos_inset_group(cx)
        .flex_1()
        .child(
            h_flex()
                .id(id)
                .p_4()
                .gap_3p5()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().list_hover))
                .on_click(move |_, window, cx| action(window, cx))
                .child(
                    h_flex()
                        .min_w_0()
                        .flex_1()
                        .gap_3p5()
                        .items_center()
                        .child(Icon::new(icon).size(px(22.)).text_color(tint))
                        .child(
                            v_flex()
                                .min_w_0()
                                .flex_1()
                                .gap_0p5()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_sm()
                                        .text_color(cx.theme().foreground)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(subtitle),
                                ),
                        ),
                )
                .child(
                    Icon::new(LucideIcon::ChevronRight)
                        .small()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .into_any_element()
}

#[inline(never)]
fn render_quick_host_row(
    item: QuickConnectItem,
    show_divider: bool,
    on_connect: HostConnectFn,
    cx: &App,
) -> AnyElement {
    let host_id = item.id.clone();
    let host_id_btn = item.id.clone();
    let cb = on_connect.clone();
    let cb_btn = on_connect;

    let env_pill = (!item.env_tag.is_empty()).then(|| {
        let (bg, fg) = macos_env_badge_colors(&item.env_tag, cx);
        div()
            .px_1p5()
            .py_0p5()
            .rounded(px(5.0))
            .bg(bg)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(fg)
            .child(item.env_tag)
    });

    h_flex()
        .id(SharedString::from(format!("quick-host-{}", item.id)))
        .px_3p5()
        .py_2p5()
        .justify_between()
        .items_center()
        .when(show_divider, |el| {
            el.border_t_1().border_color(cx.theme().border)
        })
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().list_hover))
        .on_click(move |_, window, cx| cb(&host_id, window, cx))
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .gap_3()
                .items_center()
                .child(
                    Icon::new(LucideIcon::Server)
                        .size(px(16.))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .flex_1()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(cx.theme().foreground)
                                        .child(item.name),
                                )
                                .children(env_pill),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(item.endpoint),
                        ),
                ),
        )
        .child(
            Button::new(SharedString::from(format!("quick-btn-{}", item.id)))
                .tab_stop(false)
                .secondary()
                .xsmall()
                .label("连接")
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    cb_btn(&host_id_btn, window, cx);
                }),
        )
        .into_any_element()
}

impl RenderOnce for WelcomeView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let local_card = render_action_card(
            "welcome-open-local-card",
            LucideIcon::Terminal,
            cx.theme().primary,
            "本地终端",
            "打开本机默认 Shell",
            self.on_open_local,
            cx,
        );
        let ssh_card = render_action_card(
            "welcome-new-ssh-card",
            LucideIcon::Server,
            cx.theme().success,
            "新建 SSH 连接",
            "连接远程主机并浏览文件",
            self.on_new_ssh,
            cx,
        );

        let quick_section = if self.quick_hosts.is_empty() {
            macos_inset_group(cx)
                .p_6()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("暂无已保存的服务器。点击上方「新建 SSH 连接」或左侧栏「+」添加你的第一台主机。"),
                )
                .into_any_element()
        } else {
            let on_connect = self.on_connect_host;
            let rows: Vec<AnyElement> = self
                .quick_hosts
                .into_iter()
                .enumerate()
                .map(|(idx, item)| render_quick_host_row(item, idx > 0, on_connect.clone(), cx))
                .collect();
            v_flex().children(rows).into_any_element()
        };

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
                    .py(px(56.0))
                    .gap_8()
                    .child(
                        h_flex()
                            .gap_3p5()
                            .items_center()
                            .child(brand_app_icon_element(48.0))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Mica Term"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("本地终端与远程 SSH 工作区"),
                                    ),
                            ),
                    )
                    .child(h_flex().gap_3p5().child(local_card).child(ssh_card))
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .px_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("最近与常用主机"),
                            )
                            .child(quick_section),
                    ),
            )
    }
}
