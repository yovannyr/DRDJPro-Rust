# Local library storage

This slice persists track IDs and paths. It does not persist deck transport, PCM, analysis/grid data, search text, sort order or selection. The application restores the table and keeps decks paused/unloaded unless explicit CLI tracks were supplied.

## Location and format

- Windows: `%LOCALAPPDATA%/DrDJPro/library.json`.
- macOS: `$HOME/Library/Application Support/DrDJPro/library.json` (path logic only; no macOS build/acceptance yet).
- Other platforms: `$XDG_DATA_HOME/DrDJPro/library.json`, falling back to `$HOME/.local/share/DrDJPro/library.json` (unqualified).
- `DRDJ_LIBRARY_PATH` may supply an absolute catalog filename for isolated installations/testing. Relative overrides are rejected visibly; there is no silent working-directory fallback.

The UTF-8 JSON schema has `version: 1` and `tracks`, each containing a positive unique `id` and absolute MP3/WAV `path`. Restores validate schema/version, track limit, IDs, supported paths and duplicates. Restored additions receive IDs above the existing maximum; sorting/filtering never renumbers tracks. Read/write size is bounded to 32 MiB; the existing 100,000-track and 5,000-path import caps remain safeguards, not scale/performance acceptance. Serde/serde_json are now explicit dependencies at versions already in the lockfile; tempfile is test-only.

## Save/recovery contract

A dedicated `library-storage` worker owns the catalog and `.lock` file's OS lock. A second writer fails without changing the first instance's data; the second UI reports session-only behavior. The lock file may remain on disk, but exclusivity belongs to the open OS handle rather than file existence. Closing the store releases that lock.

Changes request background saves through a bounded command channel with at most one GUI save outstanding. A later mutation remains dirty and is saved afterward. Serialization, file checks and file reads/writes run on workers, outside the audio callback and GUI timer. Snapshot construction and table filtering still occur on the UI control plane and have not been qualified at maximum library size.

Each save writes and syncs a same-directory `.tmp` file, then renames it over the complete destination. After an earlier valid catalog exists, `.bak` is replaced with that previous valid catalog before the primary is replaced. Failed writes do not truncate the live catalog. Startup ignores incomplete temp files, loads a valid primary, or recovers a valid backup if the primary is damaged/missing. A recovered backup can be one successful save older. Subsequent saves use the recovered valid bytes as the backup, never the corrupt primary bytes. Unknown schema versions are preserved rather than downgraded through an older backup. Unrecoverable data is left intact and cannot be overwritten by later session-only imports.

Write failures leave an explicit NOT saved status and Retry Save action. A further change also retries. Normal close drains outstanding work and flushes the latest snapshot on the storage thread before returning. A final flush failure causes an error exit. Terminating before an import has been accepted or before a dirty save completes can lose that pending change; power-loss/filesystem guarantees are not qualified by these tests.

Availability is checked during restoration and selected-file import/load. Missing entries retain their ID/path and show MISSING; they are not silently removed. Loading a missing path reports an error and retains the installed deck track. Restoring the file at the same path and reimporting it refreshes availability without a duplicate or new ID. There is no background filesystem watcher or relocation dialog in this slice.

## Acceptance

Store tests cover Unicode path/ID round trips, additions after restored IDs, missing-file retention, previous-good recovery without backing up corruption, ignored interrupted temp writes, forced rename failure, future-version and unrecoverable-data preservation, bounded reads, invalid catalog data, lock exclusion/release and close during a pending save.

Controller tests recreate the worker/controller with the same catalog, restore real WAV/MP3 entries, retain and mark deleted files, load the correct restored ID into a paused deck, preserve audio on missing-file failure, and exercise write-error/retry plus session-only damaged-startup behavior. These are automated headless restart scenarios, not visual/native-dialog or physical listening tests.
