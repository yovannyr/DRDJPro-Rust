# Feature Specification: Direct libusb Support

**Feature Branch**: `28-direct-libusb`  
**Created**: 2026-11-11  
**Status**: Software implemented — USB finder, generic HID mapping/capture and optional GPL-compatible xwax DVS; native hardware, visual, listening and timing acceptance deferred
**Input**: Serato DJ Pro uses `libusb-1.0.0.dylib` (319 KB) for direct USB device access. DRDJPro-Rust uses the `midir` crate for MIDI controllers but lacks direct USB access for non-MIDI devices (e.g., specific DVS turntables, specialized controllers).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - USB Device Enumeration (Priority: P1)

As a DJ connecting a USB controller, I want the application to automatically detect and enumerate all connected USB devices so that I can identify supported hardware.

**Why this priority**: Without device enumeration, users cannot connect USB controllers, DVS turntables, or specialized hardware.

**Independent Test**: Can be tested by connecting a USB device and verifying that the application lists it with correct vendor/product IDs.

**Acceptance Scenarios**:

1. **Given** a USB controller is connected, **When** the application enumerates devices, **Then** the device is listed with correct vendor ID, product ID, and manufacturer name
2. **Given** multiple USB devices are connected, **When** enumeration runs, **Then** all devices are listed individually
3. **Given** a USB device is disconnected, **When** the device list is refreshed, **Then** the device is removed from the list
4. **Given** a USB device is connected that matches a supported device, **When** enumeration completes, **Then** the device is marked as compatible

---

### User Story 2 - Direct USB Control Transfers (Priority: P1)

As a USB controller, I want to send control transfers directly to configure the device (e.g., LED brightness, display initialization) so that the device is ready for use.

**Why this priority**: Some devices require control transfers for initialization that MIDI cannot provide.

**Independent Test**: Can be tested by connecting a USB device and sending a control transfer to configure an LED or display.

**Acceptance Scenarios**:

1. **Given** a USB device is connected, **When** a control transfer is sent, **Then** the device receives the configuration command
2. **Given** a control transfer fails (device not ready), **When** retried, **Then** the transfer succeeds after the device initializes
3. **Given** a control transfer is sent with invalid parameters, **When** the device rejects it, **Then** the error is reported to the caller
4. **Given** multiple control transfers are queued, **When** processed sequentially, **Then** all commands are applied in order

---

### User Story 3 - DVS Timecode Detection (Priority: P1)

As a DJ using DVS (Digital Vinyl System), I want timecode signals from USB turntables to be detected and decoded so that I can control digital tracks with analog records.

**Why this priority**: DVS is a core DJ feature for analog-style control over digital files.

**Independent Test**: Can be tested by playing a timecode record and verifying that position tracking and speed control work accurately.

**Acceptance Scenarios**:

1. **Given** a timecode record is spinning, **When** USB data is received, **Then** the timecode signal is decoded correctly
2. **Given** the record is slowed down by 5%, **When** timecode is decoded, **Then** the playback speed adjusts accordingly
3. **Given** the record is skipped forward, **When** timecode jumps, **Then** the track position updates to the new location
4. **Given** the timecode signal is lost (record removed), **When** the device detects silence, **Then** playback pauses

---

### User Story 4 - HID Device Support (Priority: P2)

As a user with a HID controller (e.g., touchpad, encoder), I want the application to receive HID events so that I can control the DJ software with non-standard devices.

**Why this priority**: Some modern controllers use HID for touchpads, OLED displays, and advanced encoders that MIDI cannot handle.

**Independent Test**: Can be tested by connecting an HID device and verifying that events are received and processed.

**Acceptance Scenarios**:

1. **Given** an HID touchpad is connected, **When** the user swipes, **Then** the swipe is translated to a deck navigation command
2. **Given** an HID encoder is rotated, **When** the rotation amount exceeds the threshold, **Then** the pitch fader moves proportionally
3. **Given** an HID device with an OLED display, **When** data is sent, **Then** the display shows the assigned widget (track info, level meter)
4. **Given** an HID device is unplugged, **When** the disconnection event is received, **Then** the device is removed from the controller list

---

## Edge Cases

