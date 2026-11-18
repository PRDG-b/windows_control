use std::time::Duration;
use windows_control::{
    execution::Observation, exploration::Permission, exploration_web::scene_from_ocr, ocr::TextBox,
};
fn observation(words: &[&str]) -> Observation {
    Observation {
        frame_id: 1,
        captured_at: Duration::ZERO,
        width: 1920,
        height: 1080,
        boxes: words
            .iter()
            .enumerate()
            .map(|(i, s)| TextBox {
                text: (*s).into(),
                x: 20,
                y: 100 + i as u16 * 35,
                w: 100,
                h: 25,
                confidence: 95.,
            })
            .collect(),
    }
}
#[test]
fn recognizes_test_page_only_and_never_resolves_duplicate_run() {
    let page = scene_from_ocr(observation(&[
        "Execute",
        "Validate",
        "SUCCESSMODE",
        "READY",
        "RUN",
        "FAILURE",
        "DUPLICATE",
        "POPUP",
    ]));
    assert_eq!(page.state.as_deref(), Some("success_ready"));
    assert_eq!(page.candidates[0].key, "run");
    assert_eq!(page.candidates[0].permission, Permission::Allowed);
    let page = scene_from_ocr(observation(&[
        "Execute",
        "Validate",
        "DUPLICATEMODE",
        "READY",
        "RUN",
        "RUN",
    ]));
    assert_eq!(page.state.as_deref(), Some("duplicate_ready"));
    assert_eq!(page.candidates[0].permission, Permission::Unknown);
    assert!(
        scene_from_ocr(observation(&["SUCCESSMODE", "READY", "RUN"]))
            .state
            .is_none()
    );
    assert!(
        scene_from_ocr(observation(&[
            "Execute",
            "Validate",
            "SUCCESSMODE",
            "WAITING",
            "RUN"
        ]))
        .state
        .is_none()
    );
    assert!(
        scene_from_ocr(observation(&[
            "Execute",
            "Validate",
            "SUCCESSMODE",
            "READY",
            "COMPLETED",
            "RUN"
        ]))
        .state
        .is_none()
    );
}

#[test]
fn popup_selects_confirm_instead_of_underlying_run_and_rejects_low_confidence_phase() {
    let page = scene_from_ocr(observation(&[
        "Execute",
        "Validate",
        "POPUPMODE",
        "READY",
        "RUN",
        "CONFIRM",
    ]));
    assert_eq!(page.state.as_deref(), Some("popup_confirm"));
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].key, "confirm");
    let mut low = observation(&["Execute", "Validate", "SUCCESSMODE", "READY", "RUN"]);
    low.boxes[3].confidence = 20.;
    assert!(scene_from_ocr(low).state.is_none());
    let mut unclear_popup = observation(&[
        "Execute",
        "Validate",
        "POPUPMODE",
        "READY",
        "RUN",
        "CONFIRM",
    ]);
    unclear_popup.boxes[5].confidence = 20.;
    assert!(
        scene_from_ocr(unclear_popup).state.is_none(),
        "uncertain overlay must not expose underlying RUN"
    );
}
