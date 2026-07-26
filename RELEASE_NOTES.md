# CS2 Video Config Editor v2.0.0

Version 2.0.0 strengthens maintainability, support diagnostics, accessibility
coverage, and release verification while preserving the existing graphical,
console, and preset workflows.

## Highlights

- Focused modules for video configuration, Steam discovery, preferences, and diagnostics.
- Privacy-safe `-ExportDiagnostics` JSON reports with anonymized paths and Steam identifiers.
- Automated WinForms regression checks for responsive layouts, DPI scaling, High Contrast, keyboard navigation, and accessibility contracts.
- Canonical manifest-driven ZIP generation with byte-identical Windows PowerShell 5.1 and PowerShell 7 output.
- Exact verification of every packaged runtime file plus an extracted-package launcher smoke test.
- Optional Authenticode signing helpers and CI enforcement for configured release certificates.
- Correct rounded-card repainting during repeated window resizing.
- Continued atomic configuration updates, backup restore, retention, and encoding preservation.

The v2 ZIP contains the launcher and a `modules` directory. Extract and keep the
complete directory structure together when running the editor.
