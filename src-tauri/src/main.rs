#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};
use ico::{IconDir};

use std::fs;
use serde::Serialize;
use base64::{engine::general_purpose, Engine as _};
use std::env;
use std::path::{Path, PathBuf};

#[tauri::command]
fn get_ini_content(app: tauri::AppHandle) -> Result<String, String> {
    let resource_path = app.path()
        .resolve("resources/config.ini", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    fs::read_to_string(resource_path).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct AuthInfo {
    machine_code: String,
    user_type_display: String,
    auth_end: String,
}

#[tauri::command]
fn get_auth_info(app: tauri::AppHandle) -> Result<AuthInfo, String> {
    let resource_path = app
        .path()
        .resolve("resources/config.ini", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let content = fs::read_to_string(resource_path).map_err(|e| e.to_string())?;

    let mut in_auth = false;
    let mut machine_code: Option<String> = None;
    let mut auth_type: Option<String> = None;
    let mut auth_end: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_auth = trimmed.eq_ignore_ascii_case("[AUTH]");
            continue;
        }
        if !in_auth || trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "machine_code" => machine_code = Some(val),
                "auth_type" => auth_type = Some(val),
                "auth_end" => auth_end = Some(val),
                _ => {}
            }
        }
    }

    let user_type_display = match auth_type.as_deref() {
        Some("free") => "免费用户".to_string(),
        Some("trial") => "试用用户".to_string(),
        Some(other) => other.to_string(),
        None => "(未找到)".to_string(),
    };

    Ok(AuthInfo {
        machine_code: machine_code.unwrap_or_else(|| "(未找到)".to_string()),
        user_type_display,
        auth_end: auth_end.unwrap_or_else(|| "(未找到)".to_string()),
    })
}

#[derive(Serialize)]
struct Announcement {
    title: String,
    content: String,
}

#[tauri::command]
fn get_announcement(app: tauri::AppHandle) -> Result<Announcement, String> {
    let path = app
        .path()
        .resolve("resources/announcement.ini", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    let mut title = String::from("(无标题)");
    let mut body = String::from("(无内容)");

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_lowercase();
            let val = v.trim();
            match key.as_str() {
                "title" => title = val.to_string(),
                "content" => body = val.to_string(),
                _ => {}
            }
        }
    }

    Ok(Announcement { title, content: body })
}

#[tauri::command]
fn get_advertisement_data_url(app: tauri::AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .resolve("resources/advertisement.png", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let encoded = general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:image/png;base64,{}", encoded))
}

fn extract_db_date_display(content: &str) -> String {
    let mut last: Option<&str> = None;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("上次同步时间") || t.starts_with("上次检查时间") {
            last = Some(t);
        }
    }
    if let Some(l) = last {
        if let Some((_k, v)) = l.split_once('=') {
            let val = v.trim();
            let date_part = val.split_once('(').map(|(d, _)| d.trim()).unwrap_or(val);
            return format!("数据库日期: {}\n", date_part);
        }
    }
    String::new()
}

