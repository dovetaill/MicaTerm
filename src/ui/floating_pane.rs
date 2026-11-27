//! In-workspace floating terminal pane geometry, edge/corner resize math,
//! and zero-background-cost GPU stardust particle border decoration.

use std::time::Instant;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, BoxShadow, Context, Corners, CursorStyle, Entity, Hsla, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, Size, Styled as _, Window, canvas, div, fill, point, prelude::FluentBuilder as _,
    px, size,
};

pub const FLOATING_COLLAPSED_HEIGHT: f32 = 30.0;
pub const FLOATING_MIN_WIDTH: f32 = 280.0;
pub const FLOATING_MIN_HEIGHT: f32 = 160.0;
pub const FLOATING_STAGE_MARGIN: f32 = 8.0;
const PARTICLE_HALO_INSET: f32 = 12.0;
const PARTICLE_COUNT: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl FloatingRect {
    /// Default Rex-style bottom-right quadrant placement inside the central terminal stage.
    pub fn default_for_stage(stage_w: f32, stage_h: f32) -> Self {
        let (margin, min_w, avail_w, min_h, avail_h) = stage_inner_limits(stage_w, stage_h);
        let width = (stage_w * 0.52).clamp(min_w, 740.0).min(avail_w);
        let height = (stage_h * 0.54).clamp(min_h, 520.0).min(avail_h);
        let x = (stage_w - width - 20.0).max(margin);
        let y = (stage_h - height - 20.0).max(margin);
        clamp_floating_rect(
            Self {
                x,
                y,
                width,
                height,
            },
            stage_w,
            stage_h,
            false,
        )
    }
}

/// Single source of truth computing `(margin, min_w, avail_w, min_h, avail_h)` for a given stage.
fn stage_inner_limits(stage_w: f32, stage_h: f32) -> (f32, f32, f32, f32, f32) {
    let margin = FLOATING_STAGE_MARGIN;
    let avail_w = (stage_w - margin * 2.0).max(64.0);
    let avail_h = (stage_h - margin * 2.0).max(FLOATING_COLLAPSED_HEIGHT);
    let min_w = FLOATING_MIN_WIDTH.min(avail_w);
    let min_h = FLOATING_MIN_HEIGHT.min(avail_h);
    (margin, min_w, avail_w, min_h, avail_h)
}

/// Single source of truth clamping a floating pane rectangle inside the central terminal stage.
/// Preserves `rect.height` when `collapsed == true` while clamping vertical position by collapsed bar height.
pub fn clamp_floating_rect(
    mut rect: FloatingRect,
    stage_w: f32,
    stage_h: f32,
    collapsed: bool,
) -> FloatingRect {
    let (margin, min_w, avail_w, min_h, avail_h) = stage_inner_limits(stage_w, stage_h);
    rect.width = rect.width.clamp(min_w, avail_w);

    if !collapsed {
        rect.height = rect.height.clamp(min_h, avail_h);
    }

    let eff_h = if collapsed {
        FLOATING_COLLAPSED_HEIGHT.min(avail_h)
    } else {
        rect.height
    };

    let max_x = (stage_w - margin - rect.width).max(margin);
    let max_y = (stage_h - margin - eff_h).max(margin);
    rect.x = rect.x.clamp(margin, max_x);
    rect.y = rect.y.clamp(margin, max_y);
    rect
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    West,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl ResizeEdge {
    pub fn cursor(self) -> CursorStyle {
        match self {
            Self::North | Self::South => CursorStyle::ResizeUpDown,
            Self::West | Self::East => CursorStyle::ResizeLeftRight,
            Self::NorthWest | Self::SouthEast => CursorStyle::ResizeUpLeftDownRight,
            Self::NorthEast | Self::SouthWest => CursorStyle::ResizeUpRightDownLeft,
        }
    }
}

pub fn apply_resize_delta(
    origin: FloatingRect,
    edge: ResizeEdge,
    dx: f32,
    dy: f32,
    stage_w: f32,
    stage_h: f32,
) -> FloatingRect {
    let (margin, min_w, avail_w, min_h, avail_h) = stage_inner_limits(stage_w, stage_h);
    let max_right = margin + avail_w;
    let max_bottom = margin + avail_h;

    let mut left = origin.x;
    let mut right = origin.x + origin.width;
    let mut top = origin.y;
    let mut bottom = origin.y + origin.height;

    match edge {
        ResizeEdge::West | ResizeEdge::NorthWest | ResizeEdge::SouthWest => {
            left = (origin.x + dx).clamp(margin, (right - min_w).max(margin));
        }
        ResizeEdge::East | ResizeEdge::NorthEast | ResizeEdge::SouthEast => {
            right = (origin.x + origin.width + dx).clamp((left + min_w).min(max_right), max_right);
        }
        _ => {}
    }

    match edge {
        ResizeEdge::North | ResizeEdge::NorthWest | ResizeEdge::NorthEast => {
            top = (origin.y + dy).clamp(margin, (bottom - min_h).max(margin));
        }
        ResizeEdge::South | ResizeEdge::SouthWest | ResizeEdge::SouthEast => {
            bottom =
                (origin.y + origin.height + dy).clamp((top + min_h).min(max_bottom), max_bottom);
        }
        _ => {}
    }

    clamp_floating_rect(
        FloatingRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
        stage_w,
        stage_h,
        false,
    )
}

/// Isolated lightweight decoration view so particle animation frames ONLY repaint
/// this perimeter canvas without invalidating `MicaWorkspace` or `TerminalPaneView`.
pub struct FloatingBorderParticles {
    epoch: Instant,
    pub active: bool,
    pub visible_in_foreground: bool,
}

impl FloatingBorderParticles {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let _ = cx;
        Self {
            epoch: Instant::now(),
            active: true,
            visible_in_foreground: true,
        }
    }
}

