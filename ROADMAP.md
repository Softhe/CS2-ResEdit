# Roadmap

Version 2.0.0 replaces the C# application with Rust. Further work should preserve the file-format handling, backup behavior, and external-change checks described in the [README](README.md).

## Next priorities

- Add repeatable Windows UI checks at 1280x960 with 100%, 125%, and 150% scaling. Geometry tests do not prove that native text, chrome, and taskbar rendering look correct.
- Separate configuration navigation and edit state from egui rendering. Keep failed loads transactional and test transitions through the same interface the UI uses.
- Separate scan notices from apply and restore results so a refresh cannot hide an important file-operation message.
- Add a dependency advisory check with a recorded tool version and a policy for exceptions.

## Possible later work

- Command-line account listing and configuration editing.
- Windows ARM64 builds.
- Authenticode signing with a trusted publisher certificate.
- UI resources that support localization.
- Generation-tagged scan requests and bounded worker shutdown.

These are candidates, not commitments for a particular release. Linux CI checks core behavior and platform-specific compilation; it does not establish support for a Linux desktop release.
