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
        .invoke_handler(tauri::generate_handler![get_ini_content, get_auth_info, get_announcement, get_advertisement_data_url])
        .build(tauri::generate_context!())?;

    app.run(|_app_handle, _event| {});

    Ok(())
}
