use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::cell::RefCell;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::Threading::GetCurrentProcessId,
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            Input::KeyboardAndMouse::SetFocus,
            WindowsAndMessaging::*,
        },
    },
    core::Result as WinResult,
};
#[path = "overlay_state.rs"]
mod overlay_state;
use overlay_state::Lifecycle;
#[derive(Default)]
struct NativeState {
    life: Lifecycle,
    previous: Option<(HWND, u32)>,
    present_pending: bool,
    target: Option<(RECT, f64)>,
}
thread_local! { static STATE:RefCell<NativeState>=RefCell::new(NativeState::default()); }

fn own_foreground() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        let mut process = 0;
        GetWindowThreadProcessId(foreground, Some(&mut process));
        process == GetCurrentProcessId()
    }
}
fn work_area() -> Option<(RECT, f64)> {
    unsafe {
        let foreground = GetForegroundWindow();
        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let (mut x, mut y) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y);
        Some((info.rcWork, f64::from(x.max(96)) / 96.))
    }
}
pub fn attributes(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;
    // Taskbar suppression is set at creation, before Slint's first show. Never
    // use winit::focus_window: this pinned version synthesizes Alt input.
    let attrs = attrs
        .with_window_icon(
            winit::window::Icon::from_rgba(
                include_bytes!("../../resources/ferrum-icon-32.rgba").to_vec(),
                32,
                32,
            )
            .ok(),
        )
        .with_visible(false)
        .with_resizable(false)
        .with_skip_taskbar(true)
        .with_window_level(winit::window::WindowLevel::Normal);
    if let Some((work, scale)) = work_area() {
        attrs.with_position(position(
            work,
            (730. * scale).min(f64::from(work.right - work.left) - 24. * scale) as i32,
            (206. * scale) as i32,
            scale,
        ))
    } else {
        attrs
    }
}
pub fn overlay_style(style: u32) -> u32 {
    (style | WS_EX_TOOLWINDOW.0) & !(WS_EX_APPWINDOW.0 | WS_EX_TOPMOST.0 | WS_EX_NOACTIVATE.0)
}
fn native(window: &slint::Window, f: impl FnOnce(HWND)) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window
        .with_winit_window(|w| {
            if let Ok(handle) = w.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                f(HWND(handle.hwnd.get() as *mut _));
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}
fn configure(hwnd: HWND) -> WinResult<()> {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let after = overlay_style(before);
        if before != after {
            SetLastError(ERROR_SUCCESS);
            let previous = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, after as isize);
            if previous == 0 && GetLastError() != ERROR_SUCCESS {
                return Err(windows::core::Error::from_thread());
            }
            SetWindowPos(
                hwnd,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )?;
        }
        Ok(())
    }
}
fn present(ui: &super::Launcher) {
    let done = native(ui.window(), |hwnd| unsafe {
        STATE.with(|s| s.borrow_mut().present_pending = false);
        if let Err(e) = configure(hwnd) {
            ui.set_error(format!("Overlay style could not be applied: {e}").into());
        }
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if SetForegroundWindow(hwnd).as_bool() {
            let _ = SetFocus(Some(hwnd));
        }
    });
    if done {
        STATE.with(|s| s.borrow_mut().present_pending = false);
    }
    ui.invoke_focus_search();
}
/// Called only by UI-thread callbacks during the manually started GUI lifetime.
pub fn install(ui: &super::Launcher) {
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |window, event| {
        native(window, |hwnd| {
            let _ = configure(hwnd);
        });
        if let Some(ui) = weak.upgrade() {
            if STATE.with(|s| s.borrow().present_pending) {
                present(&ui);
            }
            match event {
                winit::event::WindowEvent::Focused(true) => {
                    STATE.with(|s| s.borrow_mut().life.focus())
                }
                winit::event::WindowEvent::Focused(false) => dismiss_later(&ui),
                winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    STATE.with(|s| {
                        if let Some((_, scale)) = &mut s.borrow_mut().target {
                            *scale = *scale_factor;
                        }
                    });
                    place_after_dpi(&ui);
                }
                _ => {}
            }
        }
        EventResult::Propagate
    });
}
fn place_after_dpi(ui: &super::Launcher) {
    let epoch = STATE.with(|s| s.borrow().life.current());
    let weak = ui.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        let target = STATE.with(|s| {
            let s = s.borrow();
            if s.life.current() == epoch && epoch.is_some() {
                s.target
            } else {
                None
            }
        });
        if let Some((work, scale)) = target
            && let Some(ui) = weak.upgrade()
        {
            ui.set_available_width(((work.right - work.left) as f64 / scale - 24.).max(1.) as f32);
            ui.window().with_winit_window(|window| {
                let size = window.outer_size();
                let origin = position(work, size.width as i32, size.height as i32, scale);
                ui.set_available_height(
                    ((work.bottom - origin.y) as f64 / scale - 12.).max(264.) as f32
                );
                window.set_outer_position(origin);
            });
        }
    });
}
pub fn watch_settings(settings: &super::Settings, ui: slint::Weak<super::Launcher>) {
    settings.window().on_winit_window_event(move |_, event| {
        if matches!(event, winit::event::WindowEvent::Focused(false))
            && let Some(ui) = ui.upgrade()
        {
            dismiss_later(&ui);
        }
        EventResult::Propagate
    });
}
fn dismiss_later(ui: &super::Launcher) {
    let epoch = STATE.with(|s| s.borrow().life.deactivation());
    if let Some(epoch) = epoch {
        let weak = ui.as_weak();
        let _ = slint::invoke_from_event_loop(move || {
            let dismiss = STATE.with(|s| s.borrow().life.dismiss(epoch, own_foreground()));
            if dismiss && let Some(ui) = weak.upgrade() {
                let _ = hide(&ui);
            }
        });
    }
}
pub fn show(ui: &super::Launcher) -> Result<(), slint::PlatformError> {
    spotlight_prototype::diagnostics::record("show_requested", 0);
    let area = work_area();
    let previous = unsafe { GetForegroundWindow() };
    let mut previous_process = 0;
    unsafe {
        GetWindowThreadProcessId(previous, Some(&mut previous_process));
    }
    let external = !own_foreground();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.life.show();
        s.present_pending = true;
        s.target = area;
        if external {
            s.previous = Some((previous, previous_process));
        }
    });
    ui.invoke_prepare_opening();
    if let Some((work, scale)) = area {
        ui.set_available_width(((work.right - work.left) as f64 / scale - 24.).max(1.) as f32);
        let size = ui.window().size();
        let origin = position(work, size.width as i32, size.height as i32, scale);
        ui.set_available_height(((work.bottom - origin.y) as f64 / scale - 12.).max(264.) as f32);
    }
    if let Err(error) = ui.show() {
        STATE.with(|s| s.borrow_mut().life.hide());
        return Err(error);
    }
    if let Some((work, scale)) = area {
        ui.window().with_winit_window(|window| {
            let size = window.outer_size();
            let origin = position(work, size.width as i32, size.height as i32, scale);
            ui.set_available_height(
                ((work.bottom - origin.y) as f64 / scale - 12.).max(264.) as f32
            );
            window.set_outer_position(origin);
        });
    }
    present(ui);
    ui.invoke_start_opening();
    spotlight_prototype::diagnostics::record("show_completed", 0);
    Ok(())
}
pub fn hide(ui: &super::Launcher) -> Result<(), slint::PlatformError> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.life.hide();
        s.present_pending = false;
    });
    ui.invoke_cancel_opening();
    ui.hide()?;
    spotlight_prototype::diagnostics::record("hidden", 0);
    native(ui.window(), |hwnd| unsafe {
        // Never pull focus away from an application the user has already clicked.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let previous = STATE.with(|s| s.borrow().previous);
        if let Some((previous, process)) = previous {
            let mut current_process = 0;
            GetWindowThreadProcessId(previous, Some(&mut current_process));
            if overlay_state::restore_allowed(
                GetForegroundWindow() == hwnd,
                IsWindow(Some(previous)).as_bool(),
                process == current_process,
            ) {
                let _ = SetForegroundWindow(previous);
            }
        }
    });
    Ok(())
}
pub fn animations_enabled() -> bool {
    unsafe {
        let mut enabled = windows::core::BOOL(1);
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut windows::core::BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        enabled.as_bool()
    }
}