#[tauri::command]
fn get_sync_log(app: tauri::AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .resolve("resources/sync_log.ini", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let content = fs::read_to_string(&path)
        .map_err(|_| format!("错误: 无法读取日志文件\n{}", path.display()))?;
    let prefix = extract_db_date_display(&content);
    Ok(format!("{}{}", prefix, content))
}

#[derive(Serialize)]
struct TdxPathStatus {
    path: String,
    is_valid: bool,
    error_msg: String,
}

fn resolve_tdx_ini_path() -> PathBuf {
    // 优先查找当前工作目录及其父级，再尝试 resources 目录
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let parent = cwd.parent().map(|p| p.to_path_buf());
    let candidates = vec![
        cwd.join("tdx_settings.ini"),
        cwd.join("resources").join("tdx_settings.ini"),
        cwd.join("..").join("tdx_settings.ini"),
    ];
    for p in &candidates {
        if p.exists() {
            return p.clone();
        }
    }
    if let Some(p) = parent { return p.join("tdx_settings.ini"); }
    cwd.join("tdx_settings.ini")
}

fn read_tdx_dir_from_ini(ini_path: &Path) -> Option<String> {
    let content = fs::read_to_string(ini_path).ok()?;
    let mut in_paths = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sect = &trimmed[1..trimmed.len()-1];
            in_paths = sect.eq_ignore_ascii_case("Paths");
            continue;
        }
        if in_paths {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim().eq_ignore_ascii_case("TDX_Directory") {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn write_tdx_dir_to_ini(ini_path: &Path, new_dir: &str) -> Result<(), String> {
    let line_new = format!("TDX_Directory = {}", new_dir);
    if ini_path.exists() {
        let content = fs::read_to_string(ini_path).map_err(|e| e.to_string())?;
        let mut out = String::new();
        let mut in_paths = false;
        let mut found_paths = false;
        let mut updated_key = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if in_paths && !updated_key {
                    out.push_str(&line_new);
                    out.push('\n');
                }
                let sect = &trimmed[1..trimmed.len()-1];
                in_paths = sect.eq_ignore_ascii_case("Paths");
                if in_paths { found_paths = true; }
                out.push_str(line);
                out.push('\n');
                continue;
            }
            if in_paths {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim().eq_ignore_ascii_case("TDX_Directory") {
                        out.push_str(&line_new);
                        out.push('\n');
                        updated_key = true;
                        continue;
                    }
                }
            }
            out.push_str(line);
            out.push('\n');
        }

        if in_paths && !updated_key {
            out.push_str(&line_new);
            out.push('\n');
        }
        if !found_paths {
            out.push_str("\n[Paths]\n");
            out.push_str(&line_new);
            out.push('\n');
        }
        fs::write(ini_path, out).map_err(|e| e.to_string())?
    } else {
        let out = format!("[Paths]\n{}\n", line_new);
        fs::write(ini_path, out).map_err(|e| e.to_string())?
    }
    Ok(())
}

#[tauri::command]
fn ensure_tdx_path_configured() -> Result<TdxPathStatus, String> {
    let ini_path = resolve_tdx_ini_path();
    let default_path = String::from("C:\\new_tdx");
    let saved = read_tdx_dir_from_ini(&ini_path);
    let tdx_path = saved.clone().unwrap_or(default_path.clone());
    let signals = PathBuf::from(&tdx_path).join("T0002").join("signals");
    let is_valid = signals.exists();
    let mut error_msg = String::new();

    if is_valid {
        if saved.is_none() {
            write_tdx_dir_to_ini(&ini_path, &tdx_path)?;
        }
    } else {
        error_msg = String::from("默认或已存路径无效, 请手动设置");
    }

    Ok(TdxPathStatus { path: tdx_path, is_valid, error_msg })
}

#[tauri::command]
fn set_new_tdx_path(new_path: String) -> Result<TdxPathStatus, String> {
    let signals = PathBuf::from(&new_path).join("T0002").join("signals");
    if signals.exists() {
        let ini_path = resolve_tdx_ini_path();
        write_tdx_dir_to_ini(&ini_path, &new_path)?;
        Ok(TdxPathStatus { path: new_path, is_valid: true, error_msg: String::new() })
    } else {
        Ok(TdxPathStatus {
            path: new_path,
            is_valid: false,
            error_msg: String::from("错误：您选择的路径不合法！请确保所选目录下存在 T0002\\signals 文件夹。"),
        })
    }
}

fn main() {
    run().expect("Failed to run application");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .setup(|app| {
            let quit = MenuItemBuilder::new("Quit").id("quit").build(app)?;
            let hide = MenuItemBuilder::new("Hide").id("hide").build(app)?;
            let show = MenuItemBuilder::new("Show").id("show").build(app)?;
            let settings = MenuItemBuilder::new("Settings").id("settings").build(app)?;
            let tray_menu = MenuBuilder::new(app).items(&[&settings, &quit, &hide, &show]).build()?;

            let icon_bytes = include_bytes!("../icons/icon.ico");
            let icon_dir = IconDir::read(std::io::Cursor::new(icon_bytes))?;
            let entry = icon_dir.entries().get(0).unwrap();
            let image = Image::new_owned(entry.decode()?.rgba_data().to_vec(), entry.width(), entry.height());

            let _tray = TrayIconBuilder::new()
                .icon(image)
                .menu(&tray_menu)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "hide" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("settings") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_ini_content,
            get_auth_info,
            get_announcement,
            get_advertisement_data_url,
            get_sync_log,
            ensure_tdx_path_configured,
            set_new_tdx_path
        ])
        .build(tauri::generate_context!())?;

    app.run(|_app_handle, _event| {});

    Ok(())
}
