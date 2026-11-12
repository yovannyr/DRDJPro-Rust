# Spec 28 acceptance ledger

Status after software slice 8: **in progress**. This is not native hardware,
visual, listening or release acceptance. Spec 26 remains stopped.
Current implementation/test details are in [IMPLEMENTATION.md](IMPLEMENTATION.md).

## Functional requirements

| Requirement | Software evidence | Remaining acceptance |
|---|---|---|
| FR-001 USB enumeration | rusb NativeDiscovery, read-only worker and inventory probe | Additional OS/device configurations |
| FR-002 device information | USB finder VID/PID/bus/address/profile; explicit Unicode manufacturer/product/serial queries and field errors | Native display/device string behavior |
| FR-003 control transfers | Managed sessions, query requests, bounded vendor/class transfers, managed string descriptors and clear-halt | Concrete documented firmware operations; arbitrary standard configuration writes deliberately prohibited |
| FR-004 bulk transfers | Validated endpoint/direction, 16KiB bound, short read/write counts | Real bulk hardware; USB audio uses CPAL isochronous OS streams instead of pretending bulk is PCM |
| FR-005 interrupt transfers | Validated interrupt endpoints and bounded deadlines | Real interrupt hardware/latency |
| FR-006 connect/disconnect | Discovery polling + immutable snapshots, errors retain last known devices | Polling is not native callback notification; short between-scan events can be missed |
| FR-007 DVS | Optional GPL xwax decode, CPAL input, explicit A/B ARM and renderer-bound ownership | Physical editions/position/latency/hearing tests; no broad Ink/NoiseMap claim |
| FR-008 HID | HIDAPI metadata, descriptor validation, explicit sessions/reports and buffered acquisition | Generic controller mapping/app selection still pending; real touchpad/encoder/display tests |
| FR-009 claim/release | Worker ownership, managed alternate/endpoint state, RAII, disconnect invalidation | Device-specific driver behavior; no kernel detach or driver installation |
| FR-010 error handling | Typed failures, bounded tickets, terminal policy failures, visible finder/input diagnostics | Native permission/firmware failures |
| FR-011 device registry | Explicit VID/PID registry; duplicates rejected, no invented manufacturer profiles | User-configured controller profiles and their hardware verification |
| FR-012 simultaneous devices | Unique process sessions, distinct equal-VID/PID connections, independent workers and DVS decks | Multi-device native stress/USB bandwidth |
| FR-013 finder UI | Main composition wires lazy USB discovery and explicit strings | HID app selection/mapping still pending; native visual usability |
| FR-014 I/O isolation | Discovery/I/O/HID capture/DVS input own their threads; bounded nonwaiting UI reads | OS calls can block their own worker; no forced cancellation promise |
| FR-015 hotplug | Snapshot updates and HID reconnect epochs; no app restart | Physical plug/unplug timing and documented mapped-controller recovery |

## Measurable outcomes

| Criterion | Evidence / status |
|---|---|
| SC-001 enumeration under 500ms / 10 devices | One Windows read-only probe: 19 devices, 31.91ms. A controlled 10-device/multi-platform benchmark remains open. |
| SC-002 control p99 under 5ms | Unmeasured on hardware; queue/deadline/ordering tests do not prove native p99. |
| SC-003 physical DVS position within ±10ms | Synthetic upstream word-position fixtures pass, including jumps/reverse; physical origin, input/output buffering and calibration remain open. |
| SC-004 disconnect within 500ms | Polling/default intervals tested by injected backends; physical timing remains open. |
| SC-005 HID under 10ms at 125Hz | Bounded reader, 125-packet FIFO fixture and overflow tests; synthetic burst is not a physical latency measurement. |
| SC-006 all supported profiles initialized | No unverified vendor profiles preinstalled. Native per-profile acceptance remains open. |
| SC-007 hotplug within 2 seconds | Recovery semantics tested without hardware; physical deadline remains open. |
| SC-008 no blocking audio processing | Own callback handoff uses preallocated atomics. Renderer tests measure zero Rust allocation and a processing budget. I/O and UI locks stay outside rendering; a universal OS/library zero-blocking guarantee is not claimed. |

## Deferred joint session

Run native visual, interaction, audio-input/output, vinyl/hearing and physical
USB/HID timing tests together with the user. Do not fill hardware checkboxes
from compilation, synthetic fixtures, static review or metadata enumeration.
