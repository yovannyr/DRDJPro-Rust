use drdj_usb::*;
#[test]
fn documented_synthetic_profile_decodes_only_its_declared_fields() {
    let profile = HidProfile::parse(include_bytes!(
        "../../../docs/specs/28-direct-libusb/fixtures/hid-mapping.example.json"
    ))
    .unwrap();
    let mut mapper = HidMapper::new(profile).unwrap();
    let actions = mapper
        .decode(&report(&[129, 255, 0, 128]))
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(
        actions,
        vec![
            HidMappedAction::PitchDelta {
                deck: 0,
                percent: -0.1
            },
            HidMappedAction::Seek {
                deck: 1,
                unit: 32768.0 / 65535.0
            }
        ]
    );
}
fn field() -> HidField {
    HidField {
        report_id: 0,
        bit_offset: 0,
        bit_width: 8,
        signed: false,
    }
}
fn profile(action: HidBindingAction) -> HidProfile {
    HidProfile {
        name: "synthetic fixture, not a vendor profile".into(),
        vendor_id: 0x1234,
        product_id: 1,
        interface: 1,
        usage_page: 0xff00,
        usage: 1,
        bindings: vec![HidBinding {
            when: None,
            field: field(),
            action,
        }],
    }
}
fn report(data: &[u8]) -> HidReport {
    HidReport {
        report_id: 0,
        data: data.into(),
        logical_length: data.len(),
    }
}
#[test]
fn toggle_requires_release_initially_and_after_loss_and_ignores_held_duplicates() {
    let mut mapper = HidMapper::new(profile(HidBindingAction::Toggle {
        deck: 0,
        zero_is_pressed: false,
    }))
    .unwrap();
    assert_eq!(mapper.decode(&report(&[1])).unwrap().iter().count(), 0);
    assert_eq!(mapper.decode(&report(&[0])).unwrap().iter().count(), 0);
    assert_eq!(
        mapper
            .decode(&report(&[1]))
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![HidMappedAction::Toggle { deck: 0 }]
    );
    assert_eq!(mapper.decode(&report(&[1])).unwrap().iter().count(), 0);
    mapper.reset();
    assert_eq!(mapper.decode(&report(&[1])).unwrap().iter().count(), 0);
    mapper.decode(&report(&[0])).unwrap();
    assert_eq!(mapper.decode(&report(&[1])).unwrap().iter().count(), 1);
}
#[test]
fn active_low_buttons_and_independent_decks_remain_explicit() {
    let mut mapper = HidMapper::new(profile(HidBindingAction::Toggle {
        deck: 1,
        zero_is_pressed: true,
    }))
    .unwrap();
    assert_eq!(mapper.decode(&report(&[0])).unwrap().iter().count(), 0);
    mapper.decode(&report(&[1])).unwrap();
    assert_eq!(
        mapper.decode(&report(&[0])).unwrap().iter().next(),
        Some(HidMappedAction::Toggle { deck: 1 })
    );
}
#[test]
fn numbered_cross_byte_fields_ignore_id_and_os_padding() {
    let mut profile = profile(HidBindingAction::Seek {
        deck: 0,
        minimum: 0,
        maximum: 255,
    });
    profile.bindings[0].field = HidField {
        report_id: 5,
        bit_offset: 4,
        bit_width: 8,
        signed: false,
    };
    let mut mapper = HidMapper::new(profile).unwrap();
    let packet = HidReport {
        report_id: 5,
        data: vec![5, 0xf0, 0x0f, 0x11],
        logical_length: 3,
    };
    assert_eq!(
        mapper.decode(&packet).unwrap().iter().next(),
        Some(HidMappedAction::Seek { deck: 0, unit: 1.0 })
    );
    assert_eq!(
        mapper.decode(&packet).unwrap().iter().count(),
        0,
        "unchanged axes must not seek repeatedly"
    );
    assert_eq!(
        mapper.decode(&report(&[0xff])).unwrap().iter().count(),
        0,
        "unrelated report IDs are not fields"
    );
}
#[test]
fn signed_encoders_and_full_width_unsigned_axes_do_not_overflow() {
    let mut profile = profile(HidBindingAction::PitchEncoder {
        deck: 1,
        percent_per_step: 0.5,
    });
    profile.bindings[0].field.signed = true;
    let mut mapper = HidMapper::new(profile).unwrap();
    assert_eq!(
        mapper.decode(&report(&[0x80])).unwrap().iter().next(),
        Some(HidMappedAction::PitchDelta {
            deck: 1,
            percent: -64.0
        })
    );
    assert_eq!(
        mapper.decode(&report(&[0x7f])).unwrap().iter().next(),
        Some(HidMappedAction::PitchDelta {
            deck: 1,
            percent: 63.5
        })
    );
    assert_eq!(
        mapper.decode(&report(&[0x7f])).unwrap().iter().count(),
        1,
        "relative deltas cannot be coalesced as duplicates"
    );
    let mut profile = crate_profile_axis();
    profile.bindings[0].field.bit_width = 32;
    let mut mapper = HidMapper::new(profile).unwrap();
    assert_eq!(
        mapper.decode(&report(&[0xff; 4])).unwrap().iter().next(),
        Some(HidMappedAction::Pitch { deck: 0, unit: 1.0 })
    );
}
fn crate_profile_axis() -> HidProfile {
    profile(HidBindingAction::Pitch {
        deck: 0,
        minimum: 0,
        maximum: i64::from(u32::MAX),
    })
}
#[test]
fn malformed_late_field_does_not_commit_partial_button_latches() {
    let mut profile = profile(HidBindingAction::Toggle {
        deck: 0,
        zero_is_pressed: false,
    });
    profile.bindings.push(HidBinding {
        when: None,
        field: HidField {
            bit_offset: 8,
            ..field()
        },
        action: HidBindingAction::Seek {
            deck: 1,
            minimum: 0,
            maximum: 255,
        },
    });
    let mut mapper = HidMapper::new(profile).unwrap();
    assert!(mapper.decode(&report(&[0])).is_err());
    let actions = mapper.decode(&report(&[1, 0])).unwrap();
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, HidMappedAction::Toggle { .. })),
        "failed report must not arm first button"
    );
}
#[test]
fn unsafe_malformed_and_unbounded_profiles_are_rejected_before_capture() {
    let valid = profile(HidBindingAction::Seek {
        deck: 0,
        minimum: 0,
        maximum: 255,
    });
    let bytes = serde_json::to_vec(&valid).unwrap();
    assert!(HidProfile::parse(&bytes).is_ok());
    assert!(HidProfile::parse(&vec![b' '; MAX_HID_PROFILE_BYTES + 1]).is_err());
    let mut keyboard = valid.clone();
    keyboard.usage_page = 1;
    keyboard.usage = 6;
    assert_eq!(keyboard.validate(), Err(IoError::ProtectedInput));
    for width in [0, 33] {
        let mut invalid = valid.clone();
        invalid.bindings[0].field.bit_width = width;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = valid.clone();
    invalid.bindings[0].action = HidBindingAction::PitchEncoder {
        deck: 0,
        percent_per_step: f64::NAN,
    };
    assert!(invalid.validate().is_err());
    let mut invalid = valid.clone();
    invalid.bindings[0].action = HidBindingAction::Pitch {
        deck: 2,
        minimum: 0,
        maximum: 255,
    };
    assert!(invalid.validate().is_err());
    let mut invalid = valid.clone();
    invalid.bindings[0].action = HidBindingAction::Pitch {
        deck: 0,
        minimum: 5,
        maximum: 5,
    };
    assert!(invalid.validate().is_err());
    let mut invalid = valid;
    invalid.bindings = vec![invalid.bindings[0].clone(); MAX_HID_BINDINGS + 1];
    assert!(invalid.validate().is_err());
    let json = std::str::from_utf8(&bytes)
        .unwrap()
        .replacen('{', "{\"undocumented\":true,", 1);
    assert!(HidProfile::parse(json.as_bytes()).is_err());
}
#[test]
fn all_fields_must_fit_declared_descriptor_before_any_events_are_applied() {
    let layout = ReportLayout::parse(&[
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 1, 0x81, 2, 0xc0,
    ])
    .unwrap();
    let valid = profile(HidBindingAction::Seek {
        deck: 0,
        minimum: 0,
        maximum: 255,
    });
    assert!(valid.validate_layout(&layout).is_ok());
    let mut absent_id = valid.clone();
    absent_id.bindings[0].field.report_id = 7;
    assert!(absent_id.validate_layout(&layout).is_err());
    let mut oversized = valid;
    oversized.bindings[0].field.bit_offset = 1;
    assert!(oversized.validate_layout(&layout).is_err());
}
#[test]
fn touchpad_activation_blocks_release_coordinates_and_validates_all_gate_fields() {
    let mut profile = profile(HidBindingAction::Seek {
        deck: 0,
        minimum: 0,
        maximum: 255,
    });
    profile.bindings[0].field.bit_offset = 8;
    profile.bindings[0].when = Some(HidCondition {
        field: HidField {
            bit_width: 1,
            ..field()
        },
        equals: 1,
    });
    let mut mapper = HidMapper::new(profile.clone()).unwrap();
    assert_eq!(mapper.decode(&report(&[0, 0])).unwrap().iter().count(), 0);
    assert_eq!(mapper.decode(&report(&[1, 127])).unwrap().iter().count(), 1);
    assert_eq!(
        mapper.decode(&report(&[0, 0])).unwrap().iter().count(),
        0,
        "release must not seek to zero"
    );
    assert_eq!(
        mapper.decode(&report(&[1, 127])).unwrap().iter().count(),
        1,
        "a new contact may start at the same coordinate"
    );
    profile.bindings[0].when.as_mut().unwrap().field.report_id = 1;
    assert!(profile.validate().is_err());
}
#[test]
fn gated_buttons_cannot_retrigger_a_held_press_when_condition_reappears() {
    let mut profile = profile(HidBindingAction::Toggle {
        deck: 0,
        zero_is_pressed: false,
    });
    profile.bindings[0].field.bit_width = 1;
    profile.bindings[0].when = Some(HidCondition {
        field: HidField {
            bit_offset: 1,
            bit_width: 1,
            ..field()
        },
        equals: 1,
    });
    let mut mapper = HidMapper::new(profile).unwrap();
    for value in [1, 3, 2] {
        assert_eq!(mapper.decode(&report(&[value])).unwrap().iter().count(), 0);
    }
    assert_eq!(mapper.decode(&report(&[3])).unwrap().iter().count(), 1);
    mapper.decode(&report(&[1])).unwrap();
    assert_eq!(mapper.decode(&report(&[3])).unwrap().iter().count(), 0);
}