/// Maps normalized progress `t in [0.0, 1.0)` to `(point_on_perimeter, outward_normal)`
/// along a rounded rectangle of size `(w, h)` and corner radius `r`.
fn rounded_rect_perimeter_point(w: f32, h: f32, r: f32, t: f32) -> ((f32, f32), (f32, f32)) {
    let r = r.min(w * 0.5).min(h * 0.5).max(1.0);
    let top_len = (w - 2.0 * r).max(0.0);
    let side_len = (h - 2.0 * r).max(0.0);
    let arc_len = 0.5 * std::f32::consts::PI * r;
    let total = 2.0 * (top_len + side_len) + 4.0 * arc_len;
    if total <= 1.0 {
        return ((w * 0.5, 0.0), (0.0, -1.0));
    }

    let mut d = t.rem_euclid(1.0) * total;
    // 1. Top edge (left -> right)
    if d < top_len {
        return ((r + d, 0.0), (0.0, -1.0));
    }
    d -= top_len;
    // 2. Top-right arc (-pi/2 -> 0)
    if d < arc_len {
        let ang = -std::f32::consts::FRAC_PI_2 + (d / r);
        let (s, c) = ang.sin_cos();
        return ((w - r + r * c, r + r * s), (c, s));
    }
    d -= arc_len;
    // 3. Right edge (top -> bottom)
    if d < side_len {
        return ((w, r + d), (1.0, 0.0));
    }
    d -= side_len;
    // 4. Bottom-right arc (0 -> pi/2)
    if d < arc_len {
        let ang = d / r;
        let (s, c) = ang.sin_cos();
        return ((w - r + r * c, h - r + r * s), (c, s));
    }
    d -= arc_len;
    // 5. Bottom edge (right -> left)
    if d < top_len {
        return ((w - r - d, h), (0.0, 1.0));
    }
    d -= top_len;
    // 6. Bottom-left arc (pi/2 -> pi)
    if d < arc_len {
        let ang = std::f32::consts::FRAC_PI_2 + (d / r);
        let (s, c) = ang.sin_cos();
        return ((r + r * c, h - r + r * s), (c, s));
    }
    d -= arc_len;
    // 7. Left edge (bottom -> top)
    if d < side_len {
        return ((0.0, h - r - d), (-1.0, 0.0));
    }
    d -= side_len;
    // 8. Top-left arc (pi -> 3pi/2)
    let ang = std::f32::consts::PI + (d / r);
    let (s, c) = ang.sin_cos();
    ((r + r * c, r + r * s), (c, s))
}

impl Render for FloatingBorderParticles {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let should_animate =
            self.visible_in_foreground && window.is_window_active() && !cx.reduce_motion();
        if should_animate {
            let entity = cx.entity().downgrade();
            window.on_next_frame(move |window, cx| {
                let win_active = window.is_window_active();
                let reduce = cx.reduce_motion();
                let _ = entity.update(cx, |pt, cx| {
                    if pt.visible_in_foreground && win_active && !reduce {
                        cx.notify();
                    }
                });
            });
        }

