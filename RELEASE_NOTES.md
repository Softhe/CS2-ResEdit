# CS2 Video Config Editor v2.0.1

Version 2.0.1 adds the requested `1728x1080` 16:10 preset and improves local
release-test reliability on Windows checkouts.

## Highlights

- Added `1728x1080` to the graphical, console, and command-line 16:10 preset list.
- Added regression coverage confirming that the preset resolves to CS2 aspect mode `2`.
- Made the packaged end-to-end fixture produce valid CRLF consistently from either LF or CRLF source checkouts.
- Preserved all v2.0.0 configuration safety, diagnostics, accessibility, and release-integrity behavior.

The v2 ZIP contains the launcher and a `modules` directory. Extract and keep the
complete directory structure together when running the editor.
