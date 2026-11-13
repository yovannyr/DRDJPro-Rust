# Feature Specification: iTunes Library / Apple Music Integration

**Feature Branch**: `31-itunes-library`  
**Created**: 2026-11-11  
**Status**: Software implemented with automated tests and independent review; native/manual acceptance and live Apple API qualification remain open (no developer token)
**Input**: Serato DJ Pro weak-links `iTunesLibrary` for Media Library import. DRDJPro-Rust has `library.rs` and `library_manager.rs` but no iTunes/Apple Music integration.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - iTunes Library Import (Priority: P1)

As a user migrating from iTunes, I want to import my iTunes library (tracks, playlists, ratings) so that I can continue using my existing music collection in DRDJPro-Rust.

**Why this priority**: Many users have large iTunes libraries and switching to a new DJ application requires a smooth migration path.

**Independent Test**: Can be tested by importing a sample iTunes Library XML file and verifying that tracks and playlists are imported correctly.

**Acceptance Scenarios**:

1. **Given** an iTunes Library XML file is provided, **When** import starts, **Then** all tracks are parsed and added to the library
2. **Given** tracks are imported, **When** import completes, **Then** track count, playlist count, and any errors are reported
3. **Given** an imported track is missing from disk, **When** the library is scanned, **Then** the track is marked as unavailable
4. **Given** a playlist is imported, **When** the playlist is viewed, **Then** it contains the same tracks as in iTunes

---

### User Story 2 - Apple Music Track Metadata (Priority: P2)

As a user with an Apple Music subscription, I want to enrich my local library with Apple Music metadata (correct titles, artist info, album art) so that my library is consistent and beautiful.

**Why this priority**: Apple Music provides high-quality metadata and album art that improves the library experience.

**Independent Test**: Can be tested by looking up a track via Apple Music and verifying that metadata is applied correctly.

**Acceptance Scenarios**:

1. **Given** a local track matches an Apple Music entry, **When** metadata is fetched, **Then** the title, artist, and album are updated
2. **Given** album art is available from Apple Music, **When** the metadata is fetched, **Then** the album art is downloaded and stored
3. **Given** a lookup fails, **When** the fallback is triggered, **Then** the existing local metadata is preserved
4. **Given** multiple tracks are looked up, **When** the batch completes, **Then** results show success/failure counts

---

### User Story 3 - Library Synchronization (Priority: P1)

As a user with music in both iTunes and local folders, I want to synchronize my library so that all tracks are indexed and searchable in one place.

**Why this priority**: Users often have music spread across iTunes and local folders; a unified library simplifies track search and discovery.

**Independent Test**: Can be tested by adding tracks to both iTunes and local folders, then verifying the library shows all tracks.

**Acceptance Scenarios**:

1. **Given** iTunes and local folders are configured, **When** sync runs, **Then** tracks from both sources are indexed
2. **Given** a track exists in both iTunes and local folders, **When** sync completes, **Then** the track appears only once in the library
3. **Given** a track is removed from iTunes, **When** sync runs, **Then** the track is not removed from the library if it exists locally
4. **Given** sync is complete, **When** the user searches for a track, **Then** the track is found regardless of source

---

### User Story 4 - Media Library Query (Priority: P2)

As an application, I want to query the local iTunes Media Folder for file paths, sizes, and modification dates so that file management is accurate.

**Why this priority**: iTunes organizes music into a specific folder structure; querying it ensures accurate file paths.

**Independent Test**: Can be tested by verifying that file paths retrieved from iTunes match the actual file system.

**Acceptance Scenarios**:

1. **Given** a track is queried from the iTunes library, **When** the file path is retrieved, **Then** the path matches the actual file location
2. **Given** a track file has been moved, **When** the path is verified, **Then** the track is marked as missing
3. **Given** a file path is retrieved, **When** the file size and modification date are read, **Then** the values match the file system
4. **Given** the iTunes Media Folder path is configured, **When** the folder is scanned, **Then** all media files are enumerated

---

## Edge Cases

