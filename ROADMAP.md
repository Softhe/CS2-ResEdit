# Roadmap

Version 2.0.0 replaced the C# application with Rust. Further work should preserve the file-format handling, backup behavior, and external-change checks described in the [README](README.md).

## Next priorities

- Add repeatable Windows UI checks at 1280x960 with 100%, 125%, and 150% scaling. Geometry tests do not prove that native text, chrome, and taskbar rendering look correct.
- Move the remaining blocking file I/O off the UI thread. Discovery runs in the background and preference saves are debounced; configuration loads, applies, and restores still run synchronously. They touch one small file, so this matters mainly for slow or network drives, and it needs a loading state for every navigation path.
- Smoke-test the Direct3D 12 fallback in CI. It was verified locally by breaking the OpenGL driver; hosted runners have not been checked for a usable Direct3D 12 adapter.
- Consider a resizable window. The layout still assumes a fixed-size viewport with equal-height cards.

The following earlier priorities are complete and covered by tests: separating configuration editing from egui rendering (`crates/app/src/app/editor.rs`, explicit `EditState`, tested without the UI), typed footer notices with one precedence policy (`crates/app/src/app/status.rs`), generation-tagged scan requests with stale-result rejection, and a dependency advisory check with a recorded tool version and exception policy (`deny.toml`, weekly `advisories.yml`).

## Possible later work

- Command-line account listing and configuration editing.
- Windows ARM64 builds.
- Authenticode signing with a trusted publisher certificate.
- UI resources that support localization.

These are candidates, not commitments for a particular release. Linux CI checks core behavior and platform-specific compilation; it does not establish support for a Linux desktop release.
