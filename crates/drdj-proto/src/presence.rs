//! Descriptor-derived presence checks without building a second object graph.
//! Prost remains responsible for wire decoding. Singular submessage fragments
//! merge their presence; repeated submessages must each be complete.
use crate::{Contract, ForwardMessage, ProtoError, FILE_DESCRIPTOR_SET};
use prost::encoding::{decode_key, decode_varint, skip_field, DecodeContext, WireType};
use prost_reflect::{Cardinality, DescriptorPool, Kind, MessageDescriptor};
use std::{collections::HashMap, sync::OnceLock};

struct Field {
    tag: u32,
    name: String,
    wire: WireType,
    repeated: bool,
    child: Option<Box<Shape>>,
}
struct Shape {
    fields: Vec<Field>,
    required: u64,
}
impl Shape {
    fn build(descriptor: MessageDescriptor, depth: usize) -> Option<Self> {
        // Larger/recursive future schemas safely retain the reflective path.
        if descriptor.fields().len() > 64 || depth > 32 {
            return None;
        }
        let mut required = 0;
        let mut fields = Vec::new();
        for (index, field) in descriptor.fields().enumerate() {
            if field.is_packed() || field.is_group() {
                return None;
            }
            if field.cardinality() == Cardinality::Required {
                required |= 1 << index;
            }
            let (wire, child) = match field.kind() {
                Kind::Message(message) => (
                    WireType::LengthDelimited,
                    Some(Box::new(Self::build(message, depth + 1)?)),
                ),
                Kind::String | Kind::Bytes => (WireType::LengthDelimited, None),
                Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => (WireType::SixtyFourBit, None),
                Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => (WireType::ThirtyTwoBit, None),
                _ => (WireType::Varint, None),
            };
            fields.push(Field {
                tag: field.number(),
                name: field.name().into(),
                wire,
                repeated: field.is_list(),
                child,
            });
        }
        Some(Self { fields, required })
    }
}

struct State {
    seen: u64,
    counts: [u32; 64],
    children: Vec<(usize, State)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            seen: 0,
            counts: [0; 64],
            children: Vec::new(),
        }
    }
}

fn invalid(field: &str, reason: &'static str) -> ProtoError {
    ProtoError::Validation {
        field: field.into(),
        reason,
    }
}
fn prefix(error: ProtoError, field: &str) -> ProtoError {
    match error {
        ProtoError::Validation {
            field: child,
            reason,
        } => ProtoError::Validation {
            field: format!("{field}.{child}"),
            reason,
        },
        other => other,
    }
}

impl State {
    fn read(&mut self, shape: &Shape, mut bytes: &[u8]) -> Result<(), ProtoError> {
        while !bytes.is_empty() {
            let (tag, wire) = decode_key(&mut bytes)?;
            let Some(index) = shape.fields.iter().position(|field| field.tag == tag) else {
                skip_field(wire, tag, &mut bytes, DecodeContext::default())?;
                continue;
            };
            let field = &shape.fields[index];
            if wire != field.wire {
                return Err(invalid(&field.name, "incorrect wire type"));
            }
            if let Some(child_shape) = &field.child {
                let len = decode_varint(&mut bytes)
                    .map_err(|_| invalid(&field.name, "invalid embedded length"))?;
                let len =
                    usize::try_from(len).map_err(|_| invalid(&field.name, "length overflow"))?;
                if len > bytes.len() {
                    return Err(invalid(&field.name, "truncated embedded message"));
                }
                let (child_bytes, rest) = bytes.split_at(len);
                bytes = rest;
                if field.repeated {
                    let mut child = State::default();
                    child
                        .read(child_shape, child_bytes)
                        .and_then(|()| child.finish(child_shape))
                        .map_err(|error| {
                            prefix(error, &format!("{}[{}]", field.name, self.counts[index]))
                        })?;
                    self.counts[index] += 1;
                } else {
                    let position = match self.children.iter().position(|(key, _)| *key == index) {
                        Some(position) => position,
                        None => {
                            self.children.push((index, State::default()));
                            self.children.len() - 1
                        }
                    };
                    self.children[position]
                        .1
                        .read(child_shape, child_bytes)
                        .map_err(|error| prefix(error, &field.name))?;
                }
            } else {
                skip_field(wire, tag, &mut bytes, DecodeContext::default())
                    .map_err(|_| invalid(&field.name, "malformed field encoding"))?;
            }
            self.seen |= 1 << index;
        }
        Ok(())
    }
    fn finish(&self, shape: &Shape) -> Result<(), ProtoError> {
        let missing = shape.required & !self.seen;
        if missing != 0 {
            return Err(invalid(
                &shape.fields[missing.trailing_zeros() as usize].name,
                "required field missing",
            ));
        }
        for (index, state) in &self.children {
            let field = &shape.fields[*index];
            if let Some(child) = &field.child {
                state
                    .finish(child)
                    .map_err(|error| prefix(error, &field.name))?;
            }
        }
        Ok(())
    }
}

pub(crate) fn validate<T: Contract>(bytes: &[u8]) -> Result<(), ProtoError> {
    crate::check_size(bytes.len())?;
    static SHAPES: OnceLock<HashMap<String, Option<Shape>>> = OnceLock::new();
    let shapes = SHAPES.get_or_init(|| {
        let pool =
            DescriptorPool::decode(FILE_DESCRIPTOR_SET).expect("build-generated descriptors");
        pool.all_messages()
            .filter(|m| m.full_name().starts_with("drdj.v1."))
            .map(|message| (message.full_name().into(), Shape::build(message, 0)))
            .collect()
    });
    match shapes
        .get(T::NAME)
        .ok_or_else(|| ProtoError::UnknownMessage(T::NAME.into()))?
    {
        Some(shape) => {
            let mut state = State::default();
            state.read(shape, bytes)?;
            state.finish(shape)
        }
        None => crate::validation::required(&ForwardMessage::decode(T::NAME, bytes)?.0),
    }
}
