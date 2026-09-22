//! 音效预设模型与内置库（Sound Effects & EQ Presets）。
//!
//! 支持 10 段 EQ 增益、主增益以及发烧级 DSP 参数（胆机、空间声场、BS2B、动态低音、人声水晶）。
//! 提供内置经典调音、发烧大耳校准预设，以及用户自定义预设的序列化、反序列化、导入与导出。
//! 纯逻辑实现，无 GUI 依赖。

use serde::{Deserialize, Serialize};

use crate::audio::effects::AudioEffects;
use crate::audio::equalizer::EqPreset;

/// 音效与调音预设数据结构。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundPreset {
    /// 唯一标识（内置预设如 "builtin_flat"，自定义预设如 "custom_1720000000"）。
    pub id: String,
    /// 显示名称（如 "平直 (Flat)"、"摇滚激情"、"我的发烧大耳"）。
    pub name: String,
    /// 是否为内置预设（内置预设不可删除与直接覆盖）。
    #[serde(default)]
    pub is_builtin: bool,
    /// 10 段均衡器增益（dB）。
    pub bands: [f32; 10],
    /// 主增益（dB）。
    pub master_gain_db: f32,
    /// DSP 音效参数（胆机、全景空间、BS2B、低音、人声、纯净直通）。
    pub effects: AudioEffects,
}

impl SoundPreset {
    /// 构造新的自定义音效预设。
    pub fn new_custom(
        name: impl Into<String>,
        bands: [f32; 10],
        master_gain_db: f32,
        effects: AudioEffects,
    ) -> Self {
        let name = name.into();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("custom_{timestamp}");
        Self {
            id,
            name,
            is_builtin: false,
            bands,
            master_gain_db,
            effects,
        }
    }

    /// 获取全部内置音效预设。
    pub fn builtin_presets() -> Vec<Self> {
        vec![
            Self {
                id: "builtin_flat".to_string(),
                name: "平直 (Flat)".to_string(),
                is_builtin: true,
                bands: EqPreset::Flat.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects::default(),
            },
            Self {
                id: "builtin_pop".to_string(),
                name: "流行原声 (Pop)".to_string(),
                is_builtin: true,
                bands: EqPreset::Pop.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects {
                    vocal_crystalizer_enabled: true,
                    vocal_crystalizer_level: 0.40,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_rock".to_string(),
                name: "摇滚激情 (Rock)".to_string(),
                is_builtin: true,
                bands: EqPreset::Rock.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects {
                    bass_boost_enabled: true,
                    bass_boost_level: 0.45,
                    vocal_crystalizer_enabled: true,
                    vocal_crystalizer_level: 0.30,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_classical".to_string(),
                name: "古典音乐厅 (Classical)".to_string(),
                is_builtin: true,
                bands: EqPreset::Classical.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects {
                    spatial_audio_enabled: true,
                    spatial_audio_level: 0.50,
                    dialogue_clarity_level: 0.40,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_vocal".to_string(),
                name: "纯净人声 (Vocal)".to_string(),
                is_builtin: true,
                bands: EqPreset::Vocal.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects {
                    dialogue_clarity_level: 0.70,
                    vocal_crystalizer_enabled: true,
                    vocal_crystalizer_level: 0.50,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_tube".to_string(),
                name: "温暖胆机 (Tube Warmth)".to_string(),
                is_builtin: true,
                bands: [1.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.0, -0.5, -1.0, -1.5],
                master_gain_db: 0.0,
                effects: AudioEffects {
                    tube_warmth_enabled: true,
                    tube_warmth_level: 0.55,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_spatial".to_string(),
                name: "影院全景声场 (Spatial)".to_string(),
                is_builtin: true,
                bands: [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.5, 2.0, 2.0],
                master_gain_db: 0.0,
                effects: AudioEffects {
                    spatial_audio_enabled: true,
                    spatial_audio_level: 0.75,
                    dialogue_clarity_level: 0.60,
                    bs2b_mode: false,
                    bass_boost_enabled: true,
                    bass_boost_level: 0.40,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_bs2b".to_string(),
                name: "BS2B 发烧耳放互馈".to_string(),
                is_builtin: true,
                bands: [0.0; 10],
                master_gain_db: 0.0,
                effects: AudioEffects {
                    spatial_audio_enabled: true,
                    bs2b_mode: true,
                    spatial_audio_level: 0.70,
                    ..AudioEffects::default()
                },
            },
            Self {
                id: "builtin_harman".to_string(),
                name: "哈曼参考曲线 (Harman)".to_string(),
                is_builtin: true,
                bands: EqPreset::Harman.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects::default(),
            },
            Self {
                id: "builtin_hd600".to_string(),
                name: "森海塞尔 HD600/650 校准".to_string(),
                is_builtin: true,
                bands: EqPreset::Hd600.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects::default(),
            },
            Self {
                id: "builtin_dt990".to_string(),
                name: "拜雅 DT990/880 校准".to_string(),
                is_builtin: true,
                bands: EqPreset::Dt990.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects::default(),
            },
            Self {
                id: "builtin_k701".to_string(),
                name: "AKG K701/Q701 校准".to_string(),
                is_builtin: true,
                bands: EqPreset::K701.bands(),
                master_gain_db: 0.0,
                effects: AudioEffects::default(),
            },
        ]
    }

    /// 导出为格式化 JSON 文本。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从 JSON 文本解析预设（自动保证导入为非内置预设）。
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let mut preset: Self = serde_json::from_str(json_str)?;
        // 导入的预设一律视为用户自定义，防止覆盖内置只读预设
        preset.is_builtin = false;
        // 增益范围钳位保障安全
        for b in &mut preset.bands {
            *b = b.clamp(-12.0, 12.0);
        }
        preset.master_gain_db = preset.master_gain_db.clamp(-12.0, 12.0);
        Ok(preset)
    }
}

impl std::fmt::Display for SoundPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_builtin {
            write!(f, "{}", self.name)
        } else {
            write!(f, "★ {}", self.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_have_unique_ids() {
        let presets = SoundPreset::builtin_presets();
        let mut ids = std::collections::HashSet::new();
        for p in presets {
            assert!(p.is_builtin);
            assert!(ids.insert(p.id));
        }
    }

    #[test]
    fn custom_preset_creation_and_json_roundtrip() {
        let effects = AudioEffects {
            tube_warmth_enabled: true,
            tube_warmth_level: 0.8,
            ..Default::default()
        };

        let preset = SoundPreset::new_custom(
            "测试发烧预设",
            [1.0, 2.0, 3.0, 0.0, 0.0, 0.0, -1.0, -2.0, -3.0, 0.0],
            1.5,
            effects,
        );
        assert!(!preset.is_builtin);
        assert_eq!(preset.name, "测试发烧预设");

        let json = preset.to_json().expect("序列化成功");
        let parsed = SoundPreset::from_json(&json).expect("反序列化成功");
        assert_eq!(parsed.name, preset.name);
        assert_eq!(parsed.bands, preset.bands);
        assert_eq!(parsed.master_gain_db, 1.5);
        assert_eq!(parsed.effects.tube_warmth_level, 0.8);
        assert!(!parsed.is_builtin);
    }
}
