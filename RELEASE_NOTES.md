# CS2 Video Config Editor v3.0.1

Version 3.0.1 is a compatibility and validation patch for current CS2 video
configuration files.

## Highlights

- Current CS2 files no longer need the retired `setting.aspectratiomode` entry.
- The editor infers the closest supported aspect family from resolution dimensions.
- Applying settings does not inject the retired field into current-format files.
- Invalid dimensions are rejected before reaching the graphical numeric controls.
- Legacy files that still contain an explicit aspect mode remain fully supported.

The ZIP contains the launcher and a `modules` directory. Extract and keep the
complete directory structure together when running the editor.
