//! 系统托盘辅助：应用图标生成与默认浏览器打开页面

use tray_icon::Icon;

pub fn make_app_icon() -> Icon {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = 32.0f32;
    let cy = 32.0f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if dist <= 30.0 {
                rgba[idx] = 67;
                rgba[idx + 1] = 160;
                rgba[idx + 2] = 71;
                rgba[idx + 3] = 255;
                if dist <= 22.0 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = 255;
                }
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("生成图标失败")
}

pub fn open_browser() {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", "http://127.0.0.1/"])
        .spawn();
}
