use quick_xml::{events::Event, Reader};
use std::{
    collections::{BTreeMap, HashSet},
    io::BufRead,
};
/// Deliberate engineering limits, independent of normal file-picker batch size.
pub const MAX_TRACKS: usize = 250_000;
pub const MAX_LINKS: usize = 1_000_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub id: u64,
    pub persistent_id: String,
    pub location: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub bpm: Option<f64>,
    pub rating: Option<u8>,
    pub play_count: Option<u32>,
    pub duration_ms: Option<u64>,
    pub date_added: Option<String>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<u64>,
}
#[derive(Clone, Debug, Default)]
pub struct Import {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub music_folder: Option<String>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}
impl Import {
    fn warning(&mut self, message: String) {
        self.skipped += 1;
        if self.warnings.len() < 50 {
            self.warnings.push(message);
        }
    }
}
enum Value {
    Invalid,
    Text(String),
    Integer(i64),
    Real(f64),
    Bool(bool),
    Dict(BTreeMap<String, Value>),
    Array(Vec<Value>),
}
impl Value {
    fn text(&self) -> Option<&str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn integer(&self) -> Option<i64> {
        if let Self::Integer(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}
struct Xml<'a, R: BufRead> {
    reader: Reader<std::io::Take<R>>,
    buf: Vec<u8>,
    nodes: usize,
    cancel: &'a dyn Fn() -> bool,
}
impl<R: BufRead> Xml<'_, R> {
    fn next(&mut self) -> Result<Event<'static>, String> {
        loop {
            let event = self
                .reader
                .read_event_into(&mut self.buf)
                .map_err(|e| format!("Invalid iTunes XML: {e}"))?
                .into_owned();
            self.buf.clear();
            match &event {
                Event::Decl(_) | Event::Comment(_) | Event::PI(_) => continue,
                Event::DocType(v) => {
                    if v.as_ref().contains(&b'[') {
                        return Err("Internal XML entities are not supported".into());
                    }
                    continue;
                }
                Event::Text(v) if v.as_ref().iter().all(u8::is_ascii_whitespace) => continue,
                _ => return Ok(event),
            }
        }
    }
    fn end(&mut self, name: &[u8]) -> Result<(), String> {
        match self.next()? {
            Event::End(e) if e.name().as_ref() == name => Ok(()),
            _ => Err("Malformed iTunes XML element".into()),
        }
    }
    fn text(&mut self, name: &[u8]) -> Result<String, String> {
        let mut value = String::new();
        loop {
            match self
                .reader
                .read_event_into(&mut self.buf)
                .map_err(|e| e.to_string())?
            {
                Event::Text(v) => {
                    let decoded = v.xml10_content().map_err(|e| e.to_string())?;
                    value.push_str(
                        &quick_xml::escape::unescape(&decoded).map_err(|e| e.to_string())?,
                    );
                }
                Event::CData(v) => value.push_str(&v.xml10_content().map_err(|e| e.to_string())?),
                Event::GeneralRef(v) => {
                    let entity = v.decode().map_err(|e| e.to_string())?;
                    let escaped = format!("&{entity};");
                    value.push_str(
                        &quick_xml::escape::unescape(&escaped).map_err(|e| e.to_string())?,
                    );
                }
                Event::End(v) if v.name().as_ref() == name => {
                    self.buf.clear();
                    return Ok(value);
                }
                _ => return Err("Invalid text value in iTunes XML".into()),
            }
            self.buf.clear();
            if value.len() > 32768 {
                return Err("iTunes XML field exceeds32KiB".into());
            }
        }
    }
    fn value(&mut self, event: Event<'static>, depth: usize) -> Result<Value, String> {
        self.nodes += 1;
        if self.nodes.is_multiple_of(1024) && (self.cancel)() {
            return Err("iTunes import cancelled".into());
        }
        if depth > 24 || self.nodes > 12_000_000 {
            return Err("iTunes XML structure limit exceeded".into());
        }
        match event {
            Event::Empty(e) => match e.name().as_ref() {
                b"true" => Ok(Value::Bool(true)),
                b"false" => Ok(Value::Bool(false)),
                b"string" | b"date" | b"data" => Ok(Value::Text(String::new())),
                b"array" => Ok(Value::Array(Vec::new())),
                b"dict" => Ok(Value::Dict(BTreeMap::new())),
                _ => Err("Unsupported empty plist value".into()),
            },
            Event::Start(e) => match e.name().as_ref() {
                b"string" | b"date" | b"data" => Ok(Value::Text(self.text(e.name().as_ref())?)),
                b"integer" => Ok(self
                    .text(b"integer")?
                    .trim()
                    .parse()
                    .map(Value::Integer)
                    .unwrap_or(Value::Invalid)),
                b"real" => Ok(self
                    .text(b"real")?
                    .trim()
                    .parse()
                    .map(Value::Real)
                    .unwrap_or(Value::Invalid)),
                b"true" => {
                    self.end(b"true")?;
                    Ok(Value::Bool(true))
                }
                b"false" => {
                    self.end(b"false")?;
                    Ok(Value::Bool(false))
                }
                b"dict" => {
                    let mut values = BTreeMap::new();
                    loop {
                        match self.next()? {
                            Event::End(e) if e.name().as_ref() == b"dict" => break,
                            Event::Start(e) if e.name().as_ref() == b"key" => {
                                let key = self.text(b"key")?;
                                if key.len() > 256 || values.len() >= 256 {
                                    return Err("Oversized plist dictionary".into());
                                }
                                let event = self.next()?;
                                let value = self.value(event, depth + 1)?;
                                if values.insert(key, value).is_some() {
                                    return Err("Duplicate plist key".into());
                                }
                            }
                            _ => return Err("Malformed plist dictionary".into()),
                        }
                    }
                    Ok(Value::Dict(values))
                }
                b"array" => {
                    let mut values = Vec::new();
                    loop {
                        let event = self.next()?;
                        if matches!(&event,Event::End(e)if e.name().as_ref()==b"array") {
                            break;
                        }
                        if values.len() >= MAX_LINKS {
                            return Err("Oversized plist array".into());
                        }
                        values.push(self.value(event, depth + 1)?);
                    }
                    Ok(Value::Array(values))
                }
                _ => Err("Unsupported plist value".into()),
            },
            _ => Err("Expected plist value".into()),
        }
    }
}
fn text(fields: &BTreeMap<String, Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(Value::text)
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(512)
        .collect()
}
fn integer(fields: &BTreeMap<String, Value>, key: &str) -> Result<Option<u64>, String> {
    fields
        .get(key)
        .map(|v| {
            v.integer()
                .and_then(|n| u64::try_from(n).ok())
                .ok_or_else(|| format!("Invalid {key}"))
        })
        .transpose()
}
fn track(key: &str, value: Value) -> Result<Track, String> {
    let Value::Dict(fields) = value else {
        return Err("Track entry is not a dictionary".into());
    };
    if matches!(fields.get("Protected"), Some(Value::Bool(true)))
        || matches!(fields.get("Apple Music"), Some(Value::Bool(true)))
        || text(&fields, "Kind").to_lowercase().contains("protected")
        || text(&fields, "Kind").to_lowercase().contains("apple music")
    {
        return Err("DRM/subscription track excluded".into());
    }
    let id = integer(&fields, "Track ID")?
        .or_else(|| key.parse().ok())
        .filter(|n| *n > 0)
        .ok_or("Invalid Track ID")?;
    if key.parse::<u64>().ok() != Some(id) {
        return Err("Track dictionary key does not match Track ID".into());
    }
    let location = fields
        .get("Location")
        .and_then(Value::text)
        .ok_or("Track has no local Location")?
        .to_owned();
    if location
        .to_lowercase()
        .split('?')
        .next()
        .unwrap_or_default()
        .ends_with(".m4p")
    {
        return Err("DRM .m4p track excluded".into());
    }
    let bpm = fields
        .get("BPM")
        .map(|v| match v {
            Value::Integer(n) => Ok(*n as f64),
            Value::Real(n) => Ok(*n),
            _ => Err("Invalid BPM"),
        })
        .transpose()?;
    if bpm.is_some_and(|v| !v.is_finite() || !(0.0..=1000.).contains(&v)) {
        return Err("Invalid BPM".into());
    }
    let rating = integer(&fields, "Rating")?
        .map(|n| {
            u8::try_from(n)
                .ok()
                .filter(|n| *n <= 100)
                .ok_or("Invalid rating")
        })
        .transpose()?;
    let play_count = integer(&fields, "Play Count")?
        .map(|n| u32::try_from(n).map_err(|_| "Invalid play count"))
        .transpose()?;
    let duration_ms = integer(&fields, "Total Time")?;
    if duration_ms.is_some_and(|n| n > 604_800_000) {
        return Err("Duration exceeds7days".into());
    }
    let persistent_id = text(&fields, "Persistent ID");
    Ok(Track {
        id,
        persistent_id,
        location,
        title: text(&fields, "Name"),
        artist: text(&fields, "Artist"),
        album: text(&fields, "Album"),
        album_artist: text(&fields, "Album Artist"),
        genre: text(&fields, "Genre"),
        bpm,
        rating,
        play_count,
        duration_ms,
        date_added: fields
            .get("Date Added")
            .and_then(Value::text)
            .map(str::to_owned),
    })
}
fn playlist(value: Value) -> Result<Option<Playlist>, String> {
    let Value::Dict(mut fields) = value else {
        return Err("Playlist entry is not a dictionary".into());
    };
    if matches!(fields.get("Master"), Some(Value::Bool(true)))
        || matches!(fields.get("Folder"), Some(Value::Bool(true)))
    {
        return Ok(None);
    }
    let name = text(&fields, "Name");
    if name.trim().is_empty() {
        return Err("Playlist has no name".into());
    }
    let id = fields
        .get("Playlist Persistent ID")
        .and_then(Value::text)
        .map(str::to_owned)
        .or_else(|| {
            fields
                .get("Playlist ID")
                .and_then(Value::integer)
                .map(|n| n.to_string())
        })
        .ok_or("Playlist has no stable ID")?;
    let values = match fields.remove("Playlist Items") {
        Some(Value::Array(v)) => v,
        None => Vec::new(),
        _ => return Err("Invalid playlist items".into()),
    };
    let mut tracks = Vec::with_capacity(values.len());
    for value in values {
        let Value::Dict(d) = value else {
            return Err("Invalid playlist item".into());
        };
        tracks.push(
            integer(&d, "Track ID")?
                .filter(|n| *n > 0)
                .ok_or("Invalid playlist Track ID")?,
        );
    }
    Ok(Some(Playlist { id, name, tracks }))
}
/// Streaming at the top-level track/playlist boundaries. Cancellation is checked
/// between records. Malformed XML aborts before any catalog mutation.
pub fn parse<R: BufRead>(reader: R, cancel: &dyn Fn() -> bool) -> Result<Import, String> {
    let mut xml = Xml {
        reader: Reader::from_reader(reader.take(MAX_BYTES + 1)),
        buf: Vec::new(),
        nodes: 0,
        cancel,
    };
    match xml.next()? {
        Event::Start(e) if e.name().as_ref() == b"plist" => {}
        _ => return Err("Expected an exported XML plist, not a binary iTunes library".into()),
    }
    match xml.next()? {
        Event::Start(e) if e.name().as_ref() == b"dict" => {}
        _ => return Err("Expected root plist dictionary".into()),
    }
    let mut result = Import::default();
    let mut keys = HashSet::new();
    let mut ids = HashSet::new();
    let mut playlist_ids = HashSet::new();
    let mut links = 0;
    let mut seen_tracks = 0;
    loop {
        if cancel() {
            return Err("iTunes import cancelled".into());
        }
        match xml.next()? {
            Event::End(e) if e.name().as_ref() == b"dict" => break,
            Event::Start(e) if e.name().as_ref() == b"key" => {
                let key = xml.text(b"key")?;
                if keys.len() >= 256 || !keys.insert(key.clone()) {
                    return Err("Duplicate root plist key".into());
                }
                let event = xml.next()?;
                match (key.as_str(), event) {
                    ("Tracks", Event::Empty(e)) if e.name().as_ref() == b"dict" => {}
                    ("Tracks", Event::Start(e)) if e.name().as_ref() == b"dict" => loop {
                        if cancel() {
                            return Err("iTunes import cancelled".into());
                        }
                        match xml.next()? {
                            Event::End(e) if e.name().as_ref() == b"dict" => break,
                            Event::Start(e) if e.name().as_ref() == b"key" => {
                                let key = xml.text(b"key")?;
                                if !ids.insert(key.clone()) {
                                    return Err("Duplicate Track dictionary key".into());
                                }
                                let event = xml.next()?;
                                let value = xml.value(event, 0)?;
                                seen_tracks += 1;
                                if seen_tracks > MAX_TRACKS {
                                    return Err("iTunes track limit exceeded".into());
                                }
                                match track(&key, value) {
                                    Ok(t) => {
                                        result.tracks.push(t);
                                    }
                                    Err(e) => result.warning(format!("Track {key}: {e}")),
                                }
                            }
                            _ => return Err("Malformed Tracks dictionary".into()),
                        }
                    },
                    ("Playlists", Event::Empty(e)) if e.name().as_ref() == b"array" => {}
                    ("Playlists", Event::Start(e)) if e.name().as_ref() == b"array" => loop {
                        if cancel() {
                            return Err("iTunes import cancelled".into());
                        }
                        let event = xml.next()?;
                        if matches!(&event,Event::End(e)if e.name().as_ref()==b"array") {
                            break;
                        }
                        let value = xml.value(event, 0)?;
                        if result.playlists.len() >= 4096 {
                            return Err("iTunes playlist limit exceeded".into());
                        }
                        match playlist(value) {
                            Ok(Some(p)) => {
                                links += p.tracks.len();
                                if links > MAX_LINKS || !playlist_ids.insert(p.id.clone()) {
                                    return Err("Duplicate/oversized playlists".into());
                                }
                                result.playlists.push(p);
                            }
                            Ok(None) => {}
                            Err(e) => result.warning(e),
                        }
                    },
                    ("Tracks" | "Playlists", _) => {
                        return Err("Invalid Tracks/Playlists container".into())
                    }
                    ("Music Folder", event) => {
                        let value = xml.value(event, 0)?;
                        result.music_folder = value.text().map(str::to_owned);
                    }
                    (_, event) => {
                        xml.value(event, 0)?;
                    }
                }
            }
            _ => return Err("Malformed root plist dictionary".into()),
        }
    }
    xml.end(b"plist")?;
    if !matches!(xml.next()?, Event::Eof) || xml.reader.get_ref().limit() == 0 {
        return Err("Trailing data or XML byte limit exceeded".into());
    }
    if !keys.contains("Tracks") {
        return Err("No Tracks dictionary in XML export".into());
    }
    if cancel() {
        return Err("iTunes import cancelled".into());
    }
    Ok(result)
}
