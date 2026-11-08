# Feature Specification: Cloud Sync

**Feature Branch**: `2-cloud-sync`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Enable cloud sync for DJ library, playlists, cue points, and settings across multiple devices using Supabase"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Device Library Access (Priority: P1)

As a mobile DJ, I want my entire music library synced across all my devices so I can prepare sets on one device and perform on another without manually transferring data.

**Why this priority**: Core value proposition - enables seamless multi-device workflows essential for professional DJs.

**Independent Test**: Can be fully tested by adding tracks on one device, syncing, and verifying they appear on another device. Delivers immediate value.

**Acceptance Scenarios**:

1. **Given** tracks are added to library on device A, **When** sync is triggered, **Then** tracks appear in library on device B
2. **Given** track metadata is updated on device A, **When** sync completes, **Then** changes are reflected on device B
3. **Given** a track is deleted on device A, **When** sync completes, **Then** the track is removed from device B

---

### User Story 2 - Real-Time Sync Status (Priority: P1)

As a user, I want to see clear sync status indicators so I know when my data is synchronized, syncing, or has errors.

**Why this priority**: Essential feedback mechanism that prevents confusion and data loss concerns.

**Independent Test**: Can be tested by triggering sync and verifying status changes from "Not Synced" → "Syncing" → "Synced".

**Acceptance Scenarios**:

1. **Given** sync is initiated, **When** upload starts, **Then** status shows "Syncing" with progress indicator
2. **Given** sync completes successfully, **When** all data uploaded, **Then** status shows "Synced" with timestamp
3. **Given** sync fails, **When** error occurs, **Then** status shows "Error" with description
4. **Given** device is offline, **When** sync is attempted, **Then** status shows "Offline" message

---

### User Story 3 - Selective Sync Control (Priority: P2)

As a user with limited bandwidth or storage, I want to selectively sync only library, playlists, or settings so I can control what data is synchronized.

**Why this priority**: Provides flexibility for users with constraints, reduces bandwidth usage.

**Independent Test**: Can be tested by enabling only playlist sync and verifying library data is not synced.

**Acceptance Scenarios**:

1. **Given** only "Library" sync is enabled, **When** sync occurs, **Then** only track metadata is synchronized
2. **Given** only "Playlists" sync is enabled, **When** sync occurs, **Then** only playlist data is synchronized
3. **Given** multiple categories are enabled, **When** sync occurs, **Then** all enabled categories are synchronized
4. **Given** a category is disabled, **When** sync occurs, **Then** that category's data is not modified

---

### User Story 4 - Automatic Background Sync (Priority: P2)

As a user, I want optional automatic synchronization so my changes are backed up without manual intervention.

**Why this priority**: Convenience feature that prevents data loss through automatic backups.

**Independent Test**: Can be tested by enabling auto-sync, making changes, and verifying sync occurs automatically within interval.

**Acceptance Scenarios**:

1. **Given** auto-sync is enabled, **When** data changes are made, **Then** sync occurs automatically after 5 minutes
2. **Given** auto-sync is disabled, **When** data changes are made, **Then** sync only occurs manually
3. **Given** sync is in progress, **When** user makes changes, **Then** changes are queued for next sync

---

### User Story 5 - Conflict Resolution (Priority: P2)

As a user editing data on multiple devices, I want smart conflict resolution so I don't lose work when the same data is modified in multiple places.

**Why this priority**: Prevents data loss in multi-device scenarios, essential for reliability.

**Independent Test**: Can be tested by modifying same playlist on two devices offline, then syncing both.

**Acceptance Scenarios**:

1. **Given** same item modified on two devices, **When** both sync, **Then** most recent change wins (last-write-wins)
2. **Given** items deleted on one device and modified on another, **When** sync occurs, **Then** deletion takes precedence
3. **Given** new items added on both devices, **When** sync occurs, **Then** both items are preserved with merged data

---

### Edge Cases

- What happens when sync is interrupted mid-operation?
- How does system handle exceeding cloud storage limits?
- What happens if Supabase service is unavailable?
- How are extremely large libraries (100,000+ tracks) handled?
- What happens when device IDs conflict?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST sync track library metadata (title, artist, album, BPM, key, duration, file path)
- **FR-002**: System MUST sync all user playlists and playlist contents
- **FR-003**: System MUST sync cue points for each track (hot cues with positions, colors, labels)
- **FR-004**: System MUST sync beat grid analysis data for each track
- **FR-005**: System MUST sync all application settings and preferences
- **FR-006**: System MUST display real-time sync status (syncing, synced, error, offline)
- **FR-007**: System MUST support manual sync trigger via UI button
- **FR-008**: System MUST support automatic sync on a configurable interval
- **FR-009**: System MUST allow selective sync by category (library, playlists, settings, cue points, beat grids)
- **FR-010**: System MUST track last sync timestamp per device
- **FR-011**: System MUST identify each device uniquely for conflict resolution
- **FR-012**: System MUST implement last-write-wins conflict resolution strategy
- **FR-013**: System MUST work offline and queue changes for next sync
- **FR-014**: System MUST authenticate users before allowing sync operations
- **FR-015**: System MUST isolate data per user (no cross-user data access)
- **FR-016**: System MUST handle sync errors gracefully with user-friendly messages
- **FR-017**: System MUST report item counts after successful sync (e.g., "Synced 1,234 tracks")

### Key Entities

- **Track Library**: Collection of track metadata (ID, title, artist, album, BPM, key, duration, file path, added date)
- **Playlist**: Named collection of track references with ordering
- **Cue Point**: Position marker in a track (position in seconds, color, label, type: hot cue/memory cue)
- **Beat Grid**: Analysis data for a track (first beat position, BPM, beat positions array)
- **Settings**: Application preferences (audio device, buffer size, key display format, theme, etc.)
- **Sync Status**: Current sync state (connected/disconnected, syncing/idle, last sync time, error message)
- **Device**: Unique device identifier with name and last sync timestamp

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can access their complete library on any device within 2 minutes of sync completion
- **SC-002**: Sync operations complete within 30 seconds for libraries under 10,000 tracks
- **SC-003**: 99% of sync operations complete successfully without data loss
- **SC-004**: Conflict resolution preserves at least 95% of user changes in multi-device scenarios
- **SC-005**: Sync status updates are visible within 1 second of status changes
- **SC-006**: Offline changes sync successfully when connectivity is restored
- **SC-007**: System handles 100,000+ track libraries without performance degradation
- **SC-008**: Auto-sync completes without user awareness or UI interruption

## Dependencies

- Supabase cloud service for data storage
- Internet connectivity for sync operations
- User authentication system

## Assumptions

- Users have Supabase accounts or can create them
- Internet connectivity is available most of the time
- Cloud storage limits are sufficient for typical DJ libraries (100GB+)
- Audio files themselves are not synced (only metadata)
- File paths are device-specific and handled by application logic

## Technical Constraints

- Audio files are NOT synced (too large), only metadata
- Sync requires active internet connection
- Supabase KV storage limits apply
- API rate limits may affect sync speed for very large libraries
- User must be authenticated to sync

## Out of Scope

- Actual audio file synchronization (only metadata)
- Peer-to-peer sync between devices (cloud-only)
- Real-time collaborative editing with live updates
- Version history or undo for synced data
- Offline-first architecture with eventual consistency
- Custom conflict resolution UI (automatic resolution only)
