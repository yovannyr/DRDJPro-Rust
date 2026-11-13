//! Web navigation and immutable bundled resources; no disk or engine access.
pub const HELP_URL: &str = "http://drdjhelp.localhost/index.html";
pub const HOME_URL: &str = "https://www.beatport.com/";
pub const CSP:&str="default-src 'none'; style-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
pub fn allowed(value: &str) -> bool {
    if value.len() > 8192 || value.chars().any(char::is_control) {
        return false;
    }
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if !url.username().is_empty() || url.password().is_some() || url.port() == Some(0) {
        return false;
    }
    (url.scheme() == "https" && url.host_str().is_some())
        || (url.scheme() == "http"
            && url.host_str() == Some("drdjhelp.localhost")
            && url.port().is_none())
}
/// Runtime102+ includes the InPrivate controller options required by this host.
pub fn private_runtime(version: &str) -> bool {
    version
        .split('.')
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|major| major >= 102)
}
/// OAuth query/fragment values never enter host status/history/logs.
pub fn display_url(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return "Invalid address".into();
    };
    url.set_query(None);
    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.to_string()
}
pub fn resource(value: &str) -> (&'static str, &'static [u8], u16) {
    let Ok(url) = url::Url::parse(value) else {
        return ("text/plain", b"Not found", 404);
    };
    let local = (url.scheme() == "drdjhelp" && url.host_str() == Some("localhost"))
        || (url.scheme() == "http" && url.host_str() == Some("drdjhelp.localhost"));
    if !local || url.query().is_some() {
        return ("text/plain", b"Not found", 404);
    }
    match url.path() {
        "/" | "/index.html" => (
            "text/html; charset=utf-8",
            include_bytes!("../res/help/index.html"),
            200,
        ),
        "/controls.html" => (
            "text/html; charset=utf-8",
            include_bytes!("../res/help/controls.html"),
            200,
        ),
        "/style.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../res/help/style.css"),
            200,
        ),
        "/logo.svg" => ("image/svg+xml", include_bytes!("../res/help/logo.svg"), 200),
        _ => ("text/plain; charset=utf-8", b"Help topic not found", 404),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_https_and_our_help_origin_are_navigable() {
        for u in [
            HOME_URL,
            HELP_URL,
            "https://accounts.example.org/login?state=x",
        ] {
            assert!(allowed(u), "{u}");
        }
        for u in [
            "http://example.org",
            "file:///C:/secret",
            "javascript:alert(1)",
            "data:text/html,x",
            "https://user:pass@example.org",
            "https://x:0/",
            "http://drdjhelp.localhost.evil/",
            "https://x/\n",
            "drdjhelp://localhost/controls.html",
        ] {
            assert!(!allowed(u), "{u}");
        }
    }
    #[test]
    fn help_assets_are_explicit_offline_and_csp_is_restrictive() {
        for path in ["index.html", "controls.html", "style.css", "logo.svg"] {
            let (_, bytes, status) = resource(&format!("http://drdjhelp.localhost/{path}"));
            assert_eq!(status, 200);
            assert!(!bytes.is_empty());
        }
        for u in [
            "http://drdjhelp.localhost/../../Cargo.toml",
            "file:///C:/secret",
            "https://evil/index.html",
            "drdjhelp://localhost/index.html?x=1",
        ] {
            assert_eq!(resource(u).2, 404);
        }
        assert!(CSP.contains("default-src 'none'"));
        assert!(!CSP.contains("unsafe-inline"));
    }
    #[test]
    fn authentication_values_do_not_enter_host_display() {
        assert_eq!(
            display_url("https://x/callback?code=private&state=secret#token"),
            "https://x/callback"
        );
    }
    #[test]
    fn old_or_unrecognized_runtimes_cannot_silently_drop_private_mode() {
        for v in ["", "invalid", "100.0.1.2", "101.0.1210.39"] {
            assert!(!private_runtime(v));
        }
        assert!(private_runtime("102.0.1245.30"));
        assert!(private_runtime("140.0.1.2"));
    }
}