fn position(
    work: windows::Win32::Foundation::RECT,
    width: i32,
    height: i32,
    scale: f64,
) -> winit::dpi::PhysicalPosition<i32> {
    let margin = (12. * scale) as i32;
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let desired_y = work.top + ((work.bottom - work.top) as f64 * 0.42) as i32 - height / 2;
    let top = work.top + margin;
    let reserve = height.max((570. * scale) as i32);
    let bottom = (work.bottom - reserve - margin).max(top);
    winit::dpi::PhysicalPosition::new(x, desired_y.clamp(top, bottom))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collapsed_anchor_leaves_room_for_six_results_on_short_scaled_monitors() {
        for scale in [1., 1.25, 1.5, 2.] {
            let work = windows::Win32::Foundation::RECT {
                left: -1920,
                top: -100,
                right: 0,
                bottom: (720. * scale) as i32 - 100,
            };
            let width = (686. * scale) as i32;
            let anchor = position(work, width, (206. * scale) as i32, scale);
            assert!(anchor.y + (570. * scale) as i32 <= work.bottom - (12. * scale) as i32);
            assert_eq!(anchor.x, work.left + (work.right - work.left - width) / 2);
        }
    }
    #[test]
    fn style_is_a_focusable_tool_window_without_taskbar_or_topmost_bits() {
        let before = WS_EX_APPWINDOW.0 | WS_EX_TOPMOST.0 | WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0;
        let after = overlay_style(before);
        assert_ne!(after & WS_EX_TOOLWINDOW.0, 0);
        assert_eq!(
            after & (WS_EX_APPWINDOW.0 | WS_EX_TOPMOST.0 | WS_EX_NOACTIVATE.0),
            0
        );
        assert_ne!(after & WS_EX_LAYERED.0, 0);
        assert_eq!(overlay_style(after), after);
    }
    #[test]
    fn work_area_bounds_leave_animation_and_error_room_even_on_small_displays() {
        for scale in [1., 1.25, 1.5, 2., 2.5] {
            for height in [540., 720., 1080.] {
                let work = RECT {
                    left: 120,
                    top: -800,
                    right: 2040,
                    bottom: -800 + (height * scale) as i32,
                };
                let origin = position(work, (730. * scale) as i32, (206. * scale) as i32, scale);
                let max_height = ((work.bottom - origin.y) as f64 / scale - 12.).max(264.);
                // Surface/input/footer/padding plus 108-DIP error leave at least one row.
                let results = (max_height - 206. - 108.).max(76.);
                let actual = 206. + 108. + results;
                if height >= 720. {
                    assert!(origin.y + (actual * scale) as i32 <= work.bottom);
                }
                assert!(origin.y >= work.top);
            }
        }
    }
}
