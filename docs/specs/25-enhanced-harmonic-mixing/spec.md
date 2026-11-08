# Feature 25: Enhanced Harmonic Mixing

**Status**: In Development  
**Priority**: LOW  
**Category**: Advanced Mixing

## Overview

Advanced harmonic mixing system with Camelot wheel visualization, intelligent key-based track finder, and comprehensive key distribution analytics for creating seamless harmonic transitions.

## User Scenarios

### US-025-001: Camelot Wheel Visualization

**Given** a DJ opens the Harmonic Mixing panel  
**When** they view the Camelot wheel  
**Then** they see:

- Interactive circular visualization with 24 positions
- Outer circle showing 12 major keys (B notation, e.g., 8B, 1B)
- Inner circle showing 12 minor keys (A notation, e.g., 5A, 1A)
- Color-coded positions for visual distinction
- Current deck tracks highlighted with deck colors
- Clickable key positions for selection and exploration

### US-025-002: Compatible Key Discovery

**Given** a DJ selects a key on the Camelot wheel  
**When** they view the key information panel  
**Then** the system displays:

- Selected key name and Camelot code
- List of 7 compatible keys with transition details:
  - **Perfect transitions**: Same key, perfect fifth, perfect fourth, relative major/minor
  - **Energy boost**: +1 semitone
  - **Energy drop**: -1 semitone
  - **Dramatic shifts**: ±2 or more semitones
- Difficulty rating (easy/medium/hard)
- Transition description explaining the harmonic relationship
- Pro tips for smooth mixing

### US-025-003: Key-Based Track Finder

**Given** a DJ has a track loaded on a deck  
**When** they activate the track finder  
**Then** the system:

- Identifies the current track's key
- Finds all compatible keys using Camelot rules
- Filters library for tracks in those keys
- Sorts results by transition difficulty (easy first)
- Displays track details (title, artist, BPM, key, genre)
- Provides load-to-deck buttons for each result

### US-025-004: Library Key Analytics

**Given** a DJ opens the key analyzer view  
**When** they view key distribution  
**Then** they see:

- Grid showing all 24 keys (12 major + 12 minor)
- Track count per key
- Percentage of library in each key
- Color-coded visualization bars
- Most common keys ranked list (top 5)
- Distribution balance insights

### US-025-005: Real-Time Deck Monitoring

**Given** tracks are playing on both decks  
**When** the DJ views the Harmonic Mixing panel  
**Then** they see:

- Current deck A track with key highlighted on wheel
- Current deck B track with key highlighted on wheel
- Visual indicator of key compatibility between decks
- BPM information for both tracks
- Deck-specific color coding (blue for A, orange for B)
- Pulsing animation for actively playing decks

### US-025-006: Mixing Strategy Recommendations

**Given** a DJ is viewing mixing suggestions  
**When** they examine the suggestions panel  
**Then** they see categorized advice:

- **Perfect Match**: Same key or adjacent wheel positions
- **Energy Shift**: Semitone transitions for energy changes
- **Major/Minor Switch**: Inner/outer circle transitions for mood changes
- **Advanced Technique**: Complex transitions with high difficulty
- Color-coded recommendation cards (green/yellow/blue/red)
- Practical mixing tips for each strategy

## Functional Requirements