- What happens when multiple applications compete for the same USB device?
- How are USB devices with multiple interfaces (MIDI + HID + Vendor) handled?
- What happens when a USB device disconnects unexpectedly during playback?
- How are devices with firmware bugs (malformed descriptors) handled?
- What happens when the USB bus is under high load (dropped packets)?
- How are virtual USB devices (USB-over-IP) handled?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST enumerate all USB devices connected to the system using `libusb`
- **FR-002**: System MUST display device information (vendor ID, product ID, manufacturer, serial number, device name)
- **FR-003**: System MUST support control transfers for device configuration (GET/SET descriptors, bulk/clear feature)
- **FR-004**: System MUST support bulk transfers for high-bandwidth data (DVS timecode, HID reports)
- **FR-005**: System MUST support interrupt transfers for low-latency HID events
- **FR-006**: System MUST detect USB device connection and disconnection events in real time
- **FR-007**: System MUST support DVS timecode decoding from USB turntables
- **FR-008**: System MUST support HID devices via `hidapi` for touchpads, encoders, and OLED displays
- **FR-009**: System MUST claim and release USB interfaces correctly to avoid conflicts
- **FR-010**: System MUST handle USB errors gracefully (device not found, access denied, timeout)
- **FR-011**: System MUST maintain a device registry of supported USB controllers with vendor/product ID mappings
- **FR-012**: System MUST support multiple USB devices connected simultaneously
- **FR-013**: System MUST provide a device-finder UI showing connected devices and compatibility status
- **FR-014**: System MUST isolate USB I/O in separate async tasks to prevent blocking the main thread
- **FR-015**: System MUST support hot-plugging of USB devices without requiring application restart

### Key Entities

- **USB Device**: Represents a connected USB device with vendor/product ID, descriptors, and open interface handles
- **USB Interface**: A logical interface on a USB device (e.g., MIDI interface, Vendor interface)
- **USB Endpoint**: A data endpoint on an interface (bulk, interrupt, isochronous)
- **DVS Timecode Signal**: Decoded timecode data including position, speed, and direction
- **HID Report**: Raw HID event data (touchpad swipe, encoder rotation, button press)
- **Device Registry**: Mapping of vendor/product IDs to device configurations (VID/PID → controller profile)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: USB device enumeration completes within 500 ms on a system with 10 connected USB devices
- **SC-002**: Control transfer latency is under 5 ms for 99% of transfers
- **SC-003**: DVS timecode position accuracy is within ±10 ms of the analog record position
- **SC-004**: USB device disconnection is detected within 500 ms
- **SC-005**: HID report latency is under 10 ms at 125 Hz polling rate
- **SC-006**: 100% of supported USB controllers in the device registry are detected and initialized correctly
- **SC-007**: Hot-plugged USB devices are recognized within 2 seconds without application restart
- **SC-008**: USB I/O tasks do not block the audio processing thread (zero-blocking guarantee)

## Dependencies

- `libusb` crate (0.9) for direct USB device access
- `hidapi` crate (2) for HID device support
- `midir` crate (0.10) for MIDI-over-USB (existing)
- USB backend drivers (OS-level: libusb on Linux, libusbK on Windows, IOUSBLib on macOS)

## Assumptions

- USB backend drivers are installed on the host system (libusbK on Windows, libusb on Linux, native IOUSBLib on macOS)
- Users have permission to access USB devices (no root/admin required on Linux via udev rules)
- DVS timecode signals conform to the Serato Ink or Serato Timecode V2 specification
- HID descriptors are standard-compliant and parseable

## Technical Constraints

- USB I/O must run on a dedicated thread or async task to prevent blocking
- Device enumeration must not require exclusive access (read-only discovery)
- DVS timecode decoding must be performed in real time (no post-processing)
- HID reports must be buffered to prevent data loss at high polling rates
- USB transfers must not exceed 16 KB per bulk transfer (OS-dependent limit)

## Out of Scope

- USB device firmware updates (OTA flashing)
- Custom USB protocol reverse engineering (only documented protocols are supported)
- Virtual USB device emulation (USB passthrough to VMs)
- USB power management (device suspend/resume)
- USB 4/Thunderbolt daisy-chaining optimization
