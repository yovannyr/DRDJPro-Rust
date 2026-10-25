# Local library storage — SQLite

The current storage backend is SQLite, replacing the published JSON writer. The current catalog persists stable track IDs/paths plus title, artist, album, duration and metadata-inspection state. It also persists flat crates and track membership; constant-BPM beatgrids are also stored, and schema 5 adds hot-cue data storage. Hot-cue GUI/trigger integration, deck/session restoration and SQL-backed table searching remain open. Decks start unloaded/paused unless explicit CLI tracks were supplied.

## Location and compatibility

- Windows: `%LOCALAPPDATA%/DrDJPro/library.sqlite3`.
- macOS: `$HOME/Library/Application Support/DrDJPro/library.sqlite3` (path logic only; no macOS acceptance).
- Other platforms: `$XDG_DATA_HOME/DrDJPro/library.sqlite3`, falling back to `$HOME/.local/share/DrDJPro/library.sqlite3` (unqualified).
- `DRDJ_LIBRARY_PATH` may specify an absolute database filename. Relative paths are rejected visibly.
- An existing override ending in `.json` remains supported: that exact file is the legacy source and the database uses the same basename with `.sqlite3`. For other database filenames, the sibling with extension `.json` is the migration source. An arbitrary old JSON filename without `.json` is not guessed or overwritten; choose a distinct database path with the documented sibling convention.

The database uses application ID `0x4452444a` and `PRAGMA user_version=5`. `tracks` is a STRICT table with a positive unique integer ID, Unicode path text and a unique `path_key` computed using the existing platform path-identity rules. `library_meta` records the initial migration source. The separate STRICT `track_metadata` table references track IDs with cascading deletion and stores bounded Unicode tags, nullable millisecond duration, its decoded/header origin, inspection state and errors. Existing track IDs are preserved, and additions receive IDs above the maximum. SQLite schema versions and legacy JSON versions are separate contracts.

The existing 100,000-track and 5,000-path import caps remain safeguards. SQLite rows are bounded/validated on restoration; the legacy reader retains its 32 MiB input limit. SQLite catalogs no longer inherit the old JSON writer's 32 MiB serialized-output limit. These bounds are not scale/performance acceptance.

## One-time JSON migration

A new or transactionally uninitialized database reads the sibling version-1 JSON catalog. If its primary is damaged/missing, a valid JSON `.bak` may be used. A newer JSON version is preserved and rejected, even if an older backup exists. Invalid IDs, duplicates, unsupported/relative paths and oversized JSON are rejected before database creation. JSON and JSON backup bytes are never modified or removed.

Schema creation, the migration-source marker, application/schema versions and imported rows commit in one SQLite transaction. The published JSON version-1 reader still accepts only its original ID/path data; metadata is not written back into JSON. Failure rolls back all of them. An uninitialized file left by interrupted initialization can be retried. Once initialized, SQLite is authoritative: retained JSON is never imported again, even if later changed or corrupted. A missing/damaged SQLite database with an existing SQLite backup restores that backup before considering legacy data; an unrecoverable initialized database is never silently recreated from stale JSON.

All legacy reader and migration tests use temporary profiles; the developer's actual library was not opened or changed by testing.

## Worker, transactions and backup

The existing `library-storage` worker owns its SQLite connection and all database/file operations. The GUI uses the existing bounded command channel; no connection is shared with GUI or audio callbacks. A database OS lock and the matching legacy-profile OS lock exclude both current and JSON-era application writers. The lock files can remain on disk; exclusivity belongs to open OS handles. SQLite still enforces its own transaction locks against other SQLite connections.

The current single-worker architecture uses rollback journaling (`DELETE`) with `synchronous=EXTRA`, foreign keys enabled and trusted schema disabled. WAL is unnecessary for this slice's single connection. A 200 ms SQLite busy timeout prevents an external transaction from indefinitely blocking each lock attempt; failures use the existing visible NOT saved / Retry Save flow.

Saves validate the snapshot, compare IDs/paths and mutate changed/new/removed rows inside an IMMEDIATE transaction. No-change rows are retained. A transaction failure restores the preceding committed set, including earlier removals/inserts in that transaction. Snapshot construction/filtering/sorting still runs on the GUI control plane, and reading/comparing the catalog plus full backups still costs work proportional to catalog size; this slice does not claim scalable paged SQL browsing or improved large-library latency.

Before a save, SQLite's online backup API writes the previous committed state into a unique staged file. The complete backup is synced and renamed over `.sqlite3.bak`; raw copies of a live database are not used. A backup may be one successful save older. Normal shutdown flushes the latest pending snapshot on the storage thread and joins it; final failure returns an error exit.

