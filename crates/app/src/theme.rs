//! Selectable palettes based on the approved application design previews.

use cs2_resedit_core::UiTheme;
use egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub window: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub input: Color32,
    pub preview: Color32,
    pub preview_fill: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_soft: Color32,
    pub warning: Color32,
    pub high_contrast: bool,
    pub dark_mode: bool,
    pub radius: u8,
    pub icon_color: Color32,
}

impl ThemePalette {
    /// Recolor the bundled icon while preserving its alpha mask and shading.
    /// This changes the running window icon, not the executable's shell resource.
    pub fn window_icon(&self) -> Option<std::sync::Arc<egui::IconData>> {
        static SOURCE: std::sync::OnceLock<Option<egui::IconData>> = std::sync::OnceLock::new();
        let source = SOURCE
            .get_or_init(|| {
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/cs2-resedit-icon-source.png"
                ))
                .ok()
            })
            .as_ref()?;
        let mut icon = source.clone();
        for pixel in icon.rgba.as_chunks_mut::<4>().0 {
            let brightness = u16::from(pixel[0].max(pixel[1]).max(pixel[2]));
            for (channel, accent) in pixel[..3].iter_mut().zip([
                self.icon_color.r(),
                self.icon_color.g(),
                self.icon_color.b(),
            ]) {
                *channel = ((u16::from(accent) * brightness + 127) / 255) as u8;
            }
        }
        Some(std::sync::Arc::new(icon))
    }

    pub fn balanced() -> Self {
        Self {
            window: Color32::from_rgb(14, 16, 18),
            card: Color32::from_rgb(25, 29, 32),
            card_hover: Color32::from_rgb(57, 63, 68),
            input: Color32::from_rgb(43, 48, 52),
            preview: Color32::from_rgb(9, 11, 13),
            preview_fill: Color32::from_rgb(82, 58, 32),
            border: Color32::from_rgb(105, 115, 122),
            border_strong: Color32::from_rgb(160, 171, 178),
            text: Color32::from_rgb(250, 248, 244),
            muted: Color32::from_rgb(216, 210, 201),
            disabled: Color32::from_rgb(174, 168, 159),
            accent: Color32::from_rgb(248, 157, 28),
            accent_text: Color32::from_rgb(36, 24, 10),
            accent_hover: Color32::from_rgb(255, 180, 71),
            accent_pressed: Color32::from_rgb(216, 127, 5),
            accent_soft: Color32::from_rgb(78, 54, 25),
            warning: Color32::from_rgb(255, 207, 122),
            high_contrast: false,
            dark_mode: true,
            radius: 4,
            icon_color: Color32::from_rgb(248, 157, 28),
        }
    }

    /// High-contrast fallback mirroring `SystemColors` usage in the C# theme.
    pub fn high_contrast() -> Self {
        Self {
            // Approximations of the Win32 system colors used by the C# build.
            window: Color32::WHITE,
            card: Color32::from_rgb(240, 240, 240),
            card_hover: Color32::from_rgb(229, 241, 251),
            input: Color32::WHITE,
            preview: Color32::WHITE,
            preview_fill: Color32::from_rgb(0, 120, 215),
            border: Color32::BLACK,
            border_strong: Color32::from_rgb(0, 120, 215),
            text: Color32::BLACK,
            muted: Color32::BLACK,
            disabled: Color32::GRAY,
            accent: Color32::from_rgb(0, 120, 215),
            accent_text: Color32::WHITE,
            accent_hover: Color32::from_rgb(0, 102, 204),
            accent_pressed: Color32::from_rgb(0, 120, 215),
            accent_soft: Color32::from_rgb(240, 240, 240),
            warning: Color32::from_rgb(0, 120, 215),
            high_contrast: true,
            dark_mode: false,
            radius: 4,
            icon_color: Color32::WHITE,
        }
    }

    pub fn current(high_contrast: bool) -> Self {
        if high_contrast {
            Self::high_contrast()
        } else {
            Self::for_theme(UiTheme::Glacier)
        }
    }

    pub fn for_theme(theme: UiTheme) -> Self {
        let mut palette = Self::balanced();
        let (
            window,
            card,
            input,
            preview,
            fill,
            border,
            strong,
            text,
            muted,
            accent,
            accent_text,
            radius,
        ) = match theme {
            UiTheme::Glacier => (
                0x0c1824, 0x142331, 0x1d3040, 0x09131d, 0x164352, 0x5d788b, 0x698597, 0xf1f8fc,
                0xbad0df, 0x66d5ee, 0x09202a, 9,
            ),
            UiTheme::Paper => (
                0xf6f5ed, 0xfffef8, 0xf0f2e9, 0xd5e3dc, 0x98c8b5, 0x8fa69a, 0x71877b, 0x202c2a,
                0x455650, 0x146b62, 0xffffff, 9,
            ),
            UiTheme::Monolith => (
                0x101514, 0x1b2320, 0x28322d, 0x0b0f0e, 0x334534, 0x7c8e82, 0x849689, 0xf1f7f2,
                0xbbc9bf, 0xc4ef77, 0x172019, 0,
            ),
            UiTheme::Canopy => (
                0x0b1512, 0x162820, 0x20342a, 0x11211b, 0x28513c, 0x587565, 0x819d8c, 0xf0f8f2,
                0xb6cbbd, 0x7de1b1, 0x0a2215, 10,
            ),
        };
        fn color(hex: u32) -> Color32 {
            Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
        }
        palette.window = color(window);
        palette.card = color(card);
        palette.input = color(input);
        palette.preview = color(preview);
        palette.preview_fill = color(fill);
        palette.border = color(border);
        palette.border_strong = color(strong);
        palette.text = color(text);
        palette.muted = color(muted);
        palette.disabled = color(muted);
        palette.accent = color(accent);
        palette.icon_color = if theme == UiTheme::Paper {
            Color32::WHITE
        } else {
            palette.accent
        };
        palette.accent_text = color(accent_text);
        palette.accent_soft = color(fill);
        palette.card_hover = color(input).lerp_to_gamma(color(accent), 0.12);
        palette.accent_hover = color(accent).lerp_to_gamma(color(text), 0.12);
        palette.accent_pressed = color(accent).lerp_to_gamma(color(window), 0.12);
        palette.warning = if theme == UiTheme::Paper {
            color(0x854500)
        } else {
            color(0xffcf7a)
        };
        palette.dark_mode = theme != UiTheme::Paper;
        palette.radius = radius;
        palette
    }

    pub fn apply(&self, ctx: &egui::Context, compact: bool) {
        let mut style = (*ctx.style()).clone();
        // Compact logical spacing on high-DPI desktops with a small work area.
        let (body, button, heading, small, spacing, padding) = if compact {
            (13.0, 13.0, 18.0, 11.0, 3.0, 4.0)
        } else {
            (15.5, 15.0, 22.0, 13.0, 9.0, 9.0)
        };
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(body, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(button, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(heading, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(small, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(small, egui::FontFamily::Monospace),
        );
        style.spacing.item_spacing = egui::vec2(10.0, spacing);
        style.spacing.button_padding = egui::vec2(15.0, padding);
        style.spacing.indent = 20.0;
        style.visuals.dark_mode = self.dark_mode;
        style.visuals.override_text_color = Some(self.text);
        style.visuals.window_corner_radius = egui::CornerRadius::same(self.radius);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(self.radius.min(6));
        }
        style.visuals.window_fill = self.window;
        style.visuals.panel_fill = self.window;
        style.visuals.faint_bg_color = self.card;
        style.visuals.extreme_bg_color = self.input;
        style.visuals.code_bg_color = self.input;
        style.visuals.text_edit_bg_color = Some(self.input);
        style.visuals.widgets.noninteractive.bg_fill = self.card;
        style.visuals.widgets.noninteractive.weak_bg_fill = self.card;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        style.visuals.widgets.inactive.bg_fill = self.card;
        style.visuals.widgets.inactive.weak_bg_fill = self.input;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, self.border_strong);
        style.visuals.widgets.hovered.bg_fill = self.card_hover;
        style.visuals.widgets.hovered.weak_bg_fill = self.card_hover;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5_f32, self.text);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5_f32, self.accent);
        style.visuals.widgets.active.bg_fill = self.accent_soft;
        style.visuals.widgets.active.weak_bg_fill = self.accent_soft;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5_f32, self.text);
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5_f32, self.accent);
        style.visuals.widgets.open.bg_fill = self.card_hover;
        style.visuals.widgets.open.weak_bg_fill = self.card_hover;
        style.visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.5_f32, self.accent);
        style.visuals.disabled_alpha = 0.72;
        style.visuals.selection.bg_fill = self.accent_soft;
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, self.accent);
        style.visuals.hyperlink_color = self.accent;
        style.visuals.warn_fg_color = self.warning;
        style.visuals.error_fg_color = self.warning;
        ctx.set_style(style);
    }
}

