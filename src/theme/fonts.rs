//! CJK 字体加载与族名管理（增量设计 v1.1 / T15）。
//!
//! 方案 A（首选，本实现采用）：内嵌开源 OFL 字体 **Noto Sans SC**，启动时经
//! `iced::daemon(...).font(CJK_BYTES).default_font(Font::with_name(CJK_FAMILY))`
//! 注册为全局默认字体，中文不再出现 tofu 方块。
//!
//! 字体来源：notofonts/noto-cjk `Sans/SubsetOTF/SC/NotoSansSC-Regular.otf`
//! （SIL Open Font License 1.1，许可文本见 `assets/fonts/LICENSE-OFL.txt`）。
//! 内部族名已通过解析 SFNT name 表核实：`Family(nameID=1) = "Noto Sans SC"`，
//! 与 [`CJK_FAMILY`] 逐字符一致（iced 0.13 按族名匹配已注册字体，必须精确）。
//!
//! 兜底（方案 B，未启用）：若内嵌字体在目标平台不可用，可改为按平台 cfg 取
//! 系统 CJK 族名（macOS `PingFang SC` / Windows `Microsoft YaHei` /
//! Linux `Noto Sans CJK SC`）构造 [`default_font`]，无需任何内嵌资源。

use iced::Font;

/// 内嵌的 Noto Sans SC Regular（SIL OFL 1.1）。
///
/// `include_bytes!` 在编译期把字体打进可执行文件，运行期零 IO、零路径依赖；
/// 同时保证 `cargo build` 产物自包含，无需随包分发 `assets/` 目录。
pub const CJK_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSansSC-Regular.otf");

/// 内嵌字体的内部族名（与 SFNT name 表 nameID=1 逐字符一致）。
pub const CJK_FAMILY: &str = "Noto Sans SC";

/// 全局默认字体（常规字重）。
pub fn default_font() -> Font {
    Font::with_name(CJK_FAMILY)
}

/// 全局默认字体（粗体，用于标题等强调文本）。
pub fn bold_font() -> Font {
    Font {
        weight: iced::font::Weight::Bold,
        ..default_font()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 族名常量非空且与已核实的内部族名一致。
    #[test]
    fn cjk_family_constant_is_exact() {
        assert_eq!(CJK_FAMILY, "Noto Sans SC");
        assert!(!CJK_FAMILY.is_empty());
    }

    /// 内嵌字节必须是合法 OTF/CFF（SFNT 魔数 `OTTO`），且体积合理（> 1 MiB）。
    #[test]
    fn cjk_bytes_are_valid_otf() {
        assert!(
            CJK_BYTES.len() > 1024 * 1024,
            "字体体积异常: {}",
            CJK_BYTES.len()
        );
        assert_eq!(
            &CJK_BYTES[..4],
            b"OTTO",
            "缺少 SFNT/OTTO 魔数，字体文件可能损坏"
        );
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试（增量 v1.1 / T15）
    // ──────────────────────────────────────────────────────────────

    /// `default_font()` 的族名必须与注册字体族逐字符一致，否则 iced 匹配不到
    /// 已加载字体，中文会回退 tofu。
    #[test]
    fn default_font_uses_registered_cjk_family() {
        let f = default_font();
        assert!(
            matches!(f.family, iced::font::Family::Name(name) if name == CJK_FAMILY),
            "default_font() 族名应与 CJK_FAMILY 一致"
        );
    }

    /// `bold_font()` 仅改字重，族名必须保持一致（标题强调文本仍可渲染中文）。
    #[test]
    fn bold_font_keeps_family_and_sets_weight() {
        let b = bold_font();
        assert!(
            matches!(b.family, iced::font::Family::Name(name) if name == CJK_FAMILY),
            "bold_font() 族名应与 CJK_FAMILY 一致"
        );
        assert_eq!(b.weight, iced::font::Weight::Bold);
    }

    /// 族名必须是合法 UTF-8 且不含空白/大小写混淆（iced 按精确字符串匹配）。
    #[test]
    fn cjk_family_has_no_surrounding_whitespace() {
        assert_eq!(CJK_FAMILY.trim(), CJK_FAMILY);
        assert!(!CJK_FAMILY.contains('\u{0}'));
    }
}
