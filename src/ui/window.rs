use eframe::egui;
use egui::{pos2, Pos2, Rect};

pub const COMPACT_WINDOW_W_WITH_SNIP: f32 = 198.0;
pub const COMPACT_WINDOW_W_NO_SNIP: f32 = 176.0;
pub const COMPACT_WINDOW_H: f32 = 74.0;
pub const COMPACT_WINDOW_H_WITH_SNIP: f32 = 102.0;
pub const COMPACT_BG_EXTRA_W: f32 = 36.0;
pub const COMPACT_BG_EXTRA_H: f32 = 12.0;

pub const WINDOW_MONITOR_MODE_FIXED: &str = "fixed";
pub const WINDOW_ANCHOR_TOP_LEFT: &str = "top_left";
pub const WINDOW_ANCHOR_TOP_CENTER: &str = "top_center";
pub const WINDOW_ANCHOR_TOP_RIGHT: &str = "top_right";
pub const WINDOW_ANCHOR_BOTTOM_LEFT: &str = "bottom_left";
pub const WINDOW_ANCHOR_BOTTOM_CENTER: &str = "bottom_center";
pub const WINDOW_ANCHOR_BOTTOM_RIGHT: &str = "bottom_right";

#[derive(Clone)]
pub struct MonitorChoice {
    pub id: String,
    pub label: String,
}

#[derive(Clone)]
pub struct MonitorWorkArea {
    pub id: String,
    pub work_px: windows::Win32::Foundation::RECT,
    pub is_primary: bool,
    pub scale_factor: f32,
}

#[cfg(windows)]
pub fn enumerate_monitor_work_areas() -> Vec<MonitorWorkArea> {
    use std::mem::size_of;
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    unsafe extern "system" fn enum_proc(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<MonitorWorkArea>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _).as_bool() {
            let nul = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let id = String::from_utf16_lossy(&info.szDevice[..nul]);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            let scale_factor =
                if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
                    (dpi_x as f32 / 96.0).max(0.5)
                } else {
                    1.0
                };
            out.push(MonitorWorkArea {
                id,
                work_px: info.monitorInfo.rcWork,
                is_primary: (info.monitorInfo.dwFlags & 1) != 0,
                scale_factor,
            });
        }

        BOOL(1)
    }

    let mut out: Vec<MonitorWorkArea> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut out as *mut Vec<MonitorWorkArea> as isize),
        );
    }
    out
}

#[cfg(not(windows))]
pub fn enumerate_monitor_work_areas() -> Vec<MonitorWorkArea> {
    Vec::new()
}

pub fn available_monitor_choices() -> Vec<MonitorChoice> {
    enumerate_monitor_work_areas()
        .into_iter()
        .enumerate()
        .map(|(idx, m)| MonitorChoice {
            id: m.id.clone(),
            label: format!(
                "{}{}",
                m.id,
                if m.is_primary {
                    " (primary)".into()
                } else {
                    format!(" (monitor {})", idx + 1)
                }
            ),
        })
        .collect()
}

pub fn resolve_target_monitor(monitor_id: &str) -> Option<MonitorWorkArea> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    let monitors = enumerate_monitor_work_areas();
    if monitors.is_empty() {
        return None;
    }

    let mut primary_work = RECT::default();
    let have_primary_work = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut primary_work as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok();

    if !monitor_id.trim().is_empty() {
        if let Some(m) = monitors.iter().find(|m| m.id == monitor_id) {
            return Some(m.clone());
        }
    }

    if have_primary_work {
        if let Some(m) = monitors.iter().find(|m| {
            m.work_px.left == primary_work.left
                && m.work_px.top == primary_work.top
                && m.work_px.right == primary_work.right
                && m.work_px.bottom == primary_work.bottom
        }) {
            return Some(m.clone());
        }
    }

    monitors
        .iter()
        .find(|m| m.is_primary)
        .cloned()
        .or_else(|| monitors.first().cloned())
}

#[cfg(windows)]
pub fn move_window_physical(x: i32, y: i32) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetWindowPos, SWP_NOSIZE, SWP_NOZORDER,
    };

    let title: Vec<u16> = "Mango Chat\0".encode_utf16().collect();
    if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) } {
        if !hwnd.is_invalid() {
            let _ = unsafe { SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER) };
        }
    }
}

#[cfg(not(windows))]
pub fn move_window_physical(_x: i32, _y: i32) {}

