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
