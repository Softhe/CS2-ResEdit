use crate::error::{CoreError, Result};
use crate::models::Resolution;
use std::collections::HashMap;
use std::sync::OnceLock;

fn preset_map() -> &'static HashMap<(i32, i32), Resolution> {
    static MAP: OnceLock<HashMap<(i32, i32), Resolution>> = OnceLock::new();
    MAP.get_or_init(|| {
        ResolutionCatalog::presets()
            .iter()
            .map(|p| ((p.width, p.height), *p))
            .collect()
    })
}

/// Curated resolution catalog (43 presets) plus aspect-family helpers.
pub struct ResolutionCatalog;

impl ResolutionCatalog {
    pub fn presets() -> &'static [Resolution] {
        const PRESETS: &[Resolution] = &[
            Resolution::new(640, 480, "4:3", 0),
            Resolution::new(720, 540, "4:3", 0),
            Resolution::new(800, 600, "4:3", 0),
            Resolution::new(960, 720, "4:3", 0),
            Resolution::new(1024, 768, "4:3", 0),
            Resolution::new(1152, 864, "4:3", 0),
            Resolution::new(1280, 960, "4:3", 0),
            Resolution::new(1280, 1024, "5:4", 0),
            Resolution::new(1400, 1050, "4:3", 0),
            Resolution::new(1440, 1080, "4:3", 0),
            Resolution::new(1600, 1200, "4:3", 0),
            Resolution::new(1920, 1440, "4:3", 0),
            Resolution::new(2048, 1536, "4:3", 0),
            Resolution::new(2560, 1920, "4:3", 0),
            Resolution::new(2880, 2160, "4:3", 0),
            Resolution::new(854, 480, "16:9", 1),
            Resolution::new(960, 540, "16:9", 1),
            Resolution::new(1024, 576, "16:9", 1),
            Resolution::new(1152, 648, "16:9", 1),
            Resolution::new(1280, 720, "16:9", 1),
            Resolution::new(1366, 768, "16:9", 1),
            Resolution::new(1600, 900, "16:9", 1),
            Resolution::new(1920, 1080, "16:9", 1),
            Resolution::new(2048, 1152, "16:9", 1),
            Resolution::new(2560, 1440, "16:9", 1),
            Resolution::new(2880, 1620, "16:9", 1),
            Resolution::new(3200, 1800, "16:9", 1),
            Resolution::new(3840, 2160, "16:9", 1),
            Resolution::new(640, 400, "16:10", 2),
            Resolution::new(800, 500, "16:10", 2),
            Resolution::new(960, 600, "16:10", 2),
            Resolution::new(1024, 640, "16:10", 2),
            Resolution::new(1152, 720, "16:10", 2),
            Resolution::new(1280, 800, "16:10", 2),
            Resolution::new(1440, 900, "16:10", 2),
            Resolution::new(1680, 1050, "16:10", 2),
            Resolution::new(1728, 1080, "16:10", 2),
            Resolution::new(1920, 1200, "16:10", 2),
            Resolution::new(2304, 1440, "16:10", 2),
            Resolution::new(2560, 1600, "16:10", 2),
            Resolution::new(2880, 1800, "16:10", 2),
            Resolution::new(3456, 2160, "16:10", 2),
            Resolution::new(3840, 2400, "16:10", 2),
        ];
        PRESETS
    }

    pub fn recommended_preset(mode: i32) -> Result<Resolution> {
        match mode {
            0 => Ok(Resolution::new(1280, 960, "4:3", 0)),
            1 => Ok(Resolution::new(1920, 1080, "16:9", 1)),
            2 => Ok(Resolution::new(1680, 1050, "16:10", 2)),
            _ => Err(CoreError::OutOfRange(format!(
                "Unknown aspect mode '{mode}'."
            ))),
        }
    }

    pub fn parse(value: &str, aspect_mode: Option<i32>) -> Result<Resolution> {
        let invalid = || {
            CoreError::InvalidArgument(
                "Enter a resolution as WIDTHxHEIGHT (320–32768 by 200–32768).".to_string(),
            )
        };
        let input = value.trim();
        let sep = input
            .char_indices()
            .find(|(_, c)| matches!(c, 'x' | 'X' | '×'))
            .ok_or_else(invalid)?;
        let width: i32 = input[..sep.0].trim().parse().map_err(|_| invalid())?;
        let height: i32 = input[sep.0 + sep.1.len_utf8()..]
            .trim()
            .parse()
            .map_err(|_| invalid())?;
        if !(320..=32768).contains(&width) || !(200..=32768).contains(&height) {
            return Err(invalid());
        }
        let preset = preset_map().get(&(width, height)).copied();
        let mode = match aspect_mode {
            Some(m) => {
                if !(0..=2).contains(&m) {
                    return Err(CoreError::OutOfRange(format!("Unknown aspect mode '{m}'.")));
                }
                m
            }
            None => match preset {
                Some(preset) => preset.mode,
                None => Self::automatic_aspect_mode(width, height)?,
            },
        };
        Ok(Resolution::new(
            width,
            height,
            Self::mode_name_static(mode, width, height)?,
            mode,
        ))
    }

    pub fn automatic_aspect_mode(width: i32, height: i32) -> Result<i32> {
        if !(320..=32768).contains(&width) || !(200..=32768).contains(&height) {
            return Err(CoreError::InvalidArgument(
                "Resolution dimensions are outside the supported range.".to_string(),
            ));
        }
        let ratio = width as f64 / height as f64;
        Ok([(0, 4.0 / 3.0), (1, 16.0 / 9.0), (2, 16.0 / 10.0)]
            .into_iter()
            .min_by(|a, b| {
                (a.1 - ratio)
                    .abs()
                    .partial_cmp(&(b.1 - ratio).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(mode, _)| mode)
            .unwrap_or(0))
    }

    pub fn mode_name(mode: i32, width: i32, height: i32) -> Result<&'static str> {
        Self::mode_name_static(mode, width, height)
    }

    fn mode_name_static(mode: i32, width: i32, height: i32) -> Result<&'static str> {
        if width <= 0 || height <= 0 {
            return Err(CoreError::InvalidArgument(
                "Resolution dimensions must be positive.".to_string(),
            ));
        }
        match mode {
            0 if ((width as f64 / height as f64) - 1.25).abs() < 0.03 => Ok("5:4"),
            0 => Ok("4:3"),
            1 => Ok("16:9"),
            2 => Ok("16:10"),
            _ => Err(CoreError::OutOfRange(format!(
                "Unknown aspect mode '{mode}'."
            ))),
        }
    }
}
