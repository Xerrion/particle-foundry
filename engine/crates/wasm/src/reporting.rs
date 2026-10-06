use wasm_bindgen::prelude::*;

/// Installs panic reporting at the browser boundary before initialization.
#[wasm_bindgen(start)]
pub fn start() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(window) = web_sys::window() {
            let error = js_sys::Error::new(&info.to_string());
            error.set_name("RustPanic");
            let options = web_sys::CustomEventInit::new();
            options.set_detail(error.as_ref());
            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(
                "particle-foundry:rust-panic",
                &options,
            ) {
                let _ = window.dispatch_event(&event);
            }
        }
        previous(info);
    }));
}

/// Causes a deliberate panic in a separate reporting verification build.
/// Normal WASM builds do not expose this function.
#[cfg(feature = "glitchtip-smoke")]
#[wasm_bindgen]
pub fn verify_reporting_panic() {
    panic!("Particle Foundry Rust WASM reporting verification");
}