        let elapsed = if cx.reduce_motion() {
            0.0
        } else {
            self.epoch.elapsed().as_secs_f32()
        };
        let is_active = self.active;
        let primary = cx.theme().primary;
        let cyan = cx.theme().cyan;
        let dark = cx.theme().mode.is_dark();

        canvas(
            |_, _, _| {},
            move |outer_bounds, _, window, _| {
                let inset = PARTICLE_HALO_INSET;
                let card_w = (f32::from(outer_bounds.size.width) - inset * 2.0).max(24.0);
                let card_h = (f32::from(outer_bounds.size.height) - inset * 2.0).max(24.0);
                let base_x = f32::from(outer_bounds.origin.x) + inset;
                let base_y = f32::from(outer_bounds.origin.y) + inset;

                let alpha_scale = if is_active {
                    if dark { 1.0 } else { 0.88 }
                } else {
                    0.42
                };

                for i in 0..PARTICLE_COUNT {
                    let seed = i as f32;
                    let base_t = seed / (PARTICLE_COUNT as f32);
                    let speed = 0.022 + ((i * 7) % 5) as f32 * 0.0065;
                    let dir = if i % 3 == 0 { -1.0 } else { 1.0 };
                    let t = base_t + elapsed * speed * dir;
                    let ((px_rel, py_rel), (nx, ny)) =
                        rounded_rect_perimeter_point(card_w, card_h, 8.5, t);

                    let drift_phase = elapsed * (1.5 + (i % 4) as f32 * 0.35) + seed * 1.37;
                    let outward = 1.6 + drift_phase.sin() * 2.4;
                    let cx_pt = base_x + px_rel + nx * outward;
                    let cy_pt = base_y + py_rel + ny * outward;

                    let twinkle = 0.5 + 0.5 * (elapsed * 2.4 + seed * 2.13).cos();
                    let tint: Hsla = if i % 2 == 0 { primary } else { cyan };

                    let glow_r = 2.8 + (i % 3) as f32 * 0.85;
                    let glow_alpha = (0.07 + 0.14 * twinkle) * alpha_scale;
                    window.paint_quad(
                        fill(
                            gpui_kit::Bounds {
                                origin: point(px(cx_pt - glow_r), px(cy_pt - glow_r)),
                                size: size(px(glow_r * 2.0), px(glow_r * 2.0)),
                            },
                            tint.opacity(glow_alpha),
                        )
                        .corner_radii(Corners::all(px(glow_r))),
                    );

                    let core_r = 1.0 + (i % 2) as f32 * 0.35;
                    let core_alpha = (0.24 + 0.42 * twinkle) * alpha_scale;
                    window.paint_quad(
                        fill(
                            gpui_kit::Bounds {
                                origin: point(px(cx_pt - core_r), px(cy_pt - core_r)),
                                size: size(px(core_r * 2.0), px(core_r * 2.0)),
                            },
                            tint.opacity(core_alpha),
                        )
                        .corner_radii(Corners::all(px(core_r))),
                    );
                }
            },
        )
        .size_full()
    }
}

pub fn floating_card_shadows(active: bool, cx: &App) -> Vec<BoxShadow> {
    let dark = cx.theme().mode.is_dark();
    let primary = cx.theme().primary;
    vec![
        BoxShadow {
            color: Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: if dark { 0.46 } else { 0.18 },
            },
            offset: point(px(0.0), px(10.0)),
            blur_radius: px(28.0),
            spread_radius: px(-2.0),
            inset: false,
        },
        BoxShadow {
            color: primary.opacity(if active {
                if dark { 0.22 } else { 0.15 }
            } else {
                0.07
            }),
            offset: point(px(0.0), px(2.0)),
            blur_radius: px(16.0),
            spread_radius: px(1.0),
            inset: false,
        },
    ]
}

