use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Presentation only. A theme belongs to a view, never to a Document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// An sRGB colour. Convert RGB to linear values before writing GPU instances;
/// alpha remains linear. CSS uses the same original sRGB components.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColor {
    srgb: [u8; 3],
    linear: [f32; 4],
}
impl ThemeColor {
    fn new(srgb: [u8; 3]) -> Self {
        let [r, g, b] = srgb.map(|v| {
            let srgb = f32::from(v) / 255.0;
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        });
        Self {
            srgb,
            linear: [r, g, b, 1.0],
        }
    }
    /// Conversion is cached per palette, so drawing many nodes only copies values.
    pub fn linear(self) -> [f32; 4] {
        self.linear
    }
    pub fn css(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.srgb[0], self.srgb[1], self.srgb[2]
        )
    }
}

/// Shared tokens for Rust/GPU and WebView controls.
pub struct ThemePalette {
    pub background: ThemeColor,
    pub grid: ThemeColor,
    pub node: ThemeColor,
    pub border: ThemeColor,
    pub accent: ThemeColor,
    pub edge: ThemeColor,
    pub port: ThemeColor,
    pub text: ThemeColor,
    pub muted: ThemeColor,
    pub hover: ThemeColor,
    pub cable_valid: ThemeColor,
    pub cable_invalid: ThemeColor,
}
impl Theme {
    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Dark => ThemePalette {
                background: ThemeColor::new([17, 25, 35]),
                grid: ThemeColor::new([34, 45, 58]),
                node: ThemeColor::new([36, 49, 65]),
                border: ThemeColor::new([114, 134, 154]),
                accent: ThemeColor::new([103, 212, 187]),
                edge: ThemeColor::new([115, 178, 196]),
                port: ThemeColor::new([103, 212, 187]),
                text: ThemeColor::new([219, 228, 237]),
                muted: ThemeColor::new([164, 181, 198]),
                hover: ThemeColor::new([45, 62, 81]),
                cable_valid: ThemeColor::new([103, 212, 187]),
                cable_invalid: ThemeColor::new([244, 158, 95]),
            },
            Self::Light => ThemePalette {
                background: ThemeColor::new([243, 246, 249]),
                grid: ThemeColor::new([221, 228, 236]),
                node: ThemeColor::new([255, 255, 255]),
                border: ThemeColor::new([121, 143, 164]),
                accent: ThemeColor::new([17, 119, 97]),
                edge: ThemeColor::new([56, 103, 123]),
                port: ThemeColor::new([17, 119, 97]),
                text: ThemeColor::new([27, 43, 57]),
                muted: ThemeColor::new([76, 95, 112]),
                hover: ThemeColor::new([228, 237, 242]),
                cable_valid: ThemeColor::new([17, 119, 97]),
                cable_invalid: ThemeColor::new([174, 69, 22]),
            },
        }
    }
}
impl ThemePalette {
    /// Small presentation metadata, suitable for IPC. Keys are CSS token names.
    pub fn css_variables(&self) -> BTreeMap<String, String> {
        [
            ("background", self.background),
            ("surface", self.node),
            ("border", self.border),
            ("accent", self.accent),
            ("text", self.text),
            ("muted", self.muted),
            ("hover", self.hover),
        ]
        .into_iter()
        .map(|(key, color)| (key.into(), color.css()))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contrast(a: ThemeColor, b: ThemeColor) -> f32 {
        let luminance = |c: ThemeColor| {
            let [r, g, b, _] = c.linear();
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn text_and_interaction_tokens_remain_readable_in_both_themes() {
        for theme in [Theme::Dark, Theme::Light] {
            let p = theme.palette();
            for surface in [p.background, p.node, p.hover] {
                assert!(contrast(p.text, surface) >= 4.5);
                assert!(contrast(p.muted, surface) >= 4.5);
                assert!(contrast(p.accent, surface) >= 3.0);
            }
            assert!(contrast(p.border, p.node) >= 3.0);
            assert!(contrast(p.edge, p.background) >= 3.0);
        }
    }
    #[test]
    fn css_and_gpu_share_srgb_tokens_with_linear_gpu_values() {
        let c = ThemeColor::new([128, 0, 255]);
        assert_eq!(c.css(), "#8000ff");
        let linear = c.linear();
        assert!((linear[0] - 0.21586).abs() < 0.00001);
        assert_eq!(&linear[1..], &[0., 1., 1.]);
    }
}
