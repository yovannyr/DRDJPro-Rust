# Feature Specification: Library Management

**Feature Branch**: `13-library-management`  
**Created**: December 15, 2025  
**Status**: In Development  
**Input**: User description: "Add comprehensive library management with playlists, crates, smart playlists, search, filtering, and metadata editing"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Track Organization with Playlists (Priority: P1)

As a DJ managing my collection, I want to create and organize playlists so I can prepare sets and group tracks by gig or style.

**Why this priority**: Fundamental organization tool - DJs cannot effectively prepare without playlist management.

**Independent Test**: Can be fully tested by creating playlist, adding tracks, and verifying organization persists.

**Acceptance Scenarios**:

1. **Given** user clicks "New Playlist", **When** name entered, **Then** empty playlist is created
2. **Given** playlist exists, **When** tracks are dragged to playlist, **Then** tracks are added to playlist
3. **Given** track is in playlist, **When** removed, **Then** track is removed from playlist (not library)
4. **Given** playlist has tracks, **When** reordered, **Then** custom ordering is saved

---

### User Story 2 - Advanced Search and Filtering (Priority: P1)

As a DJ finding tracks quickly, I want powerful search and filtering so I can instantly find tracks by any criteria (genre, BPM, key, etc.).

**Why this priority**: Essential workflow feature - large libraries are unmanageable without effective search.

**Independent Test**: Can be fully tested by searching for track and verifying results match query.

**Acceptance Scenarios**:

1. **Given** search term entered, **When** typed, **Then** matching tracks appear instantly
2. **Given** BPM filter applied, **When** range set (e.g., 120-130), **Then** only tracks in range shown
3. **Given** key filter applied, **When** key selected, **Then** only matching key tracks shown
4. **Given** multiple filters active, **When** combined, **Then** AND logic applies (all criteria must match)

---

### User Story 3 - Smart Playlists with Auto-Population (Priority: P2)

As a DJ wanting automated organization, I want smart playlists that auto-populate based on criteria so my collections stay current automatically.

**Why this priority**: Automation saves time and ensures playlists stay relevant as library grows.

**Independent Test**: Can be tested by creating smart playlist with criteria and verifying matching tracks populate automatically.

**Acceptance Scenarios**:

1. **Given** smart playlist created with "BPM 120-130" rule, **When** saved, **Then** all matching tracks populate automatically
2. **Given** new track imported matching rule, **When** added to library, **Then** track appears in smart playlist automatically
3. **Given** track metadata changes, **When** updated, **Then** smart playlists update accordingly
4. **Given** multiple rules combined, **When** set, **Then** AND/OR logic applies correctly

---

### User Story 4 - Crates for Collection Organization (Priority: P2)

As a DJ with large collections, I want crates (folders of playlists) so I can organize playlists by event type, venue, or mood.

**Why this priority**: Higher-level organization for complex libraries with many playlists.

**Independent Test**: Can be tested by creating crate, adding playlists, and verifying organization.

**Acceptance Scenarios**:

1. **Given** user creates crate, **When** named, **Then** empty crate appears in sidebar
2. **Given** crate exists, **When** playlists dragged into crate, **Then** playlists are organized under crate
3. **Given** crate is collapsed, **When** clicked, **Then** crate expands to show contained playlists
4. **Given** crate has icon, **When** customized, **Then** custom icon displays

---

### User Story 5 - Metadata Editing and Tagging (Priority: P2)

As a DJ curating my library, I want to edit track metadata and add custom tags so I can personalize organization beyond standard fields.

**Why this priority**: Enables personal curation and improves findability with custom categorization.

**Independent Test**: Can be tested by editing track metadata and verifying changes persist and appear in UI.

**Acceptance Scenarios**:

1. **Given** track is selected, **When** edit mode activated, **Then** all metadata fields are editable
2. **Given** BPM field edited, **When** saved, **Then** new BPM is stored and displayed
3. **Given** custom tag added, **When** entered, **Then** tag appears on track and is searchable
4. **Given** rating changed, **When** stars clicked, **Then** new rating saves and affects filtering

---

### User Story 6 - Multiple View Modes (Priority: P2)

As a DJ with different workflows, I want multiple view modes (list, grid) so I can choose visualization that suits my current task.

