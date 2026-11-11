# Feature Specification: Crash Handler

**Feature Branch**: `26-crash-handler`  
**Created**: 2026-11-11  
**Status**: In progress — local store implemented; capture, transport and UI pending
**Input**: Serato DJ Pro uses Google Crashpad for minidump generation and crash reporting. DRDJPro-Rust has `console_error_panic_hook` and `backtrace` but no external crash-handler process.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic Crash Reporting (Priority: P1)

As a user experiencing a crash, I want the application to automatically generate a crash report and minidump so that developers can diagnose the root cause.

**Why this priority**: Crash reports are essential for improving application stability and fixing critical bugs in production.

**Independent Test**: Can be tested by triggering a controlled panic and verifying that a minidump file and crash report are generated.

**Acceptance Scenarios**:

1. **Given** the application crashes with a panic, **When** crash handler is active, **Then** a minidump file is created in the crash database directory
2. **Given** a minidump is created, **When** the application restarts, **Then** the user is asked if they want to send the crash report
3. **Given** the user agrees to send the crash report, **When** upload completes, **Then** a success message is displayed
4. **Given** the user declines to send, **When** the dialog is closed, **Then** the minidump is preserved locally for later manual submission

---

### User Story 2 - Crash Report Submission (Priority: P1)

As a developer, I want to receive crash reports with stack traces so that I can identify and fix the root cause of crashes.

**Why this priority**: Without crash reports, debugging production issues is extremely difficult and time-consuming.

**Independent Test**: Can be tested by simulating a crash and verifying that the crash report contains valid stack trace information.

**Acceptance Scenarios**:

1. **Given** a minidump is generated, **When** crash report is created, **Then** stack trace includes function names and file/line numbers
2. **Given** a crash report is created, **When** report includes annotations, **Then** application version and OS information are included
3. **Given** a crash report is ready, **When** user submits it, **Then** the report is sent asynchronously without blocking UI
4. **Given** the upload server is unavailable, **When** crash report submission fails, **Then** the report is queued for retry

---

### User Story 3 - Crash Report Management (Priority: P2)

As a user, I want to manage stored crash reports so that I can review, resend, or delete them.

**Why this priority**: Users may want to see what crashes occurred and control data sharing.

**Independent Test**: Can be tested by generating multiple crash reports and verifying they can be listed, reviewed, and deleted.

**Acceptance Scenarios**:

1. **Given** one or more crash reports are stored, **When** user opens crash report manager, **Then** list of all stored reports is displayed
2. **Given** a crash report is selected, **When** user reviews details, **Then** timestamp, version, OS, and stack trace summary are shown
3. **Given** a crash report is displayed, **When** user deletes it, **Then** the minidump file is removed from disk
4. **Given** a crash report failed to upload, **When** user resubmits it, **Then** the report is resent to the server

---

### User Story 4 - Crash Handler Service (Priority: P1)

As an application, I want a dedicated crash-handler process that monitors the main application and captures crashes that occur in separate threads or native code.

**Why this priority**: Some crashes (e.g., in audio callback threads or native FFI calls) cannot be caught by Rust's `panic!` handler alone.

**Independent Test**: Can be tested by verifying that the crash-handler process is launched alongside the main application and monitors it.

**Acceptance Scenarios**:

1. **Given** the main application starts, **When** crash handler is configured, **Then** the crash-handler process is launched as a child process
2. **Given** the crash-handler is running, **When** the main application crashes, **Then** the handler captures the crash and generates a minidump
3. **Given** the main application exits normally, **When** the crash-handler detects termination, **Then** it exits cleanly after a short delay
4. **Given** the main application crashes, **When** the crash-handler times out, **Then** it exits to avoid orphaned processes

---

## Edge Cases