SQLite handles normal interrupted transactions with its journal. On separately damaged primary files, a validated DrDJ/version-compatible SQLite backup can be restored. Damaged primary bytes are first retained in a unique `.damaged.stage-*` file. Foreign or newer-version databases are never replaced by an older backup. If damage coexists with unresolved SQLite journal/WAL sidecars, automatic backup replacement is refused and the originals remain intact. Unrecoverable startup failures visibly enter session-only behavior; subsequent imports cannot overwrite the damaged catalog. Staging files left by abrupt interruption are not treated as committed catalogs.

Missing files retain their ID/path and show MISSING; they are not removed. Loading one reports an error and preserves the installed deck track. Reimporting a restored file at the same path refreshes availability without changing identity. Filesystem watchers and relocation tools remain open.


## Track metadata (introduced in schema 2)

Symphonia, already used for audio, inspects ID3/container tags and header frame/rate information on import/load workers. A missing title falls back to the filename in the table; absent artist/album and unknown duration display as an em dash. File paths remain visible/searchable. Title, artist, album and duration have sort controls; duration sorting is numeric, with unknown durations first in ascending order. IDs remain independent of display order/filtering. A cached path-to-row index avoids scanning the whole catalog for each imported metadata result.

Header-derived durations are conservative estimates displayed with `~`. Loading a track completely replaces that value with decoded PCM length and removes the approximation marker; that origin is persisted. This does not establish musical/gapless duration accuracy across arbitrary files. Tag strings/errors are limited to 512 Unicode scalar values and stripped of control characters during inspection; the reader requests a 512 KiB per-tag metadata limit and skips visuals. Duration storage is bounded to seven days; longer/unknown header durations remain unknown. There is no full PCM decode during ordinary import. An unreadable header retains the path, unknown fields and an explicit error (TYPE column `?`, accessible row description); actual load decoding still decides whether the deck can install a track.

SQLite schemas 1–3 upgrade to 4 in an IMMEDIATE transaction after taking a compatible backup. Schema 1 gains metadata/crate/grid tables; schema 2 keeps metadata and gains crate/grid tables; schema 3 keeps metadata/crates and gains grid storage. A failed upgrade retains the original version and existing IDs/paths/metadata. Existing available tracks without cached inspection results are read on the storage worker and their results saved before restoration completes; missing files keep unknown or previously cached fields. Cached metadata is reused on subsequent startups, including for now-missing media. Reimporting the same path refreshes its tags without assigning a new ID. There is no tag editing, automatic file-change watcher, artwork, metadata conflict editor or corpus-wide accuracy qualification in this slice.

## Flat crates (schema 3)

`crates` stores stable IDs, printable trimmed names and unique normalized name keys; `crate_tracks` stores unique crate/track pairs with cascading foreign keys and a track lookup index. `library_meta.last_crate_id` is a durable high-water mark, retained even after deleting all crates, so committed crate IDs are not reused after restart. Names contain 1–128 characters after trimming, reject control characters and duplicates under lowercase matching, and reserve All tracks for the built-in view. Limits are 1,024 crates and 1,000,000 memberships; they are safeguards, not scale benchmarks. JSON migration still only reads the original ID/path schema and never writes crate data to retained JSON.

The left navigation displays named crates and unfiltered membership counts plus All tracks. Selecting a crate filters the table by track ID; text search and current sorting apply within that scope, with scoped total/missing counts. Hidden track selections are cleared. New/Rename use a Save/Cancel panel; duplicate or invalid names keep the editor open with a status error. A separate target dropdown supports adding the selected library track to a crate without switching the table filter. Repeated adds are idempotent. Remove only drops the selected membership. Delete requires confirmation, and changing crate navigation cancels a pending confirmation so it cannot act on a stale selection. Deletion removes only that crate/memberships, leaves all library entries/files and playback intact, and returns to All tracks when the active crate is deleted.

File imports capture the active crate when the worker starts. Accepted paths are imported globally and assigned to that captured crate; later filter callbacks cannot redirect membership. Assignment capacity is checked before a bulk set is extended. If membership assignment fails, successfully imported global tracks remain available and the status reports the failure. Direct deck file loading adds a global library entry without implicitly changing crate membership. Active view, target selection and editor state are session-only; startup returns to All tracks.

Crate changes use the existing background save/Retry Save/latest-snapshot close path. Names, high-water mark, memberships, tracks and metadata commit together. The bounded crate relationship snapshot is currently replaced within that transaction; large relationship sets and full online backups are not performance-qualified. Pure crate logic has no file/audio operations. Nested crates, rule-based smart crates, multi-selection, drag/drop, manual member ordering, external library compatibility and autoplay are not implemented by this slice.

## Transactional manual beatgrids (schema 4)

`track_grids` stores one optional constant-BPM 4/4 grid per track ID: finite BPM in 40–240 and an integer nanosecond anchor in 0–7 days, with a cascading track foreign key. There is no row for an explicitly removed grid. Original JSON is unchanged and does not receive grid fields. Track/grid/metadata/crate updates commit together, and the previous-good backup includes them. A failed grid write rolls back the whole database transaction. The grid is associated with canonical library path/ID, not a transient filename label or selected table row; it is not an audio-content fingerprint.

