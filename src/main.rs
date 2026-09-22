//! LFPlayer — 二进制入口。
//!
//! 完整 GUI 需要 `gui` 特性（默认启用）。在无 `gui` 的纯逻辑构建下，
//! 二进制仅作占位，真正的逻辑测试通过 `cargo test --lib --no-default-features` 完成。

#[cfg(feature = "gui")]
fn main() -> iced::Result {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    lingfeng::app::run()
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("lingfeng 以 --no-default-features 构建：仅包含纯逻辑模块，无 GUI。");
    eprintln!("运行 `cargo test --lib --no-default-features` 验证纯逻辑。");
}
