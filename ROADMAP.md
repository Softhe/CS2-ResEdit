# Roadmap

Version 2.0.0 replaces the C# application with Rust. Further work should preserve the file-format handling, backup behavior, and external-change checks described in the [README](README.md).

## Next priorities

- Add repeatable Windows UI checks at 1280x960 with 100%, 125%, and 150% scaling. Geometry tests do not prove that native text, chrome, and taskbar rendering look correct.
- Move blocking file I/O off the UI thread. Discovery already runs in the background; configuration loads, applies, restores, and preference saves still run synchronously.
- Consider a resizable window. The layout still assumes a fixed-size viewport with equal-height cards.

The following earlier priorities are complete and covered by tests: separating configuration navigation and edit state from egui rendering (`crates/app/src/app/controller.rs`, explicit `EditState`), separating scan notices from apply/restore results (sticky footer status plus inline form feedback), generation-tagged scan requests with stale-result rejection, and a dependency advisory check with a recorded tool version and exception policy (`deny.toml`, weekly `advisories.yml`).

## Possible later work

- Command-line account listing and configuration editing.
- Windows ARM64 builds.
- Authenticode signing with a trusted publisher certificate.
- UI resources that support localization.

These are candidates, not commitments for a particular release. Linux CI checks core behavior and platform-specific compilation; it does not establish support for a Linux desktop release.