Each deck opens an isolated draft of its installed grid, or a new draft anchored at its current playhead if no grid exists. The initial whole-BPM seed/suggestion controls are only active for a new grid; the editor supports decimal points/commas and an explicit suggestion-to-draft button. BPM, Set 1 at playhead, clamped ±10 ms shifts and Clear affect the draft preview only. Existing zoom and explicit waveform seeking remain available. Clear is staged and still needs Save; Set 1 recreates a cleared draft. Invalid BPM cannot replace a valid draft or commit to a deck. Cancel discards it without changing the catalog or active grid. Saving applies the validated local grid and queues background catalog persistence; the footer's Saved/NOT saved/Retry Save state remains the durability indication. Session-only mode says so explicitly.

Visible downbeat labels are numbered relative to the chosen first downbeat (bar 1, marked red in UI code); bars before it can be 0 or negative. Beat positions are recomputed from anchor/index rather than rounded cumulatively. Preview is marked DRAFT; the active-grid flag continues to describe the committed engine state. Editing grid timing does not change playback rate, pitch, samples, mixer gain or play/pause state. It is not Sync, quantization or automatic beat/downbeat detection.

Save checks both the installed track's prepared-source identity and the captured original grid. Reloading/replacing a track discards its old draft; failed/cancelled loading preserves the draft. On normal application loads and library restoration, cached grids are applied to the installed matching track. Raw engine source installation still starts with no grid; the controller is responsible for applying durable library data. An anchor outside a changed file's decoded duration is not applied and is reported for manual repair. Compatible loaded decks of the same canonical track receive the same committed grid, while simultaneous drafts use optimistic conflict detection rather than overwriting one another. A differing loaded counterpart duration can produce an explicit other-deck warning; content-change detection/fingerprinting remains open.

Only committed values are persisted. Drafts, editor state and unsaved invalid text are not restored. Schema-3 upgrade/failure tests preserve metadata/crate membership; restart tests preserve fractional BPM/nanosecond anchors and explicit deletion. Variable-tempo/warp markers, variable meter, musical correctness, native rendering/interaction approval and physical listening are outside this bounded slice.

## Automated acceptance and limits

Tests cover primary/backup JSON migration, byte preservation, Unicode and missing paths, stable IDs/additions, one-time migration, invalid/newer legacy data, transactional schema rollback/retry, partial-write rollback/retry, foreign/newer SQLite preservation, SQLite backup recovery, unrecoverable-data preservation, old/new writer locks and latest-snapshot shutdown flush.

Headless controller tests migrate real MP3/WAV entries, recreate controller/engine/storage, load the same restored ID into A/B paused, retain missing entries and preserve installed audio on error. SQLite writer contention exercises visible failure and Retry Save.

An isolated child process writes a committed catalog, starts an uncommitted transaction with cache spills, and is forcibly terminated. Reopening in the parent restores the committed rows through SQLite's hot journal, releases OS locks and passes `PRAGMA integrity_check`. This is actual process-termination evidence, not physical power-loss qualification.

No visual/native-dialog, physical listening, device-latency, maximum-scale, macOS or Linux acceptance is claimed. Dependency `rusqlite` 0.41.0 uses bundled SQLite and the backup feature, with default optional features disabled; serde/serde_json remain only for legacy migration and tempfile remains test-only.

References: [SQLite pragmas](https://www.sqlite.org/pragma.html), [online backup API](https://www.sqlite.org/backup.html), [rusqlite source](https://github.com/rusqlite/rusqlite).

## Hot-cue storage foundation (2026-10-25)

Schema 5 adds STRICT `track_hot_cues`, keyed by stable track ID and zero-based slot 0–7, with cascading deletion. Each occupied slot stores an integer nanosecond position (0 through seven days), an optional empty Unicode name (at most 128 scalar characters, no controls), and a 24-bit RGB value. Slots are independent; clearing one never compacts or renumbers others. The typed model has private validated fields. Restoration additionally rejects invalid/orphaned/duplicate rows and bounds the total to eight per maximum catalog track count. Names/colors do not modify audio-file tags.

Schema 1–4 upgrades add only missing tables in one transaction, with the existing previous-good backup. Failed schema creation retains the prior version/data. Cue snapshots commit in the same transaction as track metadata, crate membership and grids; explicit removal and track deletion remove cue rows. Library restore/snapshot copies preserve the values, including missing-file entries. Legacy JSON remains ID/path-only and yields empty cue slots.

This is a data-layer foundation, not a usable cue panel: no GUI pads, engine jumps, duration validation against a loaded source, quantization, hardware feedback or session restore is implemented here. Cue positions intentionally remain cached if media changes; the following controller slice must validate against the actual decoded duration before triggering. Storage does not guess duration from approximate header metadata. Full snapshot/backup costs and maximum-size performance remain unqualified.
