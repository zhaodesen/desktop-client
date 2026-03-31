use tauri::{window::Color, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn ensure_overlay_window_with_visibility(app: &AppHandle, visible: bool) -> Result<(), String> {
    if app.get_webview_window("overlay").is_some() {
        return Ok(());
    }

    let _window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("悬浮字幕")
        .visible(visible)
        .always_on_top(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(true)
        .maximizable(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .inner_size(760.0, 190.0)
        .min_inner_size(420.0, 72.0)
        .build()
        .map_err(|error| format!("创建悬浮窗失败: {error}"))?;

    #[cfg(target_os = "windows")]
    _window
        .set_background_color(Some(Color(0, 0, 0, 0)))
        .map_err(|error| format!("设置悬浮窗背景失败: {error}"))?;

    Ok(())
}

pub fn show_overlay(app: &AppHandle) -> Result<bool, String> {
    ensure_overlay_window_with_visibility(app, true)?;

    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| "未找到悬浮窗".to_string())?;

    #[cfg(target_os = "windows")]
    window
        .set_background_color(Some(Color(0, 0, 0, 0)))
        .map_err(|error| format!("设置悬浮窗背景失败: {error}"))?;

    window
        .show()
        .map_err(|error| format!("显示悬浮窗失败: {error}"))?;

    Ok(true)
}

pub fn hide_overlay(app: &AppHandle) -> Result<bool, String> {
    let Some(window) = app.get_webview_window("overlay") else {
        return Ok(false);
    };

    window
        .hide()
        .map_err(|error| format!("隐藏悬浮窗失败: {error}"))?;

    Ok(false)
}

pub fn toggle_overlay(app: &AppHandle) -> Result<bool, String> {
    let Some(window) = app.get_webview_window("overlay") else {
        return show_overlay(app);
    };

    let visible = window
        .is_visible()
        .map_err(|error| format!("读取悬浮窗状态失败: {error}"))?;

    if visible {
        hide_overlay(app)
    } else {
        show_overlay(app)
    }
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;

    let _ = window.unminimize();
    window
        .show()
        .map_err(|error| format!("显示主窗口失败: {error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦主窗口失败: {error}"))?;

    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;

    window
        .hide()
        .map_err(|error| format!("隐藏主窗口失败: {error}"))
}
