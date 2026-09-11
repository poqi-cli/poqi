use super::*;

#[test]
fn parses_capslock_key_token() {
    let sequence = KeySequence::parse("capslock").expect("parse capslock");
    assert_eq!(sequence.combos.len(), 1);
    assert!(matches!(sequence.combos[0].code, KeyCodeSpec::CapsLock));
}

#[test]
fn parses_caps_lock_alias() {
    let sequence = KeySequence::parse("caps_lock").expect("parse caps_lock");
    assert!(matches!(sequence.combos[0].code, KeyCodeSpec::CapsLock));
}
