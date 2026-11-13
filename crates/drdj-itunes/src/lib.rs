//! Import-only iTunes/Music XML. Never opens a binary library or modifies audio.
mod paths;
mod xml;
pub use paths::{default_xml_paths, inspect, FileInfo, Prepared, PreparedTrack};
pub use xml::{parse, Import, Playlist, Track, MAX_LINKS, MAX_TRACKS};
