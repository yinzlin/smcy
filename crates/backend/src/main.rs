#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod api;
pub mod middleware;
mod app_assets;
mod di;
mod tray;

use std::sync::atomic::{AtomicU16, Ordering};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};

/// 实际监听端口：优先 80，被占用时自动回退 8080~8099，供托盘「打开页面」读取
static ACTUAL_PORT: AtomicU16 = AtomicU16::new(80);

pub fn actual_port() -> u16 {
    ACTUAL_PORT.load(Ordering::Relaxed)
}

fn main() {
    // 锁定工作目录为 exe 所在目录，确保数据库、uploads、backups 始终生成在程序同级目录，
    // 不受快捷方式「起始位置」影响
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::env::set_current_dir(dir);
        }
    }

    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("创建 runtime 失败");
        rt.block_on(async {
            let state = di::bootstrap().await;
            let app = api::routes::build_router().with_state(state);
            let mut bound = None;
            for port in std::iter::once(80u16).chain(8080u16..=8099) {
                if let Ok(listener) = tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await {
                    bound = Some((listener, port));
                    break;
                }
            }
            let (listener, port) = bound.expect("没有可用端口（80 与 8080~8099 均被占用）");
            ACTUAL_PORT.store(port, Ordering::Relaxed);
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
