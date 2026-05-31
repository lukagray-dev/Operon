use operon_gui::window::startup::calculate_startup_geometry;

#[test]
fn startup_geometry_uses_a_centered_16_by_9_layout_on_a_standard_monitor() {
    let geometry = calculate_startup_geometry(1920, 1080, 100, 50);

    assert_eq!(geometry.width, 1344);
    assert_eq!(geometry.height, 756);
    assert_eq!(geometry.x, 388);
    assert_eq!(geometry.y, 212);
}

#[test]
fn startup_geometry_still_fits_on_taller_monitors() {
    let geometry = calculate_startup_geometry(1024, 768, 0, 0);

    assert_eq!(geometry.width, 717);
    assert_eq!(geometry.height, 403);
    assert_eq!(geometry.x, 153);
    assert_eq!(geometry.y, 182);
}
