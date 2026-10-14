#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod api;
pub mod middleware;
mod app_assets;
mod di;
mod tray;

// ---- 兼容 shim：保持历史代码中的 crate::xxx 路径不变 ----
pub(crate) use common::utils;
pub(crate) use database as db;
pub(crate) use domain as models;
pub(crate) use crate::middleware::auth;

pub(crate) use application::service::*;
pub(crate) use common::utils::*;
pub(crate) use crate::middleware::auth::*;

use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};

fn main() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("创建 runtime 失败");
        rt.block_on(async {
            let state = di::bootstrap().await;
            let app = api::routes::build_router().with_state(state);
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 80));
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });

    let event_loop = EventLoop::new();

    let menu = Menu::new();
    let open_item = MenuItem::with_id("open", "打开页面", true, None);
    let quit_item = MenuItem::with_id("quit", "退出", true, None);
    let _ = menu.append(&open_item);
    let _ = menu.append(&quit_item);

    let icon = tray::make_app_icon();
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("食材收发系统")
        .with_icon(icon)
        .build()
        .expect("创建托盘图标失败");

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            match menu_event.id().as_ref() {
                "open" => tray::open_browser(),
                "quit" => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        }

        if let Ok(tray_event) = tray_channel.try_recv() {
            if let TrayIconEvent::Click { button, button_state, .. } = tray_event {
                // 仅响应左键单击：右键不打开 web（会弹出托盘菜单）
                if button == tray_icon::MouseButton::Left && button_state == tray_icon::MouseButtonState::Up {
                    tray::open_browser();
                }
            }
        }
    });
}
