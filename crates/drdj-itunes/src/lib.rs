//! Import-only iTunes/Music XML. Never opens a binary library or modifies audio.
mod paths;
mod store;
mod xml;
pub use paths::{default_xml_paths, inspect, refresh_files, FileInfo, Prepared, PreparedTrack};
pub use store::{Snapshot, Source, Store, SyncReport};
pub use xml::{parse, Import, Playlist, Track, MAX_LINKS, MAX_TRACKS};
