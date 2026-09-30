use crate::models::{DiagnosticDisplay, DiagnosticReport, DisplayInfo};
use crate::video_config::VideoConfigService;
use chrono::Utc;
use std::path::Path;

#[derive(Clone, Copy, Default)]
pub struct DiagnosticService;

impl DiagnosticService {
    pub fn create(
        configs: &VideoConfigService,
        application_version: &str,
        displays: &[DisplayInfo],
        steam_root_count: usize,
        account_count: usize,
        config_path: Option<&Path>,
    ) -> DiagnosticReport {
        let mut status = if config_path.is_none() {
            "Not selected"
        } else {
            "Valid"
        };
        let mut inspection = None;
        let mut error = None;
        if let Some(path) = config_path {
            match configs.inspect(path) {
                Ok(found) => inspection = Some(found),
                Err(err) => {
                    status = "Invalid";
                    error = Some(err.category().to_string());
                }
            }
        }

        DiagnosticReport {
            schema_version: 1,
            generated_utc: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            application_version: application_version.to_string(),
            operating_system: os_description(),
            architecture: std::env::consts::ARCH.to_string(),
            displays: displays
                .iter()
                .map(|d| DiagnosticDisplay {
                    is_primary: d.is_primary,
                    current_width: d.current_width,
                    current_height: d.current_height,
                    mode_count: d.modes.len(),
                })
                .collect(),
            steam_root_count,
            account_count,
            configuration_status: status.to_string(),
            configuration_encoding: inspection.as_ref().map(|i| i.encoding.clone()),
            configuration_has_bom: inspection.as_ref().map(|i| i.has_bom),
            configuration_line_ending: inspection.as_ref().map(|i| i.line_ending.clone()),
            error_category: error,
        }
    }

    pub fn to_json(report: &DiagnosticReport) -> String {
        serde_json::to_string_pretty(report).expect("diagnostic report serializes")
    }

    pub fn to_summary(report: &DiagnosticReport) -> String {
        let mut text = String::new();
        text.push_str(&format!("CS2 ResEdit {}\n", report.application_version));
        text.push_str(&format!("OS: {}\n", report.operating_system));
        text.push_str(&format!("Architecture: {}\n", report.architecture));
        let total_modes: usize = report.displays.iter().map(|d| d.mode_count).sum();
        text.push_str(&format!(
            "Displays: {} ({} reported modes)\n",
            report.displays.len(),
            total_modes
        ));
        text.push_str(&format!("Steam roots: {}\n", report.steam_root_count));
        text.push_str(&format!("Accounts discovered: {}\n", report.account_count));
        text.push_str(&format!("Configuration: {}\n", report.configuration_status));
        if let Some(encoding) = &report.configuration_encoding {
            text.push_str(&format!(
                "Encoding: {encoding}; BOM: {}; lines: {}\n",
                report
                    .configuration_has_bom
                    .map(|b| b.to_string())
                    .unwrap_or_default(),
                report
                    .configuration_line_ending
                    .as_deref()
                    .unwrap_or_default()
            ));
        }
        if let Some(category) = &report.error_category {
            text.push_str(&format!("Error category: {category}\n"));
        }
        text.push_str(
            "No Steam names, identifiers, paths, configuration contents, or preferences are included.",
        );
        text
    }
}

fn os_description() -> String {
    #[cfg(windows)]
    {
        os_info::get().to_string()
    }
    #[cfg(not(windows))]
    {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}
