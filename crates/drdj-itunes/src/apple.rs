//! Explicit, worker-only Apple catalog lookup. No subscriber audio or previews.
use crate::PreparedTrack;
use image::ImageReader;
use reqwest::{blocking::Client, header::HeaderValue};
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    time::Duration,
};
use url::Url;

const JSON_LIMIT: usize = 1024 * 1024;
const ART_LIMIT: usize = 10 * 1024 * 1024;
/// Decoded thumbnail, produced on the worker; never deserialize arbitrary sizes.
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
/// Original identity and file stamp prevent applying cached data to another file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Basis {
    pub track_id: u64,
    pub persistent_id: String,
    pub path_url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
    pub size: Option<u64>,
    pub modified_ms: Option<u64>,
}
impl Basis {
    pub fn of(t: &PreparedTrack) -> Self {
        Self {
            track_id: t.track.id,
            persistent_id: t.track.persistent_id.clone(),
            path_url: t.file.path_url.clone(),
            title: t.track.title.clone(),
            artist: t.track.artist.clone(),
            album: t.track.album.clone(),
            duration_ms: t.track.duration_ms,
            size: t.file.size,
            modified_ms: t.file.modified_ms,
        }
    }
    pub(crate) fn same_source(&self, t: &PreparedTrack) -> bool {
        Self::of(t) == *self
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
}
impl Metadata {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.song_id.is_empty()
            || self.song_id.len() > 32
            || !self.song_id.bytes().all(|b| b.is_ascii_digit())
            || [&self.title, &self.artist, &self.album]
                .iter()
                .any(|s| s.is_empty() || s.chars().count() > 512 || s.chars().any(char::is_control))
        {
            return Err("Invalid Apple song metadata".into());
        }
        Ok(())
    }
    pub fn apply(&self, t: &mut PreparedTrack) {
        t.track.title.clone_from(&self.title);
        t.track.artist.clone_from(&self.artist);
        t.track.album.clone_from(&self.album);
    }
}
pub struct Lookup {
    pub metadata: Metadata,
    /// Normalized JPEG, at most300x300.
    pub artwork: Option<Vec<u8>>,
}
/// Credential is held only in memory and deliberately has no Debug/Serialize.
pub struct AppleClient {
    transport: Http,
    token: HeaderValue,
    storefront: String,
}
impl AppleClient {
    /// Reads a signed developer JWT, never an Apple ID password/private signing key.
    pub fn from_env() -> Result<Self, String> {
        let token=std::env::var("DRDJ_APPLE_MUSIC_TOKEN").map_err(|_|"Apple metadata requires DRDJ_APPLE_MUSIC_TOKEN (MusicKit developer token); local import remains available".to_owned())?;
        let storefront =
            std::env::var("DRDJ_APPLE_MUSIC_STOREFRONT").unwrap_or_else(|_| "us".into());
        Self::new(&token, &storefront)
    }
    pub fn new(token: &str, storefront: &str) -> Result<Self, String> {
        let token = authorization(token)?;
        if storefront.len() != 2 || !storefront.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err("Apple storefront must be two lowercase letters".into());
        }
        let client = Client::builder()
            .tls_backend_native()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .https_only(true)
            .connect_timeout(Duration::from_secs(2))
            .user_agent("DRDJPro-Rust/0.1.0")
            .build()
            .map_err(|_| "Cannot initialize Apple HTTPS client".to_owned())?;
        Ok(Self {
            transport: Http(client),
            token,
            storefront: storefront.into(),
        })
    }
    pub fn lookup(
        &self,
        track: &PreparedTrack,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Lookup, String> {
        lookup(
            &self.transport,
            &self.token,
            &self.storefront,
            track,
            cancel,
        )
    }
}
fn authorization(token: &str) -> Result<HeaderValue, String> {
    if token.is_empty()
        || token.len() > 8192
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("Missing or malformed MusicKit developer token".into());
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|_| "Malformed MusicKit developer token".to_owned())?;
    header.set_sensitive(true);
    Ok(header)
}
trait Transport {
    fn get(
        &self,
        url: &Url,
        token: Option<&HeaderValue>,
        timeout: Duration,
        limit: usize,
    ) -> Result<Vec<u8>, String>;
}
struct Http(Client);
impl Transport for Http {
    fn get(
        &self,
        url: &Url,
        token: Option<&HeaderValue>,
        timeout: Duration,
        limit: usize,
    ) -> Result<Vec<u8>, String> {
        let mut request = self.0.get(url.clone()).timeout(timeout);
        if let Some(token) = token {
            request = request.header(reqwest::header::AUTHORIZATION, token.clone());
        }
        let response = request
            .send()
            .map_err(|_| "Apple request failed or timed out".to_owned())?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "Apple request returned HTTP {}",
                response.status().as_u16()
            ));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err("Apple response exceeds size limit".into());
        }
        bounded(response, limit)
    }
}
fn bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read Apple response".to_owned())?;
    if bytes.len() > limit {
        return Err("Apple response exceeds size limit".into());
    }
    Ok(bytes)
}
fn cancelled(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Apple lookup cancelled".into())
    } else {
        Ok(())
    }
}
fn normal(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
#[derive(Deserialize)]
struct Response {
    results: Results,
}
#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    songs: Songs,
}
#[derive(Default, Deserialize)]
struct Songs {
    #[serde(default)]
    data: Vec<Song>,
}
#[derive(Deserialize)]
struct Song {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    attributes: Attributes,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attributes {
    name: String,
    artist_name: String,
    album_name: String,
    duration_in_millis: Option<u64>,
    artwork: Option<Artwork>,
}
#[derive(Deserialize)]
struct Artwork {
    url: String,
}
fn artwork_url(template: &str) -> Result<Url, String> {
    if template.len() > 8192 || template.chars().any(char::is_control) {
        return Err("Invalid Apple artwork URL".into());
    }
    let value = template
        .replace("{w}", "300")
        .replace("{h}", "300")
        .replace("{f}", "jpg");
    if value.contains(['{', '}']) {
        return Err("Unknown Apple artwork placeholder".into());
    }
    let url = Url::parse(&value).map_err(|_| "Invalid Apple artwork URL".to_owned())?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "mzstatic.com" || host.ends_with(".mzstatic.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err("Untrusted Apple artwork URL".into());
    }
    Ok(url)
}
pub fn thumbnail(bytes: &[u8]) -> Result<Thumbnail, String> {
    if bytes.len() > ART_LIMIT {
        return Err("Artwork exceeds size limit".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Invalid artwork".to_owned())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
    ) {
        return Err("Unsupported artwork format".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|_| "Cannot decode bounded artwork".to_owned())?;
    let img = img
        .thumbnail(300.min(img.width()), 300.min(img.height()))
        .to_rgba8();
    Ok(Thumbnail {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}
fn normalize_art(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let thumb = thumbnail(bytes)?;
    let img = image::RgbaImage::from_raw(thumb.width, thumb.height, thumb.rgba)
        .ok_or("Invalid thumbnail")?;
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
        .encode_image(&image::DynamicImage::ImageRgba8(img).to_rgb8())
        .map_err(|_| "Cannot encode thumbnail".to_owned())?;
    Ok(encoded)
}
fn lookup(
    transport: &dyn Transport,
    token: &HeaderValue,
    storefront: &str,
    t: &PreparedTrack,
    cancel: &dyn Fn() -> bool,
) -> Result<Lookup, String> {
    cancelled(cancel)?;
    if !t.file.available || normal(&t.track.title).is_empty() || normal(&t.track.artist).is_empty()
    {
        return Err("Lookup requires an available local track with title and artist".into());
    }
    let mut url = Url::parse(&format!(
        "https://api.music.apple.com/v1/catalog/{storefront}/search"
    ))
    .map_err(|_| "Invalid storefront".to_owned())?;
    url.query_pairs_mut()
        .append_pair("types", "songs")
        .append_pair("limit", "25")
        .append_pair("term", &format!("{} {}", t.track.title, t.track.artist));
    let bytes = transport.get(&url, Some(token), Duration::from_secs(2), JSON_LIMIT)?;
    cancelled(cancel)?;
    let parsed: Response =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid Apple catalog response".to_owned())?;
    let mut matches = parsed.results.songs.data.into_iter().filter(|s| {
        s.kind == "songs"
            && normal(&s.attributes.name) == normal(&t.track.title)
            && normal(&s.attributes.artist_name) == normal(&t.track.artist)
            && match (t.track.duration_ms, s.attributes.duration_in_millis) {
                (Some(a), Some(b)) => a.abs_diff(b) <= 2000,
                _ => true,
            }
    });
    let song = matches
        .next()
        .ok_or("No unambiguous Apple song match; local metadata retained")?;
    if matches.any(|s| s.id != song.id) {
        return Err("Multiple Apple song matches; local metadata retained".into());
    }
    let metadata = Metadata {
        song_id: song.id,
        title: song.attributes.name,
        artist: song.attributes.artist_name,
        album: song.attributes.album_name,
    };
    metadata.validate()?;
    let artwork = if let Some(art) = song.attributes.artwork {
        let url = artwork_url(&art.url)?;
        let bytes = transport.get(&url, None, Duration::from_secs(10), ART_LIMIT)?;
        cancelled(cancel)?;
        Some(normalize_art(&bytes)?)
    } else {
        None
    };
    cancelled(cancel)?;
    Ok(Lookup { metadata, artwork })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake {
        replies: RefCell<Vec<Result<Vec<u8>, String>>>,
        calls: RefCell<Vec<(String, bool, Duration, usize)>>,
    }
    impl Transport for Fake {
        fn get(
            &self,
            u: &Url,
            h: Option<&HeaderValue>,
            t: Duration,
            l: usize,
        ) -> Result<Vec<u8>, String> {
            self.calls
                .borrow_mut()
                .push((u.to_string(), h.is_some(), t, l));
            self.replies.borrow_mut().remove(0)
        }
    }
    fn track() -> PreparedTrack {
        serde_json::from_value(serde_json::json!({"track":{"id":1,"persistent_id":"P1","location":"file:///C:/song.wav","title":"A Song","artist":"The Artist","album":"Local","album_artist":"","genre":"","bpm":null,"rating":null,"play_count":null,"duration_ms":120000,"date_added":null},"file":{"path_url":"file:///C:/song.wav","available":true,"size":100,"modified_ms":1}})).unwrap()
    }
    fn song(id: &str, art: bool) -> serde_json::Value {
        serde_json::json!({"id":id,"type":"songs","attributes":{"name":"A song","artistName":"The Artist","albumName":"Apple album","durationInMillis":121000,"artwork":if art{serde_json::json!({"url":"https://is1-ssl.mzstatic.com/image/{w}x{h}.jpg"})}else{serde_json::Value::Null}}})
    }
    fn fake(songs: Vec<serde_json::Value>) -> Fake {
        Fake {
            replies: RefCell::new(vec![Ok(serde_json::to_vec(
                &serde_json::json!({"results":{"songs":{"data":songs}}}),
            )
            .unwrap())]),
            calls: RefCell::new(vec![]),
        }
    }
    #[test]
    fn unique_match_and_sensitive_auth_and_unauthenticated_artwork() {
        let f = fake(vec![song("123", true)]);
        let mut png = Cursor::new(vec![]);
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        f.replies.borrow_mut().push(Ok(png.into_inner()));
        let token = authorization("a.b.c").unwrap();
        assert!(token.is_sensitive());
        let r = lookup(&f, &token, "us", &track(), &|| false).unwrap();
        assert_eq!(r.metadata.album, "Apple album");
        let thumb = thumbnail(r.artwork.as_ref().unwrap()).unwrap();
        assert_eq!((thumb.width, thumb.height), (8, 8));
        let c = f.calls.borrow();
        assert!(c[0].1);
        assert!(!c[1].1);
        assert_eq!(c[0].2, Duration::from_secs(2));
        assert_eq!(c[1].2, Duration::from_secs(10));
        assert!(c[1].0.contains("300x300"));
    }
    #[test]
    fn failure_ambiguity_cancellation_and_no_secret_errors() {
        let token = authorization("secret.value.jwt").unwrap();
        let original = track();
        for f in [
            fake(vec![]),
            fake(vec![song("1", false), song("2", false)]),
            Fake {
                replies: RefCell::new(vec![Err("Apple request returned HTTP 401".into())]),
                calls: RefCell::new(vec![]),
            },
            Fake {
                replies: RefCell::new(vec![Err("Apple request returned HTTP 429".into())]),
                calls: RefCell::new(vec![]),
            },
        ] {
            let error = lookup(&f, &token, "us", &original, &|| false)
                .err()
                .unwrap();
            assert!(!error.contains("secret"));
            assert_eq!(original.track.album, "Local");
        }
        let f = fake(vec![song("1", false)]);
        assert!(lookup(&f, &token, "us", &original, &|| true).is_err());
        assert!(f.calls.borrow().is_empty());
        assert!(authorization("").is_err());
        assert!(authorization("secret\r\nvalue").is_err());
        assert!(AppleClient::new("a.b.c", "../").is_err());
    }
    #[test]
    fn response_image_and_url_bounds() {
        assert!(bounded(Cursor::new([1; 9]), 8).is_err());
        assert_eq!(bounded(Cursor::new([1; 8]), 8).unwrap().len(), 8);
        for value in [
            "http://is1.mzstatic.com/a",
            "https://mzstatic.com.evil/a",
            "https://user@is1.mzstatic.com/a",
            "https://is1.mzstatic.com:444/a",
            "https://is1.mzstatic.com/{evil}",
        ] {
            assert!(artwork_url(value).is_err());
        }
        assert!(thumbnail(b"not an image").is_err());
        let f = fake(vec![song("../bad", false)]);
        assert!(lookup(
            &f,
            &authorization("a.b.c").unwrap(),
            "us",
            &track(),
            &|| false
        )
        .is_err());
    }
    #[test]
    fn real_http_adapter_rejects_redirect_errors_and_oversize_without_leaking_token() {
        use std::{io::Write, net::TcpListener};
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: https://evil.example/\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 10000000\r\n\r\n",
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = [0; 4096];
                let count = socket.read(&mut buf).unwrap();
                assert!(String::from_utf8_lossy(&buf[..count]).contains("Bearer secret.jwt.value"));
                socket.write_all(response.as_bytes()).unwrap();
            });
            // Only this private test transport permits loopback HTTP; public
            // AppleClient always enables HTTPS-only and fixed Apple endpoints.
            let http = Http(
                Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .retry(reqwest::retry::never())
                    .build()
                    .unwrap(),
            );
            let error = http
                .get(
                    &Url::parse(&format!("http://{address}/")).unwrap(),
                    Some(&authorization("secret.jwt.value").unwrap()),
                    Duration::from_secs(2),
                    32,
                )
                .unwrap_err();
            assert!(!error.contains("secret"));
            server.join().unwrap();
        }
    }
}
