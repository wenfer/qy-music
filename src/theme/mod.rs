//! 主题 / 皮肤模块。

pub mod schema;
pub mod skin;
pub mod theme;

#[cfg(feature = "gui")]
pub mod fonts;

pub use skin::{Skin, SkinColors, SkinLayout};
pub use theme::{default_skin, hex_to_rgb, DEFAULT_SKIN_ID};

#[cfg(feature = "gui")]
pub use theme::skin_color;
