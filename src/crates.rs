//! Hierarchical named track collections. No filesystem or audio operations.
use std::collections::HashSet;
pub const MAX_CRATES: usize = 1024;
pub const MAX_MEMBERSHIPS: usize = 1_000_000;
pub const MAX_CRATE_DEPTH: usize = 16;
#[derive(Clone, Debug, Default)]
pub struct CrateCatalog {
    pub last_id: i32,
    pub entries: Vec<Crate>,
}
#[derive(Clone, Debug)]
pub struct Crate {
    pub id: i32,
    pub name: String,
    pub parent_id: Option<i32>,
    pub smart_query: Option<String>,
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
fn smart_query(value: &str) -> Result<String, String> {
    let value = value.trim().to_lowercase();
    if value.is_empty() || value.chars().count() > 512 || value.chars().any(char::is_control) {
        return Err("Smart crate query must contain 1–512 printable characters".into());
    }
    Ok(value)
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
                || entry.smart_query.as_ref().is_some_and(|query| {
                    smart_query(query).as_ref() != Ok(query) || !entry.tracks.is_empty()
                })
            {
                return Err("Invalid crate identity, name or membership".into());
            }
            count += entry.tracks.len();
        }
        for entry in &self.entries {
            let mut parent_id = entry.parent_id;
            let mut depth = 1;
            while let Some(id) = parent_id {
                if id == entry.id || depth >= MAX_CRATE_DEPTH {
                    return Err("Invalid crate hierarchy or depth limit exceeded".into());
                }
                let parent = self
                    .entries
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .ok_or("Crate parent no longer exists")?;
                if parent.id >= entry.id {
                    return Err("Crate parent must precede its child".into());
                }
                parent_id = parent.parent_id;
                depth += 1;
            }
        }
        if count > MAX_MEMBERSHIPS {
            return Err("Crate membership limit exceeded".into());
        }
        Ok(())
    }
    pub fn ordered(&self) -> Vec<&Crate> {
        self.ordered_with_depth()
            .into_iter()
            .map(|(entry, _)| entry)
            .collect()
    }
    pub fn ordered_with_depth(&self) -> Vec<(&Crate, usize)> {
        fn append<'a>(
            catalog: &'a CrateCatalog,
            parent_id: Option<i32>,
            depth: usize,
            entries: &mut Vec<(&'a Crate, usize)>,
        ) {
            let mut children: Vec<_> = catalog
                .entries
                .iter()
                .filter(|entry| entry.parent_id == parent_id)
                .collect();
            children.sort_by_cached_key(|entry| (name_key(&entry.name), entry.id));
            for entry in children {
                entries.push((entry, depth));
                append(catalog, Some(entry.id), depth + 1, entries);
            }
        }
        let mut entries = Vec::with_capacity(self.entries.len());
        append(self, None, 0, &mut entries);
        entries
    }
    pub fn hierarchy_name(&self, id: i32) -> Result<String, String> {
        let mut names = Vec::new();
        let mut current = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("Crate no longer exists")?;
        loop {
            names.push(current.name.as_str());
            let Some(parent_id) = current.parent_id else {
                break;
            };
            current = self
                .entries
                .iter()
                .find(|entry| entry.id == parent_id)
                .ok_or("Crate parent no longer exists")?;
        }
        names.reverse();
        Ok(names.join(" / "))
    }
    pub fn create(&mut self, value: &str) -> Result<i32, String> {
        self.create_under(None, value)
    }
    pub fn create_under(&mut self, parent_id: Option<i32>, value: &str) -> Result<i32, String> {
        self.create_entry(parent_id, value, None)
    }
    pub fn create_smart(&mut self, value: &str, query: &str) -> Result<i32, String> {
        self.create_smart_under(None, value, query)
    }
    pub fn create_smart_under(
        &mut self,
        parent_id: Option<i32>,
        value: &str,
        query: &str,
    ) -> Result<i32, String> {
        self.create_entry(parent_id, value, Some(smart_query(query)?))
    }
    fn create_entry(
        &mut self,
        parent_id: Option<i32>,
        value: &str,
        smart_query: Option<String>,
    ) -> Result<i32, String> {
        let value = name(value)?;
        if self.entries.len() == MAX_CRATES {
            return Err("Crate limit reached".into());
        }
        let depth = match parent_id {
            Some(id) => {
                let parent = self
                    .entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .ok_or("Crate parent no longer exists")?;
                self.depth(parent.id) + 1
            }
            None => 0,
        };
        if depth > MAX_CRATE_DEPTH {
            return Err("Crate hierarchy depth limit reached".into());
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
            parent_id,
            smart_query,
            tracks: HashSet::new(),
        });
        Ok(id)
    }
    fn depth(&self, id: i32) -> usize {
        let mut depth = 0;
        let mut current = self.entries.iter().find(|entry| entry.id == id);
        while let Some(entry) = current {
            depth += 1;
            current = entry
                .parent_id
                .and_then(|parent| self.entries.iter().find(|entry| entry.id == parent));
        }
        depth
    }
    pub fn rename(&mut self, id: i32, value: &str) -> Result<(), String> {
        let query = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("Crate no longer exists")?
            .smart_query
            .clone();
        self.edit(id, value, query.as_deref())
    }
    pub fn edit(&mut self, id: i32, value: &str, query: Option<&str>) -> Result<(), String> {
        let value = name(value)?;
        let query = query.map(smart_query).transpose()?;
        if self
            .entries
            .iter()
            .any(|c| c.id != id && name_key(&c.name) == name_key(&value))
        {
            return Err("A crate with this name already exists".into());
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("Crate no longer exists")?;
        if query.is_some() && !entry.tracks.is_empty() {
            return Err("A smart crate cannot contain manual track memberships".into());
        }
        entry.name = value;
        entry.smart_query = query;
        Ok(())
    }
    pub fn delete(&mut self, id: i32) -> Result<(), String> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("Crate no longer exists")?;
        let parent_id = entry.parent_id;
        for child in self
            .entries
            .iter_mut()
            .filter(|entry| entry.parent_id == Some(id))
        {
            child.parent_id = parent_id;
        }
        self.entries.retain(|entry| entry.id != id);
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
        if entry.smart_query.is_some() {
            return Err("Smart crates use a saved query, not manual memberships".into());
        }
        let added = tracks.difference(&entry.tracks).count();
        if count + added > MAX_MEMBERSHIPS {
            return Err("Crate membership limit reached".into());
        }
        entry.tracks.extend(tracks);
        Ok(added)
    }
    pub fn remove(&mut self, crate_id: i32, track_id: i32) -> Result<bool, String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|c| c.id == crate_id)
            .ok_or("Crate no longer exists")?;
        if entry.smart_query.is_some() {
            return Err("Smart crates use a saved query, not manual memberships".into());
        }
        Ok(entry.tracks.remove(&track_id))
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

    #[test]
    fn nested_crates_validate_order_and_promote_children_on_delete() {
        let mut catalog = CrateCatalog::default();
        let parent = catalog.create("Parent").unwrap();
        let child_b = catalog.create_under(Some(parent), "B").unwrap();
        let child_a = catalog.create_under(Some(parent), "A").unwrap();
        let grandchild = catalog.create_under(Some(child_a), "Grandchild").unwrap();
        catalog.add(child_b, 42).unwrap();
        assert_eq!(
            catalog
                .ordered_with_depth()
                .iter()
                .map(|(entry, depth)| (entry.name.as_str(), *depth))
                .collect::<Vec<_>>(),
            [("Parent", 0), ("A", 1), ("Grandchild", 2), ("B", 1)]
        );
        assert!(catalog.validate(&HashSet::from([42])).is_ok());

        catalog.delete(parent).unwrap();
        assert_eq!(
            catalog
                .entries
                .iter()
                .find(|entry| entry.id == child_a)
                .unwrap()
                .parent_id,
            None
        );
        assert_eq!(
            catalog
                .entries
                .iter()
                .find(|entry| entry.id == child_b)
                .unwrap()
                .parent_id,
            None
        );
        assert_eq!(
            catalog
                .entries
                .iter()
                .find(|entry| entry.id == grandchild)
                .unwrap()
                .parent_id,
            Some(child_a)
        );
        assert_eq!(
            catalog
                .entries
                .iter()
                .find(|entry| entry.id == child_b)
                .unwrap()
                .tracks,
            HashSet::from([42])
        );
        assert!(catalog.validate(&HashSet::from([42])).is_ok());
    }

    #[test]
    fn nested_crates_reject_missing_parents_and_depth_overflow() {
        let mut catalog = CrateCatalog::default();
        assert!(catalog.create_under(Some(77), "Orphan").is_err());
        let mut parent = catalog.create("Level 0").unwrap();
        for level in 1..MAX_CRATE_DEPTH {
            parent = catalog
                .create_under(Some(parent), &format!("Level {level}"))
                .unwrap();
        }
        assert!(catalog.create_under(Some(parent), "Too deep").is_err());
        assert!(catalog.validate(&HashSet::new()).is_ok());
    }

    #[test]
    fn smart_crates_validate_queries_and_reject_manual_memberships() {
        let mut catalog = CrateCatalog::default();
        let id = catalog.create_smart("  Artists  ", "  MÉTAL ").unwrap();
        assert_eq!(catalog.entries[0].smart_query.as_deref(), Some("métal"));
        assert!(catalog.add(id, 7).is_err());
        assert!(catalog.remove(id, 7).is_err());
        for query in ["", "  ", "bad\nquery"] {
            assert!(catalog.create_smart("Invalid", query).is_err());
        }
        assert!(catalog.create_smart("Invalid", &"x".repeat(513)).is_err());
        let rejected = format!("\u{0130}{}", "x".repeat(511));
        assert!(catalog.create_smart("Invalid", &rejected).is_err());
        let accepted = format!("\u{0130}{}", "x".repeat(510));
        let boundary = catalog.create_smart("Boundary", &accepted).unwrap();
        assert_eq!(
            catalog
                .entries
                .iter()
                .find(|entry| entry.id == boundary)
                .unwrap()
                .smart_query
                .as_ref()
                .unwrap()
                .chars()
                .count(),
            512
        );
        assert!(catalog.validate(&HashSet::new()).is_ok());
        assert!(catalog.validate(&HashSet::new()).is_ok());
        assert!(catalog.edit(id, "Renamed", Some("")).is_err());
        assert_eq!(catalog.entries[0].name, "Artists");
        assert_eq!(catalog.entries[0].smart_query.as_deref(), Some("métal"));
        catalog.edit(id, "Renamed", Some("live")).unwrap();
        assert_eq!(catalog.entries[0].smart_query.as_deref(), Some("live"));
        catalog.edit(id, "Manual", None).unwrap();
        catalog.add(id, 7).unwrap();
        assert!(catalog.edit(id, "Smart", Some("new query")).is_err());
        assert!(catalog.validate(&HashSet::from([7])).is_ok());
    }
}
