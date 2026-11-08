# Feature 24: Smart Playlist Manager

**Status**: In Development  
**Priority**: LOW  
**Category**: Library Management

## Overview

AI-powered playlist generation system that creates dynamic, auto-updating playlists based on advanced rules and intelligent track recommendations.

## User Scenarios

### US-024-001: Smart Playlist Creation

**Given** a DJ wants to create a new smart playlist  
**When** they configure playlist criteria  
**Then** they can specify:

- Playlist type (energy flow, harmonic mixing, BPM progression, genre, mood)
- BPM range (min and max values)
- Key matching preferences
- Genre filters (include multiple genres)
- Minimum track rating
- Energy flow pattern (buildup/cooldown/plateau/wave)
- Exclusion rules (recently played tracks)
- Playlist name and description

### US-024-002: Automatic Track Selection

**Given** a smart playlist is created with criteria  
**When** the system generates the playlist  
**Then** it:

- Scans the entire library for matching tracks
- Applies all specified filters and rules
- Orders tracks according to playlist type
- Calculates optimal track sequence
- Ensures smooth transitions between tracks
- Updates track count and total duration
- Saves playlist for future use

### US-024-003: Energy Flow Playlists

**Given** a DJ creates an energy-based playlist  
**When** they select "Energy Flow" type  
**Then** the system:

- Analyzes energy levels of all matching tracks
- Sequences tracks based on selected flow pattern:
  - **Buildup**: Low to high energy progression
  - **Cooldown**: High to low energy progression
  - **Plateau**: Consistent energy level
  - **Wave**: Alternating energy levels
- Visualizes energy curve for the playlist
- Ensures gradual transitions (no sudden jumps)

### US-024-004: Harmonic Mixing Playlists

**Given** a DJ creates a harmonic playlist  
**When** they select "Harmonic Mixing" type  
**Then** the system:

- Prioritizes key-compatible transitions
- Follows Camelot wheel rules (±1 position)
- Allows relative major/minor switches
- Sequences tracks for smooth harmonic flow
- Displays key progression in playlist view
- Warns about difficult key transitions

### US-024-005: Track Recommendations

**Given** a DJ is currently playing a track  
**When** they open the recommendations panel  
**Then** the system suggests 10 compatible tracks based on:

- Harmonic compatibility (matching or adjacent keys)
- BPM similarity (±5 BPM range)
- Energy level matching
- Genre similarity
- Rating and play count
- Match score (0-100) with detailed reasons

### US-024-006: Playlist Analysis

**Given** a smart playlist has been generated  
**When** the DJ views playlist details  
**Then** they see analysis including:

- Energy flow visualization graph
- BPM range and average
- Genre distribution
- Total duration and track count
- Transition quality predictions
- Estimated set suitability scores
- Weak transition warnings (if any)

## Functional Requirements

### FR-024-001: Playlist Types
The system SHALL support five playlist types:
- **Energy Flow**: Sequences tracks by energy level progression
- **Harmonic Mixing**: Prioritizes key-compatible transitions
- **BPM Progression**: Orders tracks by BPM changes
- **Genre-Based**: Groups tracks by genre with smooth transitions
- **Mood-Based**: Sequences tracks by mood/vibe similarity

### FR-024-002: Playlist Criteria
The system SHALL support criteria configuration:
- BPM range with min/max values (80-180 range)
- Key matching toggle (enable/disable)
- Genre selection (multi-select from available genres)
- Minimum rating filter (0-5 stars)
- Energy flow pattern selection
- Exclude recently played option (configurable time window)
- Maximum playlist duration limit (optional)

### FR-024-003: Track Filtering
The system SHALL filter tracks based on:
- BPM within specified range
- Genre matches selected list (if specified)
- Rating meets minimum threshold
- Not in exclude list (if enabled)
- Has required metadata (BPM, key if key matching enabled)
- File accessible and playable

### FR-024-004: Track Sequencing
The system SHALL sequence tracks using algorithms for:
- **Energy Flow**: Sort by energy level, apply smoothing
- **Harmonic**: Pathfinding through Camelot wheel connections
- **BPM**: Gradual BPM changes with configurable step size
- **Genre**: Group by genre, smooth transitions at boundaries
- **Mood**: Clustering by mood attributes with transition optimization

### FR-024-005: Energy Flow Patterns
The system SHALL implement four energy patterns:
- **Buildup**: Ascending sort from low to high energy
- **Cooldown**: Descending sort from high to low energy
- **Plateau**: Group tracks within narrow energy range
- **Wave**: Alternating high-low pattern with gradual changes

### FR-024-006: Track Recommendations
The system SHALL generate recommendations by:
- Analyzing current track attributes (BPM, key, genre, energy)
- Scoring library tracks against current track (0-100)
- Applying weighted scoring: key (30%), BPM (25%), energy (20%), genre (15%), rating (10%)
- Filtering out recently played tracks
- Limiting results to top 10 matches
- Providing match reasons (e.g., "Same key", "Similar BPM", "Energy match")

### FR-024-007: Match Indicators
The system SHALL flag recommendations with:
- Harmonic match badge (key compatible)
- BPM match badge (within ±5 BPM)
- Energy match badge (similar energy level)
- Genre match badge (same genre)
- Overall match score (0-100 scale)

### FR-024-008: Playlist Generation
The system SHALL generate playlists by:
- Applying all filter criteria
- Running appropriate sequencing algorithm
- Calculating playlist metadata (duration, avg BPM, energy)
- Storing playlist in database
- Assigning unique playlist ID
- Displaying generation timestamp

### FR-024-009: Playlist Storage
The system SHALL store for each playlist:
- Unique ID
- Name and description (optional)
- Playlist type
- Creation timestamp
- Criteria used for generation
- Ordered list of track IDs
- Calculated metadata (duration, avgBpm, energyFlow array)
- Auto-update settings (if applicable)

