use std::{borrow::Cow, env};

pub fn initialize_reporting() -> Result<Option<sentry::ClientInitGuard>, &'static str> {
    let dsn = match env::var("SENTRY_SIM_DSN") {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) | Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => return Err("SENTRY_SIM_DSN is not valid text"),
    };
    let dsn = dsn
        .parse::<sentry::types::Dsn>()
        .map_err(|_| "SENTRY_SIM_DSN is invalid")?;
    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = setting("SENTRY_RELEASE").or_else(|| sentry::release_name!());
    options.environment =
        setting("SENTRY_ENVIRONMENT").or(Some(Cow::Borrowed(if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        })));
    options.send_default_pii = false;
    Ok(Some(sentry::init(options)))
}

fn setting(name: &str) -> Option<Cow<'static, str>> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(Cow::Owned)
}