pub fn anchored_pos_physical(
    work: windows::Win32::Foundation::RECT,
    size_px: (i32, i32),
    anchor: &str,
) -> (i32, i32) {
    let margin = 10;
    let w = size_px.0.max(1);
    let h = size_px.1.max(1);
    let min_x = work.left + margin;
    let min_y = work.top + margin;
    let max_x = (work.right - w - margin).max(min_x);
    let max_y = (work.bottom - h - margin).max(min_y);
    match anchor {
        WINDOW_ANCHOR_TOP_LEFT => (min_x, min_y),
        WINDOW_ANCHOR_TOP_CENTER => ((work.left + work.right - w) / 2, min_y),
        WINDOW_ANCHOR_TOP_RIGHT => (max_x, min_y),
        WINDOW_ANCHOR_BOTTOM_LEFT => (min_x, max_y),
        WINDOW_ANCHOR_BOTTOM_CENTER => ((work.left + work.right - w) / 2, max_y),
        _ => (max_x, max_y),
    }
}

pub fn place_compact_fixed_native(
    size_logical: egui::Vec2,
    monitor_id: &str,
    anchor: &str,
) -> bool {
    let Some(m) = resolve_target_monitor(monitor_id) else {
        return false;
    };
    let sf = m.scale_factor.max(0.5);
    let size_px = (
        (size_logical.x * sf).round() as i32,
        (size_logical.y * sf).round() as i32,
    );
    let (x, y) = anchored_pos_physical(m.work_px, size_px, anchor);
    move_window_physical(x, y);
    true
}

pub fn anchored_position_in_work_area(
    work: Rect,
    size: egui::Vec2,
    anchor: &str,
) -> Option<Pos2> {
    let margin = 10.0;
    let min_x = work.min.x + margin;
    let min_y = work.min.y + margin;
    let max_x = work.max.x - size.x - margin;
    let max_y = work.max.y - size.y - margin;

    if min_x > max_x || min_y > max_y {
        return None;
    }

    let (x, y) = match anchor {
        WINDOW_ANCHOR_TOP_LEFT => (min_x, min_y),
        WINDOW_ANCHOR_TOP_CENTER => {
            ((work.center().x - size.x * 0.5).clamp(min_x, max_x), min_y)
        }
        WINDOW_ANCHOR_TOP_RIGHT => (max_x, min_y),
        WINDOW_ANCHOR_BOTTOM_LEFT => (min_x, max_y),
        WINDOW_ANCHOR_BOTTOM_CENTER => {
            ((work.center().x - size.x * 0.5).clamp(min_x, max_x), max_y)
        }
        _ => (max_x, max_y),
    };

    Some(pos2(x, y))
}

pub fn work_area_rect_logical(
    _ctx: &egui::Context,
    monitor_mode: &str,
    monitor_id: &str,
) -> Option<Rect> {
    let _ = monitor_mode;
    let chosen = resolve_target_monitor(monitor_id);

    if let Some(m) = chosen {
        let sf = m.scale_factor.max(0.5);
        return Some(Rect::from_min_max(
            pos2(m.work_px.left as f32 / sf, m.work_px.top as f32 / sf),
            pos2(
                m.work_px.right as f32 / sf,
                m.work_px.bottom as f32 / sf,
            ),
        ));
    }

    None
}

pub fn clamp_window_pos(
    ctx: &egui::Context,
    pos: Pos2,
    size: egui::Vec2,
    monitor_mode: &str,
    monitor_id: &str,
) -> Pos2 {
    let Some(work) = work_area_rect_logical(ctx, monitor_mode, monitor_id) else {
        return pos;
    };
    let margin = 8.0;
    let min_x = work.min.x + margin;
    let min_y = work.min.y + margin;
    let max_x = work.max.x - size.x - margin;
    let max_y = work.max.y - size.y - margin;
    let x = if min_x <= max_x {
        pos.x.clamp(min_x, max_x)
    } else {
        min_x
    };
    let y = if min_y <= max_y {
        pos.y.clamp(min_y, max_y)
    } else {
        min_y
    };
    pos2(x, y)
}

pub fn default_compact_position_for_size(
    ctx: &egui::Context,
    size: egui::Vec2,
    monitor_mode: &str,
    monitor_id: &str,
    anchor: &str,
) -> Option<Pos2> {
    let work = work_area_rect_logical(ctx, monitor_mode, monitor_id)?;
    anchored_position_in_work_area(work, size, anchor)
}