### FR-025-001: Camelot Wheel Data Model
The system SHALL maintain Camelot wheel mapping with:
- 24 positions (12 major + 12 minor keys)
- Camelot codes (1A-12A for minor, 1B-12B for major)
- Musical key names (C, C#, D, Eb, E, F, F#, G, Ab, A, Bb, B with major/minor)
- Visual coordinates for 600x600px wheel rendering
- Color assignments per position for visual distinction

### FR-025-002: Wheel Visualization
The system SHALL render interactive Camelot wheel with:
- SVG-based scalable graphics
- 600x600 pixel default size
- Outer circle radius: 260px (major keys)
- Inner circle radius: 160px (minor keys)
- Clickable key positions (28px radius circles)
- Connection lines between adjacent keys
- Responsive hover states
- Selected key highlighting (35px radius)
- Current deck highlighting with pulsing animation

### FR-025-003: Key Compatibility Rules
The system SHALL implement Camelot key compatibility rules:
- **Perfect (Easy)**: Same key, ±1 position on wheel, relative major/minor
- **Energy Boost (Medium)**: +1 semitone (e.g., 8B → 9B)
- **Energy Drop (Medium)**: -1 semitone (e.g., 8B → 7B)
- **Dramatic (Hard)**: ±2+ semitones or opposite wheel positions
- Major/minor switches within same Camelot number

### FR-025-004: Transition Classification
For each key pair, the system SHALL determine:
- Transition type: 'perfect' | 'energy-boost' | 'energy-drop' | 'dramatic'
- Difficulty level: 'easy' | 'medium' | 'hard'
- Musical relationship (e.g., "Perfect fifth", "Relative minor")
- Mixing guidance description
- Compatibility score (implied by type/difficulty)

### FR-025-005: Compatible Keys List
When a key is selected, the system SHALL generate list of:
- 6-7 most compatible keys (prioritizing easy transitions)
- Each entry containing: target key, transition type, difficulty, description
- Sorted by difficulty (easy first, then medium, then hard)
- Visual indicators matching Camelot wheel colors

### FR-025-006: Track Finder
The system SHALL provide track finder functionality:
- Accepts source deck selection (A or B)
- Retrieves source track's key
- Generates list of compatible keys using Camelot rules
- Filters library tracks matching compatible keys
- Excludes source track from results
- Sorts by difficulty (easy transitions first)
- Limits results to reasonable count (50-100 tracks)

### FR-025-007: Track Finder Display
For each found track, the system SHALL display:
- Track title and artist name
- Musical key with colored badge
- BPM with colored badge
- Genre label
- Transition difficulty badge (easy/medium/hard)
- Transition type label (perfect/energy-boost/energy-drop)
- Transition description
- Load to Deck A/B action buttons

### FR-025-008: Deck Integration
The system SHALL integrate with deck state:
- Monitor current track on Deck A
- Monitor current track on Deck B
- Display current keys in header panel
- Highlight corresponding keys on Camelot wheel
- Update track finder source based on selected deck
- Provide deck-specific color coding (blue/orange)

### FR-025-009: Key Distribution Analytics
The system SHALL analyze library and display:
- Track count per key (all 24 keys)
- Percentage of library per key
- Visual bar charts for distribution
- Color-coded bars matching Camelot wheel
- Top 5 most common keys ranked list
- Balanced/unbalanced library indicators

### FR-025-010: Key Analyzer View
The system SHALL provide key analyzer view with:
- 6x4 grid (or 8x3) showing all keys
- Per-key statistics: name, Camelot code, count, percentage, bar chart
- Color-coded borders matching wheel colors
- Interactive selection (clicking loads track finder for that key)
- Distribution visualization
- Library balance score (0-100)

### FR-025-011: Mixing Strategy Tips
The system SHALL display four recommendation categories:
- **Perfect Match** (green card): Same key, adjacent positions, relative major/minor
- **Energy Shift** (yellow card): Semitone up/down for energy changes
- **Major/Minor Switch** (blue card): Inner/outer circle for mood variation
- **Advanced Technique** (red card): Complex jumps requiring skill
- Each with icon, title, and practical guidance text

### FR-025-012: View Modes
The system SHALL support three view modes:
- **Wheel**: Camelot wheel visualization with key info panel
- **Finder**: Track finder with filtering and deck selection
- **Analyzer**: Key distribution analytics and library statistics
- Toggle buttons to switch between modes
- Persistent view state during session

### FR-025-013: Interactive Elements
The system SHALL support user interactions:
- Click key on wheel → select key and show info
- Click track in finder → option to load to deck A or B
- Hover over key → visual feedback (opacity change)
- Deck selection toggle (A/B) in finder mode
- View mode switching buttons

### FR-025-014: Color Coding
The system SHALL use consistent color coding:
- Deck A: Blue (#3b82f6)
- Deck B: Orange (#f97316)
- Perfect transitions: Green
- Energy boost: Yellow/Orange
- Energy drop: Purple
- Dramatic shifts: Red
- Each Camelot position: Unique color from spectrum

### FR-025-015: Performance Optimization
The system SHALL optimize performance by:
- Memoizing track finder results for same source key
- Caching key distribution analytics
- Using efficient SVG rendering for wheel
- Debouncing wheel interactions
- Lazy loading track lists (virtualization for 100+ tracks)

## Success Criteria

### SC-025-001: Visual Clarity
95% of test users can identify their current track's key position on the Camelot wheel within 5 seconds

### SC-025-002: Recommendation Accuracy
Compatible key suggestions result in 90%+ harmonically smooth transitions as rated by DJs

### SC-025-003: Response Time
Track finder returns results within 300ms for libraries up to 5000 tracks

### SC-025-004: Key Detection Accuracy
Key distribution analytics accurately reflect library composition with 100% precision (no counting errors)

### SC-025-005: User Comprehension
80% of novice DJs understand Camelot wheel concept after 10 minutes of use

### SC-025-006: Interaction Performance
Wheel interactions (clicks, hovers) respond within 50ms with no perceived lag

### SC-025-007: Mixing Improvement
DJs using harmonic mixing feature report 40% increase in transition smoothness after 1 month

### SC-025-008: Feature Adoption
60% of users enable and regularly use harmonic mixing features within first week

## Dependencies

- Key Detection (Feature 6) - REQUIRED for track key analysis
- Track Library (Feature 13) - Required for track metadata and filtering
- BPM Detection (Feature 3) - Optional for BPM display in track finder
- Harmonic Mixing Assistant (existing component) - Foundation for enhanced features

## Assumptions

1. Users have basic understanding of musical keys (major/minor)
2. Track library has key detection completed for majority of tracks
3. Camelot wheel notation is industry-standard and familiar to many DJs
4. Visual color coding aids in learning and quick recognition
5. Harmonic mixing is desired by intermediate/advanced DJs

## Constraints

- Wheel visualization fixed at 600x600px for consistent layout
- Camelot wheel limited to 24 positions (12 major, 12 minor keys)
- Track finder results limited to 100 tracks for UI performance
- Key distribution analytics requires pre-analyzed tracks (cannot analyze on-the-fly)
- Enharmonic equivalents (e.g., C# vs Db) must be normalized to single representation

## Out of Scope

- Custom key notation systems (beyond Camelot and standard notation)
- Real-time key transposition/pitch shifting
- Automatic key change during transitions
- Multi-deck harmonic analysis (beyond 2 decks)
- Historical key usage analytics over time
- Key-based effect recommendations
- Integration with online key databases
- Crowd-sourced key correction system
- Genre-specific key mixing strategies
- Key signature time-series analysis during playback

## Key Entities

### CamelotPosition
- code: string (e.g., "8B", "5A")
- key: string (e.g., "C", "Am")
- position: { x: number, y: number } (pixel coordinates)
- color: string (hex color code)

### KeyTransition
- from: string (source key)
- to: string (target key)
- type: 'perfect' | 'energy-boost' | 'energy-drop' | 'dramatic'
- difficulty: 'easy' | 'medium' | 'hard'
- description: string (e.g., "Perfect fifth - smooth transition")

### CompatibleTrack
- track: TrackMetadata (full track object)
- transition: KeyTransition (describes the transition from current track)
- visualBadges: { difficulty: string, transitionType: string, keyColor: string }

### KeyDistribution
- key: string
- camelotCode: string
- trackCount: number
- percentage: number
- color: string (for visualization)
- isCommon: boolean (top 5)

### MixingStrategy
- category: 'perfect-match' | 'energy-shift' | 'major-minor-switch' | 'advanced'
- title: string
- description: string
- color: string (card background color)
- icon: ReactElement
- difficulty: 'beginner' | 'intermediate' | 'advanced'
