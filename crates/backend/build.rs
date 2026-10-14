//! 保证 rust-embed 的 ../frontend/dist 目录在编译期存在（前端未构建时写入占位页）

fn main() {
    let dist = std::path::Path::new("../frontend/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        std::fs::create_dir_all(dist).expect("创建 frontend/dist 占位目录失败");
        std::fs::write(
            &index,
            "<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><title>前端未构建</title><p>请先在 crates/frontend 执行 npm run build</p></html>",
        )
        .expect("写入前端占位页失败");
    }
    println!("cargo:rerun-if-changed=../frontend/dist");
}
