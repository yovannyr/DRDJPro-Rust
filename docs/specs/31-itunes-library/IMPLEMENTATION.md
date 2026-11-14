# Spec31 implementation and acceptance

## Slice1: bounded XML import core

`crates/drdj-itunes` parses exported iTunes/Music XML plists at top-level track
and playlist boundaries. It does not read binary ITL/Music library databases,
fetch external DTDs or write audio/tags. Missing semantic fields skip an entry;
structural/ambiguous XML aborts before catalog mutation. Internal entities are
rejected. Full source metadata includes title, artist, album, album artist, genre,
BPM, rating0-100, play count, duration, location, persistent ID and date added.
Text fields have engineering bounds (512characters for display metadata); normal
mapped fixture fields are preserved. Playlists retain order and repeated entries.

Limits:512MiB XML,250,000track records,4,096ordinary playlists,1,000,000playlist
links,24nested values and12,000,000value nodes. No full-library XML value tree is
retained; one track or playlist dictionary is materialized at a time. A single
large playlist can still consume memory up to the stated link limit.

Worker-only inspection resolves file URIs, optional Music Folder remapping,
existence/size/modification time and canonical identity. Same-file paths share
one record; playlist references map to that record while retaining repetitions.
Missing files remain unavailable entries. Unknown/skipped references are counted.
Protected/subscription/.m4p tracks (including encoded extensions) are excluded;
remote HTTP audio URLs never become local deck paths. On Windows the native
verbatim path form and existing parent aliases are normalized consistently even
before a file appears. Individual OS/network filesystem calls cannot be cancelled
while blocked; cancellation is checked between calls, records and playlist links.

Nine core tests passed, including malformed/hostile XML, entity/text handling,
all mapped fields, duplicate/ordered playlist behavior, media-root safety, DRM,
missing files, cancellation and Windows missing-to-present path identity.
One debug-profile synthetic probe:50,000tracks parsed in1.53s;100,001tracks
(21,567,151bytes XML) in2.99s. This measures parsing only, not filesystem, SQLite,
UI responsiveness or universal memory capacity. Strict crate Clippy, formatting
and diff checks passed. Independent GPT-6.1-Sol medium review and two accepted
follow-ups fixed cancellation after final file inspection and within parent walks.

Storage, incremental application import, source playlist UI and Apple metadata
adapter follow in separate slices. User confirmed no MusicKit developer token;
live Apple API/network performance acceptance cannot be completed with current
credentials. Local XML import is independent. Spec26 remains stopped; native
visual/manual/listening/hardware tests remain deferred.

## Slice2: transactional source catalog

A separate SQLite source catalog retains complete imported metadata, stable source
IDs, original playlist ordering and repetitions. It does not replace or mutate the
native library schema. Sources are bounded to32; active tracks/links retain core
limits, individual JSON records256KiB, database pages1GiB (WAL/temporary files can
add overhead). Own app/version identifiers protect unrelated databases. Worker
connections use strict tables, foreign keys, WAL and FULL synchronization.

Only changed/new rows are written. A no-op synchronization is verified against
SQLite total_changes, not just a reported count. Source removals archive records;
no local app track is deleted. Cancelled writes roll back the complete transaction.
Reads use one transaction across source, track and playlist queries, including a
two-connection WAL regression; the independent review found and accepted this fix.
Stored metadata/identity/references and bounded fields are validated on retrieval.

Seven storage plus nine parser/inspection tests passed, doctests passed. Strict
crate Clippy, formatting and diff checks passed. Isolated debug SQLite probe for
50,000rows/100changed:2.09s; source XML parsing, filesystem work and UI excluded.
Independent GPT-6.1-Sol medium review and follow-up accepted with no remaining
findings. The next slice connects ordered sources to the app/shared library.

## Slice3: native source window and shared library

The lazy iTunes window provides XML/media-folder selection, cached sources,
ordered playlists (including repetitions), complete source details and guarded
deck loading through the existing asynchronous controller. A virtual row model
avoids materializing 250,000 Slint rows. Worker threads own XML, SQLite, file
inspection and pickers. Cached reads refresh file availability without writing an
older source generation back over a concurrent synchronization.