/// WCAG-style contrast ratio between two sRGB colors.
/// Exercised by the theme unit test below.
#[cfg(test)]
pub fn contrast(first: Color32, second: Color32) -> f64 {
    fn luminance(color: Color32) -> f64 {
        fn linear(value: u8) -> f64 {
            let channel = f64::from(value) / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    let a = luminance(first);
    let b = luminance(second);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_icons_follow_theme_accents_and_preserve_transparency() {
        let source = eframe::icon_data::from_png_bytes(include_bytes!(
            "../../../assets/cs2-resedit-icon-source.png"
        ))
        .unwrap();
        let mut variants = Vec::new();
        for theme in UiTheme::ALL {
            let palette = ThemePalette::for_theme(theme);
            if theme == UiTheme::Paper {
                assert_eq!(palette.icon_color, Color32::WHITE);
            }
            let icon = palette.window_icon().expect("bundled icon");
            assert_eq!((icon.width, icon.height), (source.width, source.height));
            for (actual, original) in icon
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .zip(source.rgba.as_chunks::<4>().0)
            {
                assert_eq!(actual[3], original[3]);
                let brightness = u16::from(original[0].max(original[1]).max(original[2]));
                for (actual, accent) in actual[..3].iter().zip([
                    palette.icon_color.r(),
                    palette.icon_color.g(),
                    palette.icon_color.b(),
                ]) {
                    assert_eq!(
                        *actual,
                        ((u16::from(accent) * brightness + 127) / 255) as u8
                    );
                }
            }
            assert!(variants.iter().all(|previous| previous != &icon.rgba));
            variants.push(icon.rgba.clone());
        }
    }

    #[test]
    fn approved_themes_have_readable_text_and_correct_mode() {
        for theme in UiTheme::ALL {
            let palette = ThemePalette::for_theme(theme);
            for background in [
                palette.window,
                palette.card,
                palette.input,
                palette.preview_fill,
            ] {
                assert!(contrast(palette.text, background) >= 4.5, "{theme:?} text");
            }
            for background in [palette.window, palette.card, palette.input] {
                assert!(
                    contrast(palette.muted, background) >= 4.5,
                    "{theme:?} secondary text"
                );
            }
            assert!(
                contrast(palette.accent_text, palette.accent) >= 4.5,
                "{theme:?} button text"
            );
            let ctx = egui::Context::default();
            palette.apply(&ctx, false);
            assert_eq!(ctx.style().visuals.dark_mode, theme != UiTheme::Paper);
            assert_eq!(ctx.style().visuals.override_text_color, Some(palette.text));
        }
        assert_eq!(
            ThemePalette::current(false),
            ThemePalette::for_theme(UiTheme::Glacier)
        );
        assert!(ThemePalette::current(true).high_contrast);
    }

    #[test]
    fn readable_text_sizes_are_applied() {
        let theme = ThemePalette::balanced();
        let ctx = egui::Context::default();
        theme.apply(&ctx, false);
        let styles = &ctx.style().text_styles;
        for (style, expected) in [
            (egui::TextStyle::Body, 15.5),
            (egui::TextStyle::Button, 15.0),
            (egui::TextStyle::Heading, 22.0),
            (egui::TextStyle::Small, 13.0),
            (egui::TextStyle::Monospace, 13.0),
        ] {
            let size = styles[&style].size;
            assert!(
                (size - expected).abs() < f32::EPSILON,
                "{style:?} size {size} != {expected}"
            );
        }
    }

    #[test]
    fn balanced_theme_meets_contrast_targets() {
        let theme = ThemePalette::balanced();
        assert_eq!(theme.window, Color32::from_rgb(14, 16, 18));
        assert_eq!(theme.card, Color32::from_rgb(25, 29, 32));
        assert_eq!(theme.input, Color32::from_rgb(43, 48, 52));
        assert_eq!(theme.accent, Color32::from_rgb(248, 157, 28));
        assert!(contrast(theme.text, theme.card) >= 4.5);
        assert!(contrast(theme.text, theme.input) >= 4.5);
        assert!(contrast(theme.muted, theme.card) >= 4.5);
        assert!(contrast(theme.muted, theme.window) >= 4.5);
        assert!(contrast(theme.disabled, theme.window) >= 4.5);
        assert!(contrast(theme.border, theme.window) >= 3.0);
        assert!(contrast(theme.border_strong, theme.input) >= 3.0);
        assert!(contrast(theme.accent, theme.input) >= 3.0);
        assert!(contrast(theme.accent_text, theme.accent) >= 4.5);
    }
}
