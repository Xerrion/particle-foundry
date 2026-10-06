//! Explicit native error reporting verification. Requires a configured project DSN.

mod support;

use std::time::Duration;

fn main() -> Result<(), &'static str> {
    let guard =
        support::initialize_reporting()?.ok_or("Set GLITCHTIP_SIM_DSN before verification")?;
    if !guard.is_enabled() {
        return Err("Native error reporting is disabled");
    }
    let event_id = sentry::capture_message(
        "Particle Foundry native GlitchTip verification",
        sentry::Level::Error,
    );
    if event_id.is_nil() {
        return Err("The SDK did not capture the verification event");
    }
    if !guard.flush(Some(Duration::from_secs(10))) {
        return Err("The native reporting queue did not drain within 10 seconds");
    }
    println!("Native verification event captured: {event_id}");
    println!("Transport queue drained. Check GlitchTip for event ingestion.");
    Ok(())
}
