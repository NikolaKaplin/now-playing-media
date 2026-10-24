use crate::validation::validate_control;

#[test]
fn validates_controls_before_contacting_the_os() {
    assert!(validate_control("pause", "", None).is_ok());
    assert!(validate_control("seek", "player", Some(12_000.0)).is_ok());
    assert!(validate_control("delete", "", None).is_err());
    assert!(validate_control("play", "bad\0id", None).is_err());
    for position in [
        None,
        Some(-1.0),
        Some(0.5),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(9_007_199_254_740_992.0),
    ] {
        assert!(validate_control("seek", "", position).is_err());
    }
}