- What happens when the iTunes library contains references to network-attached storage?
- How are Apple Music DRM-protected tracks handled?
- What happens when the iTunes library XML is corrupted?
- How are duplicate tracks (same file referenced by multiple playlists) handled?
- What happens on Windows where iTunes uses a different library location than macOS?
- How are tracks with non-UTF-8 filenames handled?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST parse iTunes Library XML files (both legacy PLIST and new XML formats)
- **FR-002**: System MUST extract track metadata from iTunes XML (title, artist, album, album artist, genre, BPM, rating, play count, duration, location)
- **FR-003**: System MUST import playlists from iTunes XML with correct track ordering
- **FR-004**: System MUST detect and handle duplicate tracks during import (same file, different metadata)
- **FR-005**: System MUST verify imported track files exist on disk
- **FR-006**: System MUST support incremental library synchronization (only import new/changed tracks)
- **FR-007**: System MUST provide an Apple Music metadata lookup API for track enrichment
- **FR-008**: System MUST download and cache album art from Apple Music for imported tracks
- **FR-009**: System MUST support platform-specific iTunes library paths (macOS: `~/Music/iTunes/`, Windows: `%USERPROFILE%\Music\iTunes\`)
- **FR-010**: System MUST query the iTunes Media Folder for file path resolution
- **FR-011**: System MUST exclude Apple Music DRM-protected tracks from import with a user warning
- **FR-012**: System MUST handle corrupted or incomplete iTunes XML files gracefully (skip unreadable entries)
- **FR-013**: System MUST allow users to select a custom iTunes Library XML file for import
- **FR-014**: System MUST preserve iTunes-specific metadata (rating, play count, date added) in the DRDJPro-Rust library

### Key Entities

- **iTunes Library**: Parsed XML representing the iTunes library with tracks, playlists, and preferences
- **iTunes Track**: Track metadata from the iTunes XML (field mapping to DRDJPro-Rust Track)
- **iTunes Playlist**: Playlist with name, persistent ID, and ordered track references
- **Apple Music Lookup Result**: Metadata returned from the Apple Music API for a track
- **Import Progress**: Current import state (tracks processed, total tracks, errors, skipped duplicates)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: iTunes Library XML is parsed within 30 seconds for a library of 50,000 tracks
- **SC-002**: Track metadata is imported with at least 95% field accuracy (compared to iTunes XML source)
- **SC-003**: Duplicate detection correctly identifies 100% of duplicate tracks (same file path, different playlist entries)
- **SC-004**: Incremental sync completes within 5 seconds for libraries with < 1% changed tracks
- **SC-005**: Apple Music metadata lookup returns results within 2 seconds for 95% of track lookups
- **SC-006**: Album art download completes within 10 seconds per track with < 1% failure rate
- **SC-007**: Corrupted XML files are handled gracefully with no crashes (100% resilience)
- **SC-008**: Users can successfully import libraries of 100,000+ tracks without out-of-memory errors

## Dependencies

- `serde` crate (1) with `derive` feature for XML/JSON deserialization
- `serde_xml` crate (0.8) for iTunes XML parsing
- `sqlx` crate (0.8) with SQLite runtime for local library database
- `reqwest` crate (0.12) with JSON for Apple Music API calls
- `image` crate (0.25) for album art download and caching
- Platform-specific: `objc` and `objc_id` crates (macOS only) for native iTunes Library framework (optional)

## Assumptions

- iTunes Library XML files are in the standard Apple format (XML, not binary PLIST)
- Apple Music API is accessible and supports the MusicKit JavaScript SDK or MusicAPI
- DRM-protected Apple Music tracks cannot be imported (only metadata lookup)
- Users have valid Apple Music credentials for metadata enrichment
- iTunes library paths follow the default conventions (custom paths can be specified)

## Technical Constraints

- XML parsing must handle both legacy PLIST-based and modern XML-based iTunes libraries
- Apple Music API requires a valid MusicKit developer token for metadata lookups
- Album art must be cached locally to avoid repeated downloads
- Import must not lock the library during long-running operations (use async/progress reporting)
- File paths must be normalized to the application's platform conventions

## Out of Scope

- Full Apple Music streaming integration (metadata lookup only; streaming is covered by Spec 30)
- iTunes library editing/writing back (import-only in this version)
- Apple Music playlist creation or modification
- Sync with Apple Music cloud library (local library import only)
- Integration with Apple Music for Artists or analytics
- Legacy Winamp or Windows Media Player library import (only iTunes in this version)
