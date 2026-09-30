# CS2 ResEdit codebase analysis

This report records the September 29 review of the Rust workspace. Later changes are listed in the [changelog](CHANGELOG.md).

## Assessment

The core has a narrow configuration-editing interface and tests for encoding, line endings, backups, and external file changes. Steam and display discovery run away from the UI thread. The UI is readable, but `ResEditApp` still owns rendering, navigation, persistence, scans, recovery, and diagnostics.

Before this review, formatting, strict Clippy checks, and 54 tests passed. That baseline did not cover malformed Unicode backup names, invalid UTF-8 after a BOM, concurrent guarded writers, excessive parser nesting, failed navigation, selected custom-file restoration, or scan timeouts.

The workspace's Git pointer referred to Linux worktree metadata that Windows could not access. The review therefore inspected the source directly. The later 2.0.0 release uses a fresh checkout rather than modifying that pointer.

## File-safety findings

- Backup-name validation sliced UTF-8 before proving that timestamp text was ASCII. It now rejects non-ASCII suffixes before slicing.
- BOM-prefixed UTF-8 used lossy decoding. Both BOM and non-BOM paths now reject invalid UTF-8 without writing or creating a backup.
- Temporary files and backups used predictable names with truncation or an existence check before creation. They now reserve files with `create_new`, retry collisions, sync completed contents, and remove partial files after failures.
- Guarded writes could accept the same snapshot concurrently. In-process writes are now serialized, backup bytes come from the checked snapshot, and another check runs before replacement. An unrelated process can still write during the final scheduling window; replacement is not a cross-process compare-and-swap operation.
- Retention counted only parseable backups, so corrupt editor-owned files could accumulate. It now inventories recognized filenames independently of content validity.
- KeyValues parsing had no recursion limit. It now rejects nesting beyond 128 levels.
- Path deduplication lowercased paths on every platform. Path keys now preserve case on case-sensitive systems.
- Public aspect helpers accepted invalid dimensions. They now reject zero and negative dimensions.

## Application findings

- Failed navigation cleared a valid configuration before validating its replacement. Loading is now transactional.
- Preferences remembered recent files but not the selected custom file. The schema now stores `LastConfigPath` and still accepts older schema-1 settings.
- Hung scans kept the UI on a 50 ms repaint loop. After ten seconds, a timeout notice appears and polling slows to the normal two-second interval. A later result can still be accepted.
- Diagnostics export hid I/O errors. It now reports the error or export location through a testable helper.
- The window icon loaded the README screenshot. It now uses the dedicated icon source; the executable resource still uses the ICO file.

## Build findings

- The publish script resolved the project root but ran Cargo in the caller's directory. It now passes the workspace manifest explicitly.
- The compiler was selected through floating `stable`. Workspace metadata and the toolchain file now pin Rust 1.98.0.
- Both crates declare repository metadata and disable crates.io publication.
- Release-note validation searched broad prose for words such as `planned`. It now checks the release heading and draft marker.
- Dependabot checks Cargo and GitHub Actions weekly.

## Code quality

`VideoConfigService` keeps decoding, validation, snapshot handling, replacement, and recovery behind read, inspect, guarded-update, backup-listing, and guarded-restore methods. Tests use the same interface as the application.

Preferences and Steam services are small enough to follow without tracing many modules. The limited KeyValues parser fits its login-user-data use case. Its depth limit bounds recursive input.

The largest maintenance cost is `ResEditApp`. It stores loaded state, derived pending state, formatted labels, modal state, workers, and preferences together. Changes often require coordinated edits in both rendering and state-transition code.

No direct Cargo dependency was clearly unused. Duplicate dependency versions came from transitive GUI and platform dependencies, not removable direct declarations. Comments explaining file preservation and Windows FFI are useful. Historical C# parity comments can be shortened when they no longer explain a current constraint.

## Recommended direction

Separate file navigation and editing state from egui rendering. A controller should own the selected path, snapshot, pending edit, and transition outcomes. Rendering should request operations such as select, reload, apply, and restore. Tests should exercise that controller without constructing the whole UI.

Then replace parallel pending fields with explicit states for no file, invalid input, unchanged values, and changed values. Keep operation results separate from background-scan notices.

A scan coordinator with generation-tagged requests could reject stale results and support bounded shutdown. The current timeout fixes the repaint-loop problem without implementing cancellation.

Add a dependency advisory check after selecting a recorded tool version and exception policy. The review environment had no advisory tool, so it did not claim that dependency vulnerability scanning had passed.

Keep source checks, packaged-binary checks, native UI checks, and live game behavior distinct. Passing one does not establish the others.