#[allow(clippy::too_many_arguments)]
pub fn render_floating_pane_shell(
    id_suffix: &str,
    rect: FloatingRect,
    collapsed: bool,
    active: bool,
    particles: Entity<FloatingBorderParticles>,
    pane_element: gpui_kit::AnyElement,
    on_start_resize: impl Fn(ResizeEdge, Point<Pixels>, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> gpui_kit::AnyElement {
    let eff_h = if collapsed {
        FLOATING_COLLAPSED_HEIGHT
    } else {
        rect.height
    };
    let halo = PARTICLE_HALO_INSET;

    let handle_strip = |edge: ResizeEdge, name: &'static str| {
        let cb = on_start_resize.clone();
        let base = div()
            .id(SharedString::from(format!("float-resize-{name}-{id_suffix}")))
            .absolute()
            .occlude()
            .cursor(edge.cursor())
            .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                cb(edge, ev.position, window, cx);
            });
        match edge {
            ResizeEdge::North => base.top(px(-3.0)).left(px(8.0)).right(px(8.0)).h(px(6.0)),
            ResizeEdge::South => base
                .bottom(px(-3.0))
                .left(px(8.0))
                .right(px(8.0))
                .h(px(6.0)),
            ResizeEdge::West => base.left(px(-3.0)).top(px(8.0)).bottom(px(8.0)).w(px(6.0)),
            ResizeEdge::East => base.right(px(-3.0)).top(px(8.0)).bottom(px(8.0)).w(px(6.0)),
            ResizeEdge::NorthWest => base.top(px(-4.0)).left(px(-4.0)).size(px(10.0)),
            ResizeEdge::NorthEast => base.top(px(-4.0)).right(px(-4.0)).size(px(10.0)),
            ResizeEdge::SouthWest => base.bottom(px(-4.0)).left(px(-4.0)).size(px(10.0)),
            ResizeEdge::SouthEast => base.bottom(px(-4.0)).right(px(-4.0)).size(px(10.0)),
        }
    };

    let Size { width, height } = size(px(rect.width), px(eff_h));

    div()
        .id(SharedString::from(format!("floating-pane-wrap-{id_suffix}")))
        .absolute()
        .left(px(rect.x - halo))
        .top(px(rect.y - halo))
        .w(width + px(halo * 2.0))
        .h(height + px(halo * 2.0))
        .child(div().absolute().inset_0().child(particles))
        .child(
            div()
                .id(SharedString::from(format!("floating-pane-body-{id_suffix}")))
                .absolute()
                .occlude()
                .left(px(halo))
                .top(px(halo))
                .w(width)
                .h(height)
                .rounded(px(8.5))
                .shadow(floating_card_shadows(active, cx))
                .child(pane_element)
                .when(!collapsed, |card| {
                    card.child(handle_strip(ResizeEdge::North, "n"))
                        .child(handle_strip(ResizeEdge::South, "s"))
                        .child(handle_strip(ResizeEdge::West, "w"))
                        .child(handle_strip(ResizeEdge::East, "e"))
                        .child(handle_strip(ResizeEdge::NorthWest, "nw"))
                        .child(handle_strip(ResizeEdge::NorthEast, "ne"))
                        .child(handle_strip(ResizeEdge::SouthWest, "sw"))
                        .child(handle_strip(ResizeEdge::SouthEast, "se"))
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_rect_clamps_within_stage_across_collapse_and_shrink() {
        let def = FloatingRect::default_for_stage(1000.0, 700.0);
        assert!(def.x > 400.0 && def.y > 250.0);
        assert!(def.x + def.width <= 1000.0 - FLOATING_STAGE_MARGIN + 0.01);
        assert!(def.y + def.height <= 700.0 - FLOATING_STAGE_MARGIN + 0.01);

        // Shrinking stage pulls floating pane inside bounds without losing collapsed height
        let pushed = FloatingRect {
            x: 900.0,
            y: 650.0,
            width: 500.0,
            height: 360.0,
        };
        let collapsed = clamp_floating_rect(pushed, 600.0, 400.0, true);
        assert_eq!(collapsed.height, 360.0);
        assert!(collapsed.x + collapsed.width <= 600.0 - FLOATING_STAGE_MARGIN + 0.01);
        assert!(
            collapsed.y + FLOATING_COLLAPSED_HEIGHT <= 400.0 - FLOATING_STAGE_MARGIN + 0.01
        );

        let resized = apply_resize_delta(def, ResizeEdge::SouthEast, 500.0, 500.0, 1000.0, 700.0);
        assert!(resized.x == def.x && resized.y == def.y);
        assert!(resized.x + resized.width <= 1000.0 - FLOATING_STAGE_MARGIN + 0.01);
        assert!(resized.y + resized.height <= 700.0 - FLOATING_STAGE_MARGIN + 0.01);
    }
}