### FR-024-010: Playlist Display
The system SHALL display playlists with:
- Playlist name and type icon
- Track count and total duration
- Average BPM
- Type-specific metadata
- Mini energy flow graph (for energy playlists)
- Creation/update timestamp
- Edit and delete actions

### FR-024-011: Track List View
For selected playlist, the system SHALL display:
- Ordered track list with sequence numbers
- Track title and artist
- BPM, key, and genre for each track
- Rating stars
- Load to deck buttons (A/B)
- Color coding for transitions (green = smooth, yellow = caution, red = difficult)

### FR-024-012: Playlist Regeneration
The system SHALL allow users to:
- Regenerate playlist with same criteria
- Update playlist with new library additions
- Refresh based on current filters
- Preserve playlist name and settings
- Update generation timestamp

### FR-024-013: Playlist Analysis
The system SHALL analyze playlists for:
- Energy profile (min/max/average/flow pattern)
- BPM profile (min/max/average/variance)
- Transition quality predictions (0-100 per transition)
- Estimated set duration
- Genre diversity score
- Weak transition identification

### FR-024-014: Transition Quality Prediction
The system SHALL predict transition quality based on:
- BPM difference (closer = higher score)
- Key compatibility (harmonic = bonus)
- Energy level changes (gradual = higher score)
- Genre compatibility
- Historical transition success data (if available)
- Score range: 0-100

### FR-024-015: Playlist Deletion
The system SHALL support:
- Delete playlist with confirmation dialog
- Preserve library tracks (only delete playlist reference)
- Remove from playlist list immediately
- Cleanup associated metadata

## Success Criteria

### SC-024-001: Generation Speed
Smart playlist generation completes within 3 seconds for 1000-track library

### SC-024-002: Recommendation Relevance
Track recommendations score 80%+ relevance based on user feedback (accept/reject tracking)

### SC-024-003: Transition Quality
Generated playlists have 85%+ smooth transitions (quality score ≥70)

### SC-024-004: Energy Flow Accuracy
Energy-based playlists follow selected pattern with 95% adherence to intended flow

### SC-024-005: Harmonic Accuracy
Harmonic playlists maintain 90%+ key-compatible transitions following Camelot rules

### SC-024-006: Storage Efficiency
Playlist metadata uses less than 5KB per playlist on average

### SC-024-007: Update Performance
Playlist regeneration completes within 2 seconds for existing playlists

### SC-024-008: User Adoption
DJs create and use 3+ smart playlists on average within first month

## Dependencies

- Track Library (Feature 13) - Required for track metadata and access
- BPM Detection (Feature 3) - Required for BPM-based filtering and sequencing
- Key Detection (Feature 6) - Required for harmonic mixing playlists
- Harmonic Mixing (Feature 6 extended) - Required for Camelot wheel rules
- Mix History (Feature 23) - Optional for recently played exclusion

## Assumptions

1. Library contains sufficient tracks (50+) for meaningful playlists
2. Tracks have accurate metadata (BPM, key, genre)
3. Users understand playlist type concepts (energy flow, harmonic mixing)
4. Energy level analysis can be approximated from BPM and spectral features
5. Generated playlists are starting points, subject to DJ curation

## Constraints

- Playlist generation limited to 200 tracks maximum for performance
- Recommendation engine limited to top 10 results
- Energy flow analysis requires pre-analyzed tracks (BPM + basic audio features)
- Harmonic playlists require key detection for all tracks
- Maximum 50 smart playlists per user for storage management

## Out of Scope

- Cloud-based playlist sharing
- Collaborative playlist editing
- Real-time playlist updates during performance
- Integration with streaming services (Spotify, Apple Music)
- AI-trained mood/vibe detection (using rule-based approximation)
- Automatic playlist naming/tagging
- Playlist versioning and history
- Export to external DJ software formats
- Crowd-sourced playlist templates
- Machine learning-based recommendation engine

## Key Entities

### SmartPlaylist
- id: string
- name: string
- type: PlaylistType ('energy' | 'harmonic' | 'bpm' | 'genre' | 'mood')
- criteria: PlaylistCriteria
- tracks: TrackMetadata[]
- totalDuration: number
- averageBpm: number
- energyFlow: number[] (energy per track)
- createdAt: Date
- updatedAt: Date

### PlaylistCriteria
- bpmRange: { min: number, max: number }
- keyMatching: boolean
- genres: string[]
- minRating: number (0-5)
- energyFlow: 'buildup' | 'cooldown' | 'plateau' | 'wave'
- excludePlayed: boolean
- maxDuration: number (optional, seconds)

### TrackRecommendation
- track: TrackMetadata
- score: number (0-100)
- harmonicMatch: boolean
- bpmMatch: boolean
- energyMatch: boolean
- genreMatch: boolean
- reasons: string[] (explanation of match factors)

### PlaylistAnalysis
- energyProfile: { min: number, max: number, average: number, flow: string }
- bpmProfile: { min: number, max: number, average: number, variance: number }
- transitions: TransitionPrediction[]
- estimatedSetDuration: number
- genreDiversity: number (0-1 scale)
- weakTransitions: TransitionPrediction[] (quality <70)

### TransitionPrediction
- fromTrack: string (track name)
- toTrack: string (track name)
- quality: number (0-100)
- bpmDifference: number
- keyCompatible: boolean
- energyChange: number
- warnings: string[]

### PlaylistType
- 'energy': Energy flow-based sequencing
- 'harmonic': Key-compatible transitions
- 'bpm': BPM progression
- 'genre': Genre-based grouping
- 'mood': Mood/vibe similarity
