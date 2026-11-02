//! Persisted track-table presentation, independent of track identities.
use serde::{Deserialize, Serialize};

pub const COLUMN_COUNT: usize = 11;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableColumns {
    pub version: u8,
    pub order: Vec<i32>,
    pub hidden: Vec<i32>,
}

impl Default for TableColumns {
    fn default() -> Self {
        Self {
            version: 1,
            order: vec![6, 0, 3, 7, 8, 4, 5, 9, 10, 2, 1],
            hidden: vec![],
        }
    }
}

impl TableColumns {
    pub fn validate(&self) -> Result<(), String> {
        let mut sorted = self.order.clone();
        sorted.sort_unstable();
        if self.version != 1
            || sorted != (0..COLUMN_COUNT as i32).collect::<Vec<_>>()
            || self.hidden.len() >= COLUMN_COUNT
            || self.hidden.iter().any(|id| !self.order.contains(id))
            || self
                .hidden
                .iter()
                .enumerate()
                .any(|(i, id)| self.hidden[..i].contains(id))
        {
            return Err("Invalid track-table column preferences".into());
        }
        Ok(())
    }

    pub fn change(&mut self, id: i32, action: i32) -> Result<(), String> {
        let index = self
            .order
            .iter()
            .position(|&column| column == id)
            .ok_or("Unknown table column")?;
        match action {
            0 => {
                if let Some(index) = self.hidden.iter().position(|&column| column == id) {
                    self.hidden.remove(index);
                } else if self.hidden.len() + 1 < COLUMN_COUNT {
                    self.hidden.push(id);
                } else {
                    return Err("At least one table column must remain visible".into());
                }
            }
            -1 if index > 0 => self.order.swap(index, index - 1),
            1 if index + 1 < self.order.len() => self.order.swap(index, index + 1),
            -1 | 1 => return Err("Column is already at the table edge".into()),
            _ => return Err("Unknown table column action".into()),
        }
        self.validate()
    }
}

pub fn label(id: i32) -> &'static str {
    match id {
        0 => "Song",
        1 => "Type",
        2 => "Path",
        3 => "Artist",
        4 => "Album",
        5 => "Length",
        6 => "#",
        7 => "BPM",
        8 => "Key",
        9 => "Gain dB",
        10 => "Analysis",
        _ => "Unknown",
    }
}

pub fn sort_label(id: i32) -> &'static str {
    match id {
        0 => "name",
        1 => "type",
        2 => "path",
        3 => "artist",
        4 => "album",
        5 => "duration",
        6 => "id",
        7 => "BPM",
        8 => "key",
        9 => "gain",
        10 => "analysis",
        _ => "unknown",
    }
}

pub fn width(id: i32) -> f32 {
    match id {
        0 => 180.,
        2 => 240.,
        3 | 4 => 140.,
        5 | 9 => 90.,
        6 => 60.,
        7 | 8 | 1 => 70.,
        10 => 100.,
        _ => 70.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_validate_visibility_order_and_last_visible_column() {
        let mut columns = TableColumns::default();
        columns.change(7, -1).unwrap();
        assert_eq!(&columns.order[..4], &[6, 0, 7, 3]);
        columns.change(7, 0).unwrap();
        assert!(columns.hidden.contains(&7));
        columns.change(7, 0).unwrap();
        for id in 0..10 {
            columns.change(id, 0).unwrap();
        }
        assert!(columns.change(10, 0).is_err());
        assert!(columns.change(99, 0).is_err());
        columns.order[0] = columns.order[1];
        assert!(columns.validate().is_err());
    }
}