Native schema18 and compatible optional protobuf field9 retain imported-tag
preference across restart, restore, background tag scans and deck loading.
Existing native track IDs, analysis, cues and local tracks are retained. Source
removals never delete shared tracks. Source SQLite commits and asynchronous native
saves are separate phases; the window shows native save status, and reopening a
cached source reconciles an interrupted application. Cancellation can leave an
explicitly reported applied prefix. The application loop budgets 2ms per tick;
publishing/model replacement is outside that budget and UI latency is unqualified.

The shared catalog limit is250,000; the existing directory/picker batch limits
remain unchanged. A regression constructs100,001 native records. Other tests cover
actual WAV deck loading with imported title precedence, v17-to-v18 migration,
restart/restore, protobuf compatibility, source import/reopen and filesystem
availability changes, ordered virtual playlists, cancelled late worker results
and an invalid media override preserving the committed source. Root strict Clippy,
76 native SQLite tests, 21 protobuf crate tests, five native protobuf adapter tests
and17 iTunes core tests passed. Independent GPT-6.1-Sol medium review accepted the
slice after fixes. No native visual/manual acceptance is claimed.
The native shared catalog requires UTF-8 paths. The source URI can preserve other
native filenames, but shared application reports an unsupported path and stops
with an explicit applied-prefix count; no complete native import is claimed then.

## Slice4: optional Apple catalog metadata and bounded artwork cache

The explicit APPLE METADATA action looks up the selected local track; APPLE
PLAYLIST processes at most50 unique tracks from the current source playlist.
No automatic network lookups occur during import, playback or source opening.
Configure `DRDJ_APPLE_MUSIC_TOKEN` with an already signed MusicKit developer JWT
in the process environment before launching, and optionally
`DRDJ_APPLE_MUSIC_STOREFRONT` (two lowercase letters; default `us`). Never enter
an Apple ID password or signing private key. Missing configuration gives a visible
message and leaves local import available. The user currently has no token.

Worker-only reqwest uses OS TLS, verified HTTPS, fixed Apple catalog endpoints,
no redirects/retries,2s lookup and10s artwork timeouts. Bearer authorization is
sensitive and sent only to the catalog; artwork uses HTTPS mzstatic.com hosts
without credentials. Response bodies are capped at1MiB JSON/10MiB artwork. Images
are bounded to2048x2048/32MiB allocation and normalized to at most300x300 JPEG;
small covers are not enlarged. No audio, preview, DRM or cloud-library API is used.

Matching requires one unique song ID with normalized title/artist equality and,
when both durations exist, a difference no greater than2s. It intentionally does
not guess typographical corrections or choose among ambiguous releases. Failure,
ambiguity, invalid artwork or HTTP401/429 retains existing local metadata and
cached data. Batch status reports success/failure counts; only successful IDs
reach the shared-library application loop. Cancellation can retain committed
cache successes, recovered by reopening, while discarding late UI output.

Source schema2 adds separate original-identity/date-stamp overlays and thumbnail
blobs. XML records stay intact. Cache storage is capped at128MiB (inside the1GiB
source database bound); reaching the quota rejects new enrichment and preserves
existing entries rather than deleting unrelated data. Inactive source records do
not count against active overlay read limits. Size/modified time and source fields
are checked on read and atomically on write; a late result cannot replace a newer
file generation. After an externally changed file, IMPORT / SYNC refreshes the
source stamp before another enrichment. Timestamps are identity hints, not content
hashes; same-size replacements preserving timestamps cannot be distinguished.
Only the selected cached thumbnail is decoded on a worker and transferred to
Slint as pixels. Original XML metadata remains visible beneath Apple values.
An enrichment action shares immutable source/playlist arrays; it does not clone
or reindex the complete large catalog to update a single song's metadata.

Independent GPT-6.1-Sol medium core and app reviews accepted the implementation
after fixing stale file-generation writes and archived-row counting. Tests cover
matching/ambiguity, sensitive authorization separated from artwork, real loopback
HTTP302/401/429/size handling, cancellation, image/URL bounds, source migration,
restart, stale writes, inactive rows, partial/all-failed batches, success-only
native application and artwork restart. These are local automated tests, not
live-provider reliability or latency evidence.

## Requirement and acceptance ledger

