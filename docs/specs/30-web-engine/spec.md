# Feature Specification: Embedded Web Engine

**Feature Branch**: `30-web-engine`  
**Created**: 2026-11-11  
**Status**: Planned  
**Input**: Serato DJ Pro uses `QtWebEngineQuick` and `QtWebEngineCore` for an embedded browser for Beatport streaming interface, documentation, and help. DRDJPro-Rust uses Slint which has no built-in browser engine.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Embedded Browser for Beatport (Priority: P1)

As a DJ with a Beatport subscription, I want to browse and stream tracks directly from Beatport within the application so that I can discover and play music without switching to an external browser.

**Why this priority**: Integrated streaming discovery is a key workflow for modern DJs; opening an external browser interrupts the mixing flow.

**Independent Test**: Can be tested by launching the embedded browser and navigating to Beatport, verifying that authentication and track playback work.

**Acceptance Scenarios**:

1. **Given** the user opens the streaming tab, **When** the embedded browser loads, **Then** the Beatport login page is displayed
2. **Given** the user logs in to Beatport, **When** authentication completes, **Then** the Beatport catalog is shown
3. **Given** a track is streamed from Beatport, **When** the stream starts, **Then** audio playback begins within 2 seconds
4. **Given** the user drops the streamed track onto a deck, **When** the drop is accepted, **Then** the track is loaded and ready for playback

---

### User Story 2 - Navigation and History (Priority: P1)

As a user browsing the embedded browser, I want to navigate back and forward and manage browser history so that I can move between pages efficiently.

**Why this priority**: Standard browser navigation is essential for a good user experience in any embedded web view.

**Independent Test**: Can be tested by navigating through multiple pages and verifying back/forward buttons work correctly.

**Acceptance Scenarios**:

1. **Given** the user navigates through 5 pages, **When** the back button is clicked, **Then** the previous page is displayed
2. **Given** the user goes back and then forward, **When** forward is clicked, **Then** the current page is restored
3. **Given** the user reloads a page, **When** the reload completes, **Then** the page is refreshed
4. **Given** the user types a URL, **When** Enter is pressed, **Then** the browser navigates to that URL

---

### User Story 3 - In-App Help and Documentation (Priority: P2)

As a new user, I want to access help documentation and tutorials within the embedded browser so that I can learn the application without leaving the UI.

**Why this priority**: Built-in help improves the onboarding experience and reduces support requests.

**Independent Test**: Can be tested by opening the help page and verifying that documentation content is rendered correctly.

**Acceptance Scenarios**:

1. **Given** the user opens the help page, **When** the help browser loads, **Then** the documentation home page is displayed
2. **Given** a help topic is selected, **When** the link is followed, **Then** the topic page is rendered
3. **Given** a help page contains images or code examples, **When** the page loads, **Then** all assets are displayed correctly
4. **Given** the help page is offline-capable, **When** the network is unavailable, **Then** cached pages are shown

---

### User Story 4 - GPU-Accelerated Rendering (Priority: P2)

As the application, I want the embedded browser to use GPU acceleration so that web content renders smoothly without impacting audio performance.

**Why this priority**: GPU acceleration offloads rendering from the CPU, preventing audio glitches during heavy web content rendering.

**Independent Test**: Can be tested by loading a complex web page and monitoring GPU usage and audio callback latency.

**Acceptance Scenarios**:

1. **Given** a complex web page with CSS animations is loaded, **When** the page is rendered, **Then** GPU acceleration is used (CPU usage < 10%)
2. **Given** the web engine is rendering, **When** audio playback is active, **Then** no audio glitches occur
3. **Given** the web view is minimized, **When** rendering continues, **Then** GPU usage drops to near zero
4. **Given** the GPU is unavailable, **When** the browser initializes, **Then** it falls back to software rendering gracefully

---

## Edge Cases

- What happens when the embedded browser runs out of memory?
- How are JavaScript errors handled in the embedded web view?
- What happens when a website requires a popup window?
- How are HTTPS certificate errors handled?
- What happens when the network is slow and pages time out?
- How are local files (e.g., help documentation) served from within the application?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST embed a web browser widget using the `webview` crate (0.14) or equivalent
- **FR-002**: System MUST load and display remote web pages (e.g., Beatport, documentation)
- **FR-003**: System MUST support standard browser navigation (back, forward, reload, URL bar)
- **FR-004**: System MUST maintain independent browser history per web view
- **FR-005**: System MUST support JavaScript execution in web views
- **FR-006**: System MUST support OAuth authentication flows for streaming providers
- **FR-007**: System MUST support GPU-accelerated rendering when available
- **FR-008**: System MUST fall back to software rendering when GPU acceleration is unavailable
- **FR-009**: System MUST provide a Slint container (`ui/web_view.slint`) for hosting web views
- **FR-010**: System MUST isolate web view cookies and storage from the main application
- **FR-011**: System MUST support loading local HTML files from the application bundle
- **FR-012**: System MUST handle navigation errors (404, 500, network timeout) gracefully
- **FR-013**: System MUST support custom User-Agent header (`DRDJPro-Rust/0.1.0`)
- **FR-014**: System MUST allow embedding of third-party web content with CSP compliance

### Key Entities

- **WebView**: Embedded browser widget with navigation, history, and rendering
- **WebEngine**: Browser initialization and configuration (GPU acceleration, User-Agent, storage path)
- **Browser Navigation State**: Current URL, history stack, loaded pages
- **OAuth Session**: Authentication session for streaming providers (tokens, expiry)
- **Local Resource Loader**: File-based loader for embedded help and documentation

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Web page load time is under 5 seconds for typical Beatport pages on broadband
- **SC-002**: GPU-accelerated rendering uses < 10% CPU for standard web pages
- **SC-003**: No audio glitches occur during web view rendering under normal load
- **SC-004**: Back/forward navigation works correctly with a history depth of at least 50 pages
- **SC-005**: OAuth authentication flows complete successfully for Beatport and supported providers
- **SC-006**: Local help documentation renders correctly with all images and code examples
- **SC-007**: Web view initialization completes within 2 seconds
- **SC-008**: Memory usage of the web view does not exceed 500 MB for extended browsing sessions

## Dependencies

- `webview` crate (0.14) for embedded browser widget
- `tokio` runtime for async operations (e.g., OAuth callbacks)
- `reqwest` crate for HTTP requests (e.g., token refresh)
- GPU drivers (OS-dependent: DirectX on Windows, OpenGL/Metal on macOS, OpenGL/Vulkan on Linux)
- Streaming provider API keys (Beatport, etc.)

## Assumptions

- The `webview` crate uses the native OS web view (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux)
- Streaming providers support OAuth 2.0 authentication
- Users have an active internet connection for streaming content
- Local help documentation is bundled with the application and served from `res/help/`

## Technical Constraints

- Web view must run on the main thread (OS limitation for native web views)
- GPU acceleration may not be available on all platforms (software fallback required)
- Web view memory usage can grow significantly (limit with periodic GC)
- OAuth flows require a local HTTP server for redirect URIs
- CSP headers from embedded pages must not block local resource loading

## Out of Scope

- Full Chrome/Chromium bundling (uses OS-native web view instead)
- Extension/plugin support for web views
- WebRTC or peer-to-peer communication within web views
- PDF rendering within web views (use dedicated PDF viewer)
- Multiple web view instances per deck (one web view per tab)
