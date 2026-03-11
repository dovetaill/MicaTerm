use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;
use crate::app::windowing::WindowController;
use crate::shell::view_model::ShellViewModel;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn default_window_size() -> (u32, u32) {
    (1440, 900)
}

fn sync_top_status_bar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_settings_menu(state.show_settings_menu);
    window.set_is_window_maximized(state.is_window_maximized);
    window.set_is_window_active(state.is_window_active);
}

pub fn bind_top_status_bar(window: &AppWindow) {
    let view_model = Rc::new(RefCell::new(ShellViewModel::default()));
    let controller = Rc::new(WindowController::new(window));

    sync_top_status_bar_state(window, &view_model.borrow());

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_settings_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_settings_menu();
        window.set_show_settings_menu(state.show_settings_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_settings_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_settings_menu();
        window.set_show_settings_menu(state.show_settings_menu);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_minimize_requested(move || {
        controller_ref.minimize();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_close_requested(move || {
        let _ = controller_ref.close();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_requested(move || {
        let _ = controller_ref.drag();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });
}

pub fn run() -> Result<()> {
    let window = AppWindow::new()?;
    bind_top_status_bar(&window);
    window.run()?;
    Ok(())
}
