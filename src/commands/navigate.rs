use serde::Serialize;

use crate::error::Error;
use crate::exec::exec;

#[derive(Serialize)]
struct NavArgs<'a> {
    href: &'a str,
    replace: bool,
}

/// Client-side navigation to `href`.
///
/// Tears down the current LiveView and mounts the destination, the same way
/// a `<.link navigate={...}>` click does. Pass `replace = true` to replace
/// the current history entry instead of pushing a new one.
///
/// Equivalent to `Phoenix.LiveView.JS.navigate/2`.
///
/// # Errors
///
/// See [`Error`].
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
/// lv::navigate("/room/42", false)?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn navigate(href: &str, replace: bool) -> Result<(), Error> {
    exec("navigate", &NavArgs { href, replace })
}

/// Client-side patch to `href` within the current LiveView.
///
/// Stays in the current LV (does not remount) and re-runs `handle_params/3`
/// with the new URL. Pass `replace = true` to replace the history entry.
///
/// Equivalent to `Phoenix.LiveView.JS.patch/2`.
///
/// # Errors
///
/// See [`Error`].
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
/// lv::patch("/room/42?tab=chat", true)?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn patch(href: &str, replace: bool) -> Result<(), Error> {
    exec("patch", &NavArgs { href, replace })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::encode_command;
    use serde_json::Value;

    #[test]
    fn navigate_encodes_href_and_replace() {
        let args = NavArgs {
            href: "/room/42",
            replace: true,
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("navigate", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "navigate");
        assert_eq!(parsed[0][1]["href"], "/room/42");
        assert_eq!(parsed[0][1]["replace"], true);
    }

    #[test]
    fn patch_defaults_to_push_history() {
        let args = NavArgs {
            href: "/room/42?tab=chat",
            replace: false,
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("patch", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "patch");
        assert_eq!(parsed[0][1]["replace"], false);
    }

    #[test]
    fn non_wasm_stubs_to_ok() {
        assert!(navigate("/", false).is_ok());
    }
}
