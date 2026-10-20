//! Footer status: which message is visible, and which events may replace it.
//!
//! Messages come from three kinds of source, and the precedence between
//! them is decided here instead of by the callers:
//!
//! - User actions (`report`) replace whatever is shown.
//! - Apply and restore results (`report_operation`) stay visible through
//!   background events and edits, until the user starts something else.
//! - Background events (`report_background`) and the pending-edit hint
//!   (`hint`) never hide an operation result; the hint also never hides a
//!   warning.
//!
//! Preference problems are kept apart and shown next to the main message,
//! so a settings warning and an apply result can both be seen.

/// One status message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) warning: bool,
}

impl Notice {
    pub(crate) fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            warning: false,
        }
    }

    pub(crate) fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            warning: true,
        }
    }

    /// Both messages in order; a warning if either one is.
    pub(crate) fn then(self, next: Notice) -> Self {
        Self {
            text: format!("{} {}", self.text, next.text),
            warning: self.warning || next.warning,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreferencesIssue {
    /// Settings could not be read at startup; defaults are in use. Stays
    /// for the session, because the remembered selections are gone.
    Read(String),
    /// The last save failed. Cleared by the next successful save.
    Save(String),
}

#[derive(Debug, Default)]
pub(crate) struct StatusLine {
    operation: Option<Notice>,
    event: Option<Notice>,
    preferences: Option<PreferencesIssue>,
}

impl StatusLine {
    /// The result of something the user just did.
    pub(crate) fn report(&mut self, notice: Notice) {
        self.operation = None;
        self.event = Some(notice);
    }

    /// An apply or restore result.
    pub(crate) fn report_operation(&mut self, notice: Notice) {
        self.operation = Some(notice);
        self.event = None;
    }

    /// Something that happened without the user asking, such as a scan.
    pub(crate) fn report_background(&mut self, notice: Notice) {
        if self.operation.is_none() {
            self.event = Some(notice);
        }
    }

    /// Guidance about the pending edit.
    pub(crate) fn hint(&mut self, notice: Notice) {
        if self.operation.is_none() && !self.event.as_ref().is_some_and(|event| event.warning) {
            self.event = Some(notice);
        }
    }

    /// The main message, if any.
    pub(crate) fn current(&self) -> Option<&Notice> {
        self.operation.as_ref().or(self.event.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> &str {
        self.current().map_or("", |notice| notice.text.as_str())
    }

    #[cfg(test)]
    pub(crate) fn is_warning(&self) -> bool {
        self.current().is_some_and(|notice| notice.warning)
    }

    #[cfg(test)]
    pub(crate) fn has_operation(&self) -> bool {
        self.operation.is_some()
    }

    pub(crate) fn preferences_read_failed(&mut self, message: String) {
        self.preferences = Some(PreferencesIssue::Read(message));
    }

    /// Records the outcome of a preferences save.
    pub(crate) fn preferences_saved(&mut self, result: Result<(), String>) {
        match result {
            Err(message) => self.preferences = Some(PreferencesIssue::Save(message)),
            Ok(()) => {
                if matches!(self.preferences, Some(PreferencesIssue::Save(_))) {
                    self.preferences = None;
                }
            }
        }
    }

    pub(crate) fn preferences_warning(&self) -> Option<&str> {
        match &self.preferences {
            Some(PreferencesIssue::Read(message) | PreferencesIssue::Save(message)) => {
                Some(message)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_results_survive_background_events_and_hints() {
        let mut status = StatusLine::default();
        status.report_operation(Notice::info("Changes applied."));
        status.report_background(Notice::warning("Display scan failed."));
        status.hint(Notice::info("No pending changes."));
        assert_eq!(status.text(), "Changes applied.");
        status.report(Notice::info("Configuration loaded."));
        assert_eq!(status.text(), "Configuration loaded.");
        assert!(!status.has_operation());
    }

    #[test]
    fn hints_do_not_hide_warnings() {
        let mut status = StatusLine::default();
        status.report(Notice::warning("Could not load backups."));
        status.hint(Notice::info("No pending changes."));
        assert!(status.is_warning());
        status.report_background(Notice::info("Steam accounts refreshed."));
        status.hint(Notice::info("No pending changes."));
        assert_eq!(status.text(), "No pending changes.");
    }

    #[test]
    fn notices_compose_in_order_and_keep_warnings() {
        let notice = Notice::info("Steam accounts refreshed.")
            .then(Notice::warning("Configuration loaded."));
        assert_eq!(
            notice.text,
            "Steam accounts refreshed. Configuration loaded."
        );
        assert!(notice.warning);
    }

    #[test]
    fn save_failures_clear_on_success_but_read_failures_stay() {
        let mut status = StatusLine::default();
        status.preferences_saved(Err("disk full".into()));
        assert_eq!(status.preferences_warning(), Some("disk full"));
        status.preferences_saved(Ok(()));
        assert_eq!(status.preferences_warning(), None);

        status.preferences_read_failed("invalid JSON".into());
        status.preferences_saved(Ok(()));
        assert_eq!(status.preferences_warning(), Some("invalid JSON"));
        // Neither kind affects the main message.
        assert_eq!(status.current(), None);
    }
}