| Requirement | Software evidence | Remaining qualification |
|---|---|---|
| FR-001/002 | Standard exported iTunes/Music XML plists, all mapped fields | No binary ITL/native database access; unspecified alternative XML schemas unsupported |
| FR-003/004 | Ordered repeated playlist links, canonical same-file identity | Native source browsing acceptance |
| FR-005 | Worker file inspection and refreshed cached availability | Slow/unavailable network-share OS calls can block their worker |
| FR-006 | Changed-only source SQL, native dedup/preference reconciliation | Full end-to-end incremental timing on a real large library |
| FR-007/008 | Configured Apple client, conservative matching, durable covers | Live API unavailable without developer token |
| FR-009/013 | Conventional picker hints and custom XML/media-folder selection | Native pickers and platform conventions on each OS |
| FR-010 | Exported Music Folder resolution and explicit override | XML import inspects referenced files; unreferenced files use existing native folder import |
| FR-011/012 | DRM exclusions/warnings, per-record semantic skips, structural abort | Real damaged-library corpus; no universal100% resilience claim |
| FR-014 | Full metadata in source SQLite, original values visible | Native/manual workflow acceptance |

| Criterion | Measured automated evidence | Acceptance limit |
|---|---|---|
| SC-001 |50,000 synthetic XML records parsed in1.53s | Parser-only, not full import |
| SC-002 | Exact mapped fixture fields incl. rating/play count/date |95% real-library accuracy unmeasured |
| SC-003 | Canonical/alias duplicate and ordered-repeat regressions |100% universal filesystem identity not claimed |
| SC-004 |50,000-row SQLite incremental update/100changes in2.09s | XML/filesystem/UI excluded; full5s criterion open |
| SC-005/006 |2s/10s request timeouts and bounded failure paths | No live95% lookup latency or <1% artwork failure measurement |
| SC-007 | Malformed/entity/bounds/cancellation regressions | Not proof of resilience against every possible corrupt input |
| SC-008 |100,001 parsed and native catalog records without OOM in probes | Full100k file/SQLite/native UI import and peak memory unqualified |

Use existing quick-xml/rusqlite architecture instead of parallel serde_xml/sqlx
stacks; reqwest0.13.5 uses native TLS to avoid an additional bundled crypto stack.
All new file/network/database work stays outside the audio callback. Native tag
preference modifies control-plane catalog metadata, not DSP or decoded audio time.
Spec26 remains stopped; joint visual/manual/listening tests remain deferred.

Primary references:
- https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens
- https://developer.apple.com/documentation/applemusicapi/search-for-catalog-resources-(by-type)
- https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.ClientBuilder.html

## Final regression notes

The wider native catalog regression now derives its boundary search and expected
ID from MAX_TRACKS: the former literal99999.wav legitimately also matched199999.wav
after raising capacity to250,000. Its delta-allocation/revision assertions remain
unchanged; independent review accepted the boundary-test correction.

A default-parallel workspace run passed1,688 library and566 active app tests
(six pre-existing ignored hardware/media/long-duration qualifications), then the
unchanged crash-process normal-exit test exceeded its5s wall-clock assertion while
nine process/dump scenarios ran together. The unchanged complete crash-process
suite passed9/9 serially in14.36s. The reviewer confirmed that serial harness
execution isolates process/dump resource contention; explicitly concurrent test
scenarios keep their own concurrency. This is not proof of the5s bound under
parallel process load. Spec26 implementation is stopped and was not modified.

Final automated checkpoint (2026-11-13, Windows):

- `cargo test --workspace --locked --quiet -- --test-threads=1`:2,400 passed,
  zero failed, six pre-existing ignored qualifications. Includes the250,000-track
  catalog boundary and all existing GUI/process integrations.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- After the final reviewed immutable-array sharing change,
  `cargo test -p drdjpro-ui-spike --bin drdjpro-ui-spike itunes --locked --quiet`:
  all nine targeted tests passed.
- `cargo clippy --workspace --all-targets --features dvs-xwax --locked -- -D warnings`:
  passed for the final diff, without warning suppression.
- `cargo build -p drdjpro-ui-spike --release --features dvs-xwax --bin drdjpro-ui-spike --locked`:
  passed; cold release10m42s, final repeat1.36s. Executable35,781,632bytes.
- Focused rustfmt/Cargo formatting and diff checks passed. Core and app independent
  reviewers used GPT-6.1 Sol with medium effort and accepted all fixes.

The rebuilt shared target (debug/test/check and DVS release variants together)
contains11,879,593,990bytes (11.06GiB), with zero incremental files; C: has51.77GiB
free at this checkpoint. These are observations, not a Cargo disk quota. No native
application/browser window or real Apple/Beatport API session was used for acceptance.
