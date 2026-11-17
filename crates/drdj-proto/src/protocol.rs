use crate::{SyncRequest, SyncResponse};

/// Current sync contract version, independent of application build version.
pub const SCHEMA_VERSION: u32 = 1;

/// Process an already validated sync exchange without network or persistence.
/// Version mismatches return no updates, so incompatible data is not applied.
pub fn respond(request: SyncRequest) -> SyncResponse {
    if request.schema_version == SCHEMA_VERSION {
        SyncResponse {
            schema_version: SCHEMA_VERSION,
            updates: request.updates,
            state: request.state,
            version_error: None,
            native_updates: request.native_updates,
            playlists: request.playlists,
        }
    } else {
        SyncResponse {
            schema_version: SCHEMA_VERSION,
            updates: Vec::new(),
            state: None,
            version_error: Some(format!(
                "expected {}, received {}",
                SCHEMA_VERSION, request.schema_version
            )),
            native_updates: Vec::new(),
            playlists: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success() {
        let req = SyncRequest {
            schema_version: SCHEMA_VERSION,
            updates: vec![],
            state: None,
            ..Default::default()
        };
        let res = respond(req);
        assert_eq!(res.schema_version, SCHEMA_VERSION);
        assert!(res.version_error.is_none());
    }

    #[test]
    fn test_mismatch() {
        let req = SyncRequest {
            schema_version: 2,
            updates: vec![],
            state: None,
            ..Default::default()
        };
        let res = respond(req);
        assert!(res.version_error.is_some());
        assert!(res.updates.is_empty());
        assert!(res.state.is_none());
    }
}
