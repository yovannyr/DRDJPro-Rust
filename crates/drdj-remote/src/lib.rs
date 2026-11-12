//! Generic RGB remote display and standard MIDI input. No proprietary protocol.
mod pads;
mod render;
mod worker;
pub use pads::{PadEvent, PadProfile, PadState, VelocityCurve};
pub use render::{render, Config, Cue, Frame, Resolution, Snapshot, Widgets};
pub use worker::DisplayWorker;
