use operon_gui::state::AppState;

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

#[test]
fn zoom_controls_stay_in_supported_bounds() {
    let mut state = AppState::new();

    // The default zoom should start at 100%.
    assert!(approx_eq(state.ui_scale(), 1.0));

    // One zoom step should move the scale in the expected direction.
    state.zoom_in();
    assert!(approx_eq(state.ui_scale(), 1.1));

    state.zoom_out();
    assert!(approx_eq(state.ui_scale(), 1.0));

    // Big jumps should still stay inside the supported clamp range.
    state.set_ui_scale(99.0);
    assert!(approx_eq(state.ui_scale(), 1.5));

    state.set_ui_scale(0.01);
    assert!(approx_eq(state.ui_scale(), 0.8));

    // Resetting the zoom should always return to the neutral 1.0 scale.
    state.reset_zoom();
    assert!(approx_eq(state.ui_scale(), 1.0));
}

#[test]
fn reload_generation_increments_monotonically() {
    let mut state = AppState::new();

    assert_eq!(state.reload_generation(), 0);
    assert_eq!(state.mark_reload(), 1);
    assert_eq!(state.mark_reload(), 2);
    assert_eq!(state.reload_generation(), 2);
}
