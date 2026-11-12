//! Explicit user-authored HID fields; no vendor offsets or profiles are guessed.
use crate::{HidCommand, HidReport, HidTarget, IoError, MAX_HID_REPORT_BYTES};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Profile files and per-report processing have fixed engineering bounds.
pub const MAX_HID_PROFILE_BYTES: usize = 65536;
pub const MAX_HID_BINDINGS: usize = 64;
/// Offset is relative to the report payload, excluding a nonzero report ID.
/// Bits use HID's least-significant-bit-first field order; signed uses two's complement.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HidField {
    pub report_id: u8,
    pub bit_offset: u16,
    pub bit_width: u8,
    #[serde(default)]
    pub signed: bool,
}
/// Mappings produce intent; the application's existing deck guards decide
/// whether loading/output/transport state permits applying it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HidBindingAction {
    Toggle {
        deck: u8,
        #[serde(default)]
        zero_is_pressed: bool,
    },
    Seek {
        deck: u8,
        minimum: i64,
        maximum: i64,
    },
    Pitch {
        deck: u8,
        minimum: i64,
        maximum: i64,
    },
    PitchEncoder {
        deck: u8,
        percent_per_step: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HidBinding {
    pub field: HidField,
    pub action: HidBindingAction,
    /// Optional same-report activation, e.g. a touchpad's tip/contact bit.
    #[serde(default)]
    pub when: Option<HidCondition>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HidCondition {
    pub field: HidField,
    pub equals: i64,
}
impl HidField {
    fn limits(&self) -> Result<(i64, i64), IoError> {
        if !(1..=32).contains(&self.bit_width)
            || usize::from(self.bit_offset) + usize::from(self.bit_width) > MAX_HID_REPORT_BYTES * 8
        {
            return Err(invalid("HID field outside supported report"));
        }
        Ok(if self.signed {
            (
                -(1i64 << (self.bit_width - 1)),
                (1i64 << (self.bit_width - 1)) - 1,
            )
        } else {
            (0, (1i64 << self.bit_width) - 1)
        })
    }
}
/// Exact collection metadata must match the explicitly selected native path.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HidProfile {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub bindings: Vec<HidBinding>,
}
impl HidProfile {
    /// Parse at most 64KiB; reject unknown properties, malformed or unsafe maps.
    pub fn parse(bytes: &[u8]) -> Result<Self, IoError> {
        if bytes.len() > MAX_HID_PROFILE_BYTES {
            return Err(invalid("HID profile exceeds 64KiB"));
        }
        let profile: Self = serde_json::from_slice(bytes)
            .map_err(|error| IoError::Backend(format!("Invalid HID profile: {error}")))?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn matches(&self, target: &HidTarget) -> bool {
        (
            self.vendor_id,
            self.product_id,
            self.interface,
            self.usage_page,
            self.usage,
        ) == (
            target.vendor_id,
            target.product_id,
            target.interface,
            target.usage_page,
            target.usage,
        )
    }
    /// Check every configured ID/field against the newly opened descriptor
    /// before dispatching any report, including IDs not yet received.
    pub fn validate_layout(&self, layout: &crate::ReportLayout) -> Result<(), IoError> {
        self.validate()?;
        if layout.contains_system_input() {
            return Err(IoError::ProtectedInput);
        }
        for binding in &self.bindings {
            for field in std::iter::once(&binding.field)
                .chain(binding.when.iter().map(|condition| &condition.field))
            {
                let length = layout
                    .input_reports()
                    .get(&field.report_id)
                    .ok_or(invalid("mapped HID input ID is not declared"))?;
                if usize::from(field.bit_offset) + usize::from(field.bit_width) > length * 8 {
                    return Err(invalid("mapped HID field exceeds declared input"));
                }
            }
        }
        Ok(())
    }
    /// Validate policy and field bounds before opening or receiving anything.
    pub fn validate(&self) -> Result<(), IoError> {
        if self.name.trim().is_empty()
            || self.name.len() > 128
            || self.bindings.is_empty()
            || self.bindings.len() > MAX_HID_BINDINGS
        {
            return Err(invalid("invalid HID profile name/binding count"));
        }
        HidCommand::Open {
            target: HidTarget {
                path: b"profile-validation".to_vec(),
                vendor_id: self.vendor_id,
                product_id: self.product_id,
                interface: self.interface,
                usage_page: self.usage_page,
                usage: self.usage,
            },
        }
        .validate()?;
        for binding in &self.bindings {
            let field = &binding.field;
            let (low, high) = field.limits()?;
            if let Some(condition) = &binding.when {
                let (low, high) = condition.field.limits()?;
                if condition.field.report_id != field.report_id
                    || !(low..=high).contains(&condition.equals)
                {
                    return Err(invalid("invalid same-report HID activation condition"));
                }
            }
            let deck = match binding.action {
                HidBindingAction::Toggle { deck, .. } => deck,
                HidBindingAction::Seek {
                    deck,
                    minimum,
                    maximum,
                }
                | HidBindingAction::Pitch {
                    deck,
                    minimum,
                    maximum,
                } => {
                    if minimum >= maximum || minimum < low || maximum > high {
                        return Err(invalid("HID axis calibration outside field range"));
                    }
                    deck
                }
                HidBindingAction::PitchEncoder {
                    deck,
                    percent_per_step,
                } => {
                    if !percent_per_step.is_finite() || !(0.01..=10.0).contains(&percent_per_step) {
                        return Err(invalid("invalid HID encoder sensitivity"));
                    }
                    deck
                }
            };
            if deck > 1 {
                return Err(invalid("HID mapping supports decks A/B only"));
            }
        }
        Ok(())
    }
}
fn invalid(reason: &'static str) -> IoError {
    IoError::InvalidParameter(reason)
}
fn field_value(field: &HidField, report: &HidReport) -> Result<i64, IoError> {
    let prefix = usize::from(report.report_id != 0);
    if report.logical_length > report.data.len()
        || report.logical_length < prefix
        || (prefix != 0 && report.data.first() != Some(&report.report_id))
    {
        return Err(IoError::InvalidHidReport);
    }
    let payload = &report.data[prefix..report.logical_length];
    let end = usize::from(field.bit_offset) + usize::from(field.bit_width);
    if end > payload.len() * 8 {
        return Err(invalid("HID mapping field exceeds actual report"));
    }
    let mut value = 0u32;
    for index in 0..usize::from(field.bit_width) {
        let bit = usize::from(field.bit_offset) + index;
        value |= u32::from((payload[bit / 8] >> (bit % 8)) & 1) << index;
    }
    let value = i64::from(value);
    Ok(
        if field.signed && value & (1i64 << (field.bit_width - 1)) != 0 {
            value - (1i64 << field.bit_width)
        } else {
            value
        },
    )
}
/// Copy-only intents; decoding a report allocates no action vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HidMappedAction {
    Toggle { deck: u8 },
    Seek { deck: u8, unit: f64 },
    Pitch { deck: u8, unit: f64 },
    PitchDelta { deck: u8, percent: f64 },
}
pub struct HidActions {
    values: [Option<HidMappedAction>; MAX_HID_BINDINGS],
    len: usize,
}
impl HidActions {
    pub fn iter(&self) -> impl Iterator<Item = HidMappedAction> + '_ {
        self.values[..self.len].iter().flatten().copied()
    }
    fn push(&mut self, action: HidMappedAction) {
        self.values[self.len] = Some(action);
        self.len += 1;
    }
}
/// Button latches await a real release initially and after every loss/epoch.
/// Duplicate held reports cannot retrigger. No held audio controls are owned here.
pub struct HidMapper {
    profile: Arc<HidProfile>,
    ready: [bool; MAX_HID_BINDINGS],
    pressed: [bool; MAX_HID_BINDINGS],
    axes: [Option<i64>; MAX_HID_BINDINGS],
}
impl HidMapper {
    pub fn new(profile: HidProfile) -> Result<Self, IoError> {
        Self::with_profile(Arc::new(profile))
    }
    /// Share one validated immutable profile between settings and active input.
    pub fn with_profile(profile: Arc<HidProfile>) -> Result<Self, IoError> {
        profile.validate()?;
        Ok(Self {
            profile,
            ready: [false; MAX_HID_BINDINGS],
            pressed: [false; MAX_HID_BINDINGS],
            axes: [None; MAX_HID_BINDINGS],
        })
    }
    pub fn profile(&self) -> &HidProfile {
        &self.profile
    }
    pub fn reset(&mut self) {
        self.ready.fill(false);
        self.pressed.fill(false);
        self.axes.fill(None);
    }
    /// Transactional: malformed fields emit no partial intents or latch changes.
    pub fn decode(&mut self, report: &HidReport) -> Result<HidActions, IoError> {
        let mut result = HidActions {
            values: [None; MAX_HID_BINDINGS],
            len: 0,
        };
        let mut ready = self.ready;
        let mut pressed = self.pressed;
        let mut axes = self.axes;
        for (index, binding) in self.profile.bindings.iter().enumerate() {
            if binding.field.report_id != report.report_id {
                continue;
            }
            if let Some(condition) = &binding.when {
                if field_value(&condition.field, report)? != condition.equals {
                    ready[index] = false;
                    pressed[index] = false;
                    axes[index] = None;
                    continue;
                }
            }
            let raw = field_value(&binding.field, report)?;
            let action = match binding.action {
                HidBindingAction::Toggle {
                    deck,
                    zero_is_pressed,
                } => {
                    let down = if zero_is_pressed { raw == 0 } else { raw != 0 };
                    let trigger = ready[index] && !pressed[index] && down;
                    if !down {
                        ready[index] = true;
                    }
                    pressed[index] = down;
                    trigger.then_some(HidMappedAction::Toggle { deck })
                }
                HidBindingAction::Seek {
                    deck,
                    minimum,
                    maximum,
                }
                | HidBindingAction::Pitch {
                    deck,
                    minimum,
                    maximum,
                } => {
                    let value = raw.clamp(minimum, maximum);
                    let changed = axes[index] != Some(value);
                    axes[index] = Some(value);
                    let unit = (value - minimum) as f64 / (maximum - minimum) as f64;
                    changed.then_some(if matches!(binding.action, HidBindingAction::Seek { .. }) {
                        HidMappedAction::Seek { deck, unit }
                    } else {
                        HidMappedAction::Pitch { deck, unit }
                    })
                }
                HidBindingAction::PitchEncoder {
                    deck,
                    percent_per_step,
                } => (raw != 0).then_some(HidMappedAction::PitchDelta {
                    deck,
                    percent: raw as f64 * percent_per_step,
                }),
            };
            if let Some(action) = action {
                result.push(action);
            }
        }
        self.ready = ready;
        self.pressed = pressed;
        self.axes = axes;
        Ok(result)
    }
}
