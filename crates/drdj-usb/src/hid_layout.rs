//! Bounded HID report-size parsing. This does not invent vendor field mappings.
use crate::{IoError, MAX_HID_REPORT_BYTES};
use std::collections::BTreeMap;

/// Payload byte lengths by report ID (without the API's report-ID prefix).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportLayout {
    pub(crate) input: BTreeMap<u8, usize>,
    pub(crate) output: BTreeMap<u8, usize>,
    pub(crate) feature: BTreeMap<u8, usize>,
    pub(crate) numbered: bool,
    /// Conservative protection: keyboard/mouse/system-control usages were seen.
    pub(crate) system_input: bool,
}
#[derive(Clone, Copy, Default)]
struct Global {
    page: u32,
    size: u32,
    count: u32,
    id: u8,
}

pub(crate) fn system_usage(page: u32, usage: u32) -> bool {
    page == 7 || (page == 1 && matches!(usage, 2 | 6 | 7 | 0x80..=0x8f))
}
fn malformed(reason: &'static str) -> IoError {
    IoError::HidDescriptor(reason)
}
fn append(table: &mut BTreeMap<u8, u32>, global: Global) -> Result<(), IoError> {
    let bits = global
        .size
        .checked_mul(global.count)
        .filter(|bits| *bits > 0)
        .ok_or_else(|| malformed("invalid report field size/count"))?;
    let entry = table.entry(global.id).or_default();
    *entry = entry
        .checked_add(bits)
        .filter(|bits| *bits <= (MAX_HID_REPORT_BYTES * 8) as u32)
        .ok_or_else(|| malformed("report exceeds size limit"))?;
    Ok(())
}
impl ReportLayout {
    /// Parse standard short HID items. Reject truncated, excessive, unbalanced,
    /// reserved and unsupported long-item layouts rather than guessing a size.
    pub fn parse(descriptor: &[u8]) -> Result<Self, IoError> {
        if descriptor.is_empty() || descriptor.len() > hidapi::MAX_REPORT_DESCRIPTOR_SIZE {
            return Err(malformed("empty or excessive descriptor"));
        }
        let mut result = Self::default();
        let mut input = BTreeMap::new();
        let mut output = BTreeMap::new();
        let mut feature = BTreeMap::new();
        let mut global = Global::default();
        let mut stack = Vec::new();
        let mut depth = 0u8;
        let mut usage_minimum = None;
        let mut offset = 0;
        while offset < descriptor.len() {
            let header = descriptor[offset];
            offset += 1;
            if header == 0xfe {
                return Err(malformed("long HID items are not supported"));
            }
            let length = match header & 3 {
                3 => 4,
                value => usize::from(value),
            };
            if descriptor.len() - offset < length {
                return Err(malformed("truncated HID item"));
            }
            let data = &descriptor[offset..offset + length];
            offset += length;
            let value = data.iter().enumerate().fold(0u32, |value, (index, byte)| {
                value | (u32::from(*byte) << (index * 8))
            });
            let tag = header >> 4;
            if (header >> 2) & 3 == 0 && usage_minimum.is_some() {
                return Err(malformed("usage minimum without maximum"));
            }
            match ((header >> 2) & 3, tag) {
                (1, 0) => {
                    if value > 0xffff {
                        return Err(malformed("usage page exceeds supported range"));
                    }
                    global.page = value;
                    result.system_input |= value == 7;
                }
                (1, 7) => global.size = value,
                (1, 8) => {
                    global.id = u8::try_from(value)
                        .ok()
                        .filter(|id| *id != 0)
                        .ok_or_else(|| malformed("invalid report ID"))?;
                    result.numbered = true;
                }
                (1, 9) => global.count = value,
                (1, 10) => {
                    if stack.len() >= 16 {
                        return Err(malformed("global stack too deep"));
                    }
                    stack.push(global);
                }
                (1, 11) => {
                    global = stack
                        .pop()
                        .ok_or_else(|| malformed("global stack underflow"))?
                }
                (2, 0) => {
                    let page = if value > 0xffff {
                        value >> 16
                    } else {
                        global.page
                    };
                    result.system_input |= system_usage(page, value & 0xffff);
                }
                (2, 1) => {
                    usage_minimum = Some((
                        if value > 0xffff {
                            value >> 16
                        } else {
                            global.page
                        },
                        value & 0xffff,
                    ))
                }
                (2, 2) => {
                    let (page, start) = usage_minimum
                        .take()
                        .ok_or_else(|| malformed("usage maximum without minimum"))?;
                    let end_page = if value > 0xffff {
                        value >> 16
                    } else {
                        global.page
                    };
                    let end = value & 0xffff;
                    if page != end_page || start > end {
                        return Err(malformed("invalid usage range"));
                    }
                    result.system_input |= page == 7
                        || (page == 1
                            && [(2, 2), (6, 7), (0x80, 0x8f)]
                                .iter()
                                .any(|&(low, high)| start <= high && end >= low));
                }
                (0, 10) => {
                    depth = depth
                        .checked_add(1)
                        .filter(|depth| *depth <= 32)
                        .ok_or_else(|| malformed("collections too deep"))?;
                }
                (0, 12) => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| malformed("collection underflow"))?
                }
                (0, 8 | 9 | 11) => {
                    if depth == 0 || data.is_empty() {
                        return Err(malformed(
                            "report main item outside collection or without flags",
                        ));
                    }
                    match tag {
                        8 => append(&mut input, global)?,
                        9 => append(&mut output, global)?,
                        _ => append(&mut feature, global)?,
                    }
                }
                (3, _) => return Err(malformed("reserved HID item type")),
                (0, _) => return Err(malformed("unsupported HID main item")),
                _ => {}
            }
        }
        if depth != 0 || !stack.is_empty() || usage_minimum.is_some() {
            return Err(malformed("unbalanced HID collections/global stack"));
        }
        if input.is_empty() && output.is_empty() && feature.is_empty() {
            return Err(malformed("descriptor has no reports"));
        }
        if result.numbered
            && [&input, &output, &feature]
                .iter()
                .any(|map| map.contains_key(&0))
        {
            return Err(malformed("mixed numbered and unnumbered reports"));
        }
        let convert =
            |source: BTreeMap<u8, u32>, api_prefix: bool| -> Result<BTreeMap<u8, usize>, IoError> {
                source
                    .into_iter()
                    .map(|(id, bits)| {
                        let bytes = bits.div_ceil(8) as usize;
                        if bytes + usize::from(api_prefix || id != 0) > MAX_HID_REPORT_BYTES {
                            return Err(malformed("report API frame exceeds size limit"));
                        }
                        Ok((id, bytes))
                    })
                    .collect()
            };
        result.input = convert(input, false)?;
        result.output = convert(output, true)?;
        result.feature = convert(feature, true)?;
        Ok(result)
    }
    /// Maximum raw Input frame, including an ID only for numbered reports.
    pub fn input_capacity(&self) -> Option<usize> {
        self.input
            .iter()
            .map(|(id, len)| len + usize::from(*id != 0))
            .max()
    }
    /// Declared input payload lengths, indexed by report ID.
    pub fn input_reports(&self) -> &BTreeMap<u8, usize> {
        &self.input
    }
    /// Declared output payload lengths; the API ID byte is additional.
    pub fn output_reports(&self) -> &BTreeMap<u8, usize> {
        &self.output
    }
    /// Declared feature payload lengths; the API ID byte is additional.
    pub fn feature_reports(&self) -> &BTreeMap<u8, usize> {
        &self.feature
    }
    /// Whether this descriptor uses nonzero report IDs.
    pub fn is_numbered(&self) -> bool {
        self.numbered
    }
    /// Whether protected system input usages were encountered.
    pub fn contains_system_input(&self) -> bool {
        self.system_input
    }
    /// Validate a raw HIDAPI Input packet, preserving any OS padding in `data`.
    pub fn input_identity(&self, data: &[u8]) -> Result<(u8, usize), IoError> {
        let id = if self.numbered {
            *data.first().ok_or(IoError::InvalidHidReport)?
        } else {
            0
        };
        let logical =
            self.input.get(&id).ok_or(IoError::InvalidHidReport)? + usize::from(self.numbered);
        if data.len() < logical || data.len() > self.input_capacity().unwrap_or_default() {
            return Err(IoError::InvalidHidReport);
        }
        Ok((id, logical))
    }
    pub(crate) fn output_valid(&self, data: &[u8], feature: bool) -> bool {
        let Some(id) = data.first() else {
            return false;
        };
        let table = if feature { &self.feature } else { &self.output };
        table.get(id).is_some_and(|bytes| data.len() == bytes + 1)
    }
}
