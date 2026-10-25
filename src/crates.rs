//! Flat named track collections. No filesystem or audio operations.
use std::collections::HashSet;
pub const MAX_CRATES: usize = 1024;
pub const MAX_MEMBERSHIPS: usize = 1_000_000;
#[derive(Clone, Debug, Default)]
pub struct CrateCatalog {
    pub last_id: i32,
    pub entries: Vec<Crate>,
}
#[derive(Clone, Debug)]
pub struct Crate {
    pub id: i32,
    pub name: String,
    pub tracks: HashSet<i32>,
}
pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}
fn name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 128 || value.chars().any(char::is_control) {
        return Err("Crate name must contain 1–128 printable characters".into());
    }
    if name_key(value) == "all tracks" {
        return Err("All tracks is a reserved name".into());
    }
    Ok(value.to_owned())
}
impl CrateCatalog {
    pub fn validate(&self, track_ids: &HashSet<i32>) -> Result<(), String> {
        if self.entries.len() > MAX_CRATES || self.last_id < 0 || self.last_id == i32::MAX {
            return Err("Invalid crate count or ID sequence".into());
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut count = 0;
        for entry in &self.entries {
            if entry.id <= 0
                || entry.id > self.last_id
                || !ids.insert(entry.id)
                || name(&entry.name)? != entry.name
                || !names.insert(name_key(&entry.name))
                || entry.tracks.iter().any(|id| !track_ids.contains(id))
            {
                return Err("Invalid crate identity, name or membership".into());
            }
            count += entry.tracks.len();
        }
        if count > MAX_MEMBERSHIPS {
            return Err("Crate membership limit exceeded".into());
        }
        Ok(())
    }
    pub fn ordered(&self) -> Vec<&Crate> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_cached_key(|c| (name_key(&c.name), c.id));
        entries
    }
    pub fn create(&mut self, value: &str) -> Result<i32, String> {
        let value = name(value)?;
        if self.entries.len() == MAX_CRATES {
            return Err("Crate limit reached".into());
        }
        if self
            .entries
            .iter()
            .any(|c| name_key(&c.name) == name_key(&value))
        {
            return Err("A crate with this name already exists".into());
        }
        let id = self
            .last_id
            .checked_add(1)
            .filter(|id| *id < i32::MAX)
            .ok_or("Crate IDs exhausted")?;
        self.last_id = id;
        self.entries.push(Crate {
            id,
            name: value,
            tracks: HashSet::new(),
        });
        Ok(id)
    }
    pub fn rename(&mut self, id: i32, value: &str) -> Result<(), String> {
        let value = name(value)?;
        if self
            .entries
            .iter()
            .any(|c| c.id != id && name_key(&c.name) == name_key(&value))
        {
            return Err("A crate with this name already exists".into());
        }
        self.entries
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or("Crate no longer exists")?
            .name = value;
        Ok(())
    }
    pub fn delete(&mut self, id: i32) -> Result<(), String> {
        let index = self
            .entries
            .iter()
            .position(|c| c.id == id)
            .ok_or("Crate no longer exists")?;
        self.entries.remove(index);
        Ok(())
    }
    pub fn add(&mut self, crate_id: i32, track_id: i32) -> Result<bool, String> {
        Ok(self.add_many(crate_id, &HashSet::from([track_id]))? > 0)
    }
    pub fn add_many(&mut self, crate_id: i32, tracks: &HashSet<i32>) -> Result<usize, String> {
        let count: usize = self.entries.iter().map(|c| c.tracks.len()).sum();
        let entry = self
            .entries
            .iter_mut()
            .find(|c| c.id == crate_id)
            .ok_or("Crate no longer exists")?;
        let added = tracks.difference(&entry.tracks).count();
        if count + added > MAX_MEMBERSHIPS {
            return Err("Crate membership limit reached".into());
        }
        entry.tracks.extend(tracks);
        Ok(added)
    }
    pub fn remove(&mut self, crate_id: i32, track_id: i32) -> Result<bool, String> {
        Ok(self
            .entries
            .iter_mut()
            .find(|c| c.id == crate_id)
            .ok_or("Crate no longer exists")?
            .tracks
            .remove(&track_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_names_and_keeps_ids_monotonic_after_delete_and_restore() {
        let mut catalog = CrateCatalog::default();
        for value in ["", "  ", "All tracks", "bad\nname"] {
            assert!(catalog.create(value).is_err());
        }
        assert!(catalog.create(&"X".repeat(129)).is_err());
        let first = catalog.create("  Música 日本  ").unwrap();
        assert_eq!(catalog.entries[0].name, "Música 日本");
        assert!(catalog.create("MÚSICA 日本").is_err());
        let second = catalog.create("Warmup").unwrap();
        assert!(catalog.rename(second, "música 日本").is_err());
        catalog.rename(first, "Festival").unwrap();
        assert_eq!(catalog.entries[0].id, first);
        catalog.delete(second).unwrap();
        catalog.delete(first).unwrap();
        let mut restored = catalog.clone();
        assert!(restored.create("Next").unwrap() > second);
        assert!(restored.validate(&HashSet::new()).is_ok());
    }
    #[test]
    fn memberships_are_sets_and_independent_between_crates() {
        let mut catalog = CrateCatalog::default();
        let a = catalog.create("A").unwrap();
        let b = catalog.create("B").unwrap();
        assert!(catalog.add(a, 7).unwrap());
        assert!(!catalog.add(a, 7).unwrap());
        catalog.add(b, 7).unwrap();
        catalog.add(b, 9).unwrap();
        assert!(catalog.remove(a, 7).unwrap());
        assert!(!catalog.remove(a, 7).unwrap());
        catalog.delete(a).unwrap();
        assert_eq!(catalog.entries[0].tracks, HashSet::from([7, 9]));
        assert!(catalog.validate(&HashSet::from([7, 9])).is_ok());
        assert!(catalog.validate(&HashSet::from([7])).is_err());
        assert!(catalog.add(999, 7).is_err());
    }
    #[test]
    fn untrusted_sequences_duplicate_names_and_orphan_memberships_are_rejected() {
        let mut catalog = CrateCatalog::default();
        let id = catalog.create("One").unwrap();
        catalog.add(id, 1).unwrap();
        catalog.last_id = 0;
        assert!(catalog.validate(&HashSet::from([1])).is_err());
        catalog.last_id = id;
        catalog.entries.push(catalog.entries[0].clone());
        assert!(catalog.validate(&HashSet::from([1])).is_err());
    }
}