- What happens when the crash occurs during crash-handler initialization?
- How are crashes handled when disk space is insufficient?
- What happens if the crash-report server returns a 403 error?
- How are concurrent crashes handled?
- What happens when the crash-database directory has restrictive permissions?
- How are crashes in audio callbacks (real-time thread) handled without blocking?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST generate minidump files when the application crashes
- **FR-002**: System MUST capture stack traces with function names, file paths, and line numbers
- **FR-003**: System MUST annotate crash reports with application version, OS, and CPU architecture
- **FR-004**: System MUST store crash reports in `~/.local/share/DRDJPro-Rust/crashes/` (Linux/macOS) or `%APPDATA%\DRDJPro-Rust\crashes\` (Windows)
- **FR-005**: System MUST allow users to opt in or opt out of automatic crash-report submission
- **FR-006**: System MUST support manual crash-report submission from a crash-report manager UI
- **FR-007**: System MUST queue crash reports when the server is unavailable and retry on next startup
- **FR-008**: System MUST send crash reports asynchronously without blocking the application
- **FR-009**: System MUST support a dedicated crash-handler process that monitors the main application
- **FR-010**: System MUST capture unhandled Rust panics and native segmentation faults
- **FR-011**: System MUST display a post-crash dialog asking the user whether to submit the crash report
- **FR-012**: System MUST log all crash-related events for debugging
- **FR-013**: System MUST support uploading crash reports over HTTPS
- **FR-014**: System MUST limit the number of stored crash reports to 50 to prevent disk exhaustion
- **FR-015**: System MUST compress crash reports using gzip when uploading
- **FR-016**: System MUST exclude personal file paths from stack traces for privacy

### Key Entities

- **Minidump**: Binary crash snapshot containing memory state, stack trace, and thread information
- **Crash Report**: JSON metadata including version, OS, annotation, attachment list, and minidump path
- **Crash Handler Process**: External process that monitors the main application and captures crashes
- **Crash Queue**: Persistent list of queued crash reports pending upload
- **Crash Database Directory**: Local storage directory for minidump files and crash reports

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Minidump generation completes within 5 seconds of a crash for 99% of crash types
- **SC-002**: Stack traces include correct function names and source locations for 95% of Rust panics
- **SC-003**: Crash reports are uploaded successfully within 10 seconds when the server is available
- **SC-004**: Crash reports are never lost: 100% of crashes generate a minidump file
- **SC-005**: Crash handler process exits within 30 seconds after the main application terminates
- **SC-006**: Disk usage for crash reports does not exceed 500 MB (50 reports × 10 MB average)
- **SC-007**: Crash-report submission does not increase application shutdown time by more than 5 seconds
- **SC-008**: Users can successfully submit, delete, and review crash reports from the UI

## Dependencies

- `backtrace` crate for stack-trace capture
- `minidump` crate for minidump generation
- `reqwest` crate for HTTP upload
- `tokio` runtime for async operations
- `dirs` crate for platform-specific cache directories
- Crash-report server endpoint (to be configured)

## Assumptions

- Crash reports are sent over HTTPS to a configurable server URL
- Users have network connectivity when submitting crash reports
- The minimum Rust version is 1.75 (for stable `backtrace` support on all platforms)
- Crash-handler process does not require admin/root privileges
- Minidump files do not contain sensitive data (credentials, private keys, etc.)

## Technical Constraints

- Minidump generation must not block the audio processing thread
- Crash handler must be platform-specific (Windows: `.exe`, macOS: `.app`, Linux: ELF binary)
- Stack traces in native FFI code may lack line numbers
- Some crashes (e.g., out-of-memory) may prevent crash-handler execution
- Uploads are best-effort: failures must not prevent normal application shutdown

## Out of Scope

- Automatic crash-report submission without user consent (GDPR requires opt-in)
- Crash-report de-duplication (duplicate reports are sent separately)
- Crash-report visualization beyond stack trace display
- Integration with third-party crash-analytics services (Sentry, Bugsnag) in this version
- Live crash monitoring and remote debugging