**Why this priority**: Flexibility improves workflow - grid for browsing, list for detailed info.

**Independent Test**: Can be tested by switching view modes and verifying track display changes appropriately.

**Acceptance Scenarios**:

1. **Given** list view is active, **When** displayed, **Then** tracks shown in detailed rows with all metadata
2. **Given** grid view is active, **When** displayed, **Then** tracks shown as cards with artwork
3. **Given** view mode changed, **When** switched, **Then** transition is smooth and selection persists
4. **Given** view mode preference set, **When** app reopens, **Then** last used view mode is active

---

### Edge Cases

- What happens when playlist has thousands of tracks?
- How does system handle duplicate tracks in playlists?
- What happens when smart playlist criteria matches entire library (10,000+ tracks)?
- How are missing files handled when playlist references deleted tracks?
- What happens when importing playlists from other DJ software?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support creating, renaming, and deleting playlists
- **FR-002**: System MUST support adding/removing tracks from playlists
- **FR-003**: System MUST support reordering tracks within playlists
- **FR-004**: System MUST provide full-text search across all metadata fields
- **FR-005**: System MUST support filtering by BPM range, key, genre, year, rating
- **FR-006**: System MUST support smart playlists with auto-population rules
- **FR-007**: System MUST support AND/OR logic in smart playlist rules
- **FR-008**: System MUST support crates (folders) containing multiple playlists
- **FR-009**: System MUST support track metadata editing (title, artist, album, BPM, key, genre, year, rating, comments)
- **FR-010**: System MUST support custom color tagging per track
- **FR-011**: System MUST support play count and last played tracking
- **FR-012**: System MUST provide list view and grid view modes
- **FR-013**: System MUST support sorting by any metadata field (ascending/descending)
- **FR-014**: System MUST support multi-select for batch operations
- **FR-015**: System MUST display total track count and duration for playlists
- **FR-016**: System MUST support playlist export to M3U/PLS formats
- **FR-017**: System MUST support duplicate track detection

### Key Entities

- **Playlist**: Playlist ID, name, track IDs (ordered), color, created date, modified date
- **Crate**: Crate ID, name, playlist IDs, icon, collapsed state
- **Smart Playlist**: Playlist ID, name, rules array, auto-update enabled, last updated
- **Smart Rule**: Field name, operator (equals, contains, between, greater than, less than), value(s), connector (AND/OR)
- **Track Metadata**: All Track fields plus genre, year, rating (0-5 stars), color tag, comments, date added, play count, last played
- **View Settings**: View mode (list/grid), sort field, sort direction, column visibility

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Search returns results within 100ms for libraries under 10,000 tracks
- **SC-002**: Playlist creation and track addition completes within 200ms
- **SC-003**: Smart playlist updates occur within 1 second of metadata changes
- **SC-004**: View mode switching completes within 300ms without lag
- **SC-005**: Batch operations on 100 tracks complete within 2 seconds
- **SC-006**: Filtering by multiple criteria returns results within 150ms
- **SC-007**: Metadata edits persist immediately and sync to cloud within 5 seconds
- **SC-008**: Users can organize 10,000+ track libraries without performance degradation

## Dependencies

- Track metadata storage system
- Cloud sync system (Feature 2) for playlist/metadata sync
- Search indexing for fast text search

## Assumptions

- Users have organized workflow requiring playlists and crates
- Library sizes range from 100 to 50,000 tracks
- Metadata is stored locally and synced to cloud
- Users understand smart playlist rule concepts
- Standard metadata fields cover 90% of user needs

## Technical Constraints

- Large playlists (5,000+ tracks) may impact rendering performance
- Full-text search requires indexing overhead
- Smart playlist updates can trigger frequent recalculation
- Cloud sync may delay metadata changes across devices
- Browser storage limits affect offline library capacity

## Out of Scope

- Automatic BPM/key detection during import (separate feature)
- Audio analysis during library management
- Integration with external music services beyond streaming (Feature 5)
- Automatic genre detection or classification
- Collaborative playlists (multiple users editing same playlist)
- Playlist version history or undo
- Automatic duplicate removal
- iTunes/Rekordbox/Serato library import (separate feature)
