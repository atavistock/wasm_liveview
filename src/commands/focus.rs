use serde::Serialize;

use crate::error::Error;
use crate::exec::{exec, NoArgs};

#[derive(Serialize)]
struct SelectorArgs<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
}

/// Focuses the element at `to`, or the LiveView root when `None`.
///
/// Equivalent to `Phoenix.LiveView.JS.focus(to: ...)`.
///
/// # Errors
///
/// See [`Error`] for browser-bridge failure modes.
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
/// lv::focus(Some("#first-name"))?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn focus(to: Option<&str>) -> Result<(), Error> {
    exec("focus", &SelectorArgs { to })
}

/// Focuses the first focusable descendant of `to`.
///
/// When `to` is `None` the search starts from the LiveView root. Equivalent
/// to `Phoenix.LiveView.JS.focus_first(to: ...)`.
///
/// # Errors
///
/// See [`Error`].
pub fn focus_first(to: Option<&str>) -> Result<(), Error> {
    exec("focus_first", &SelectorArgs { to })
}

/// Pushes the current focus onto LiveView's focus stack.
///
/// Pair with [`pop_focus`] to restore focus later (for example after closing
/// a modal). Equivalent to `Phoenix.LiveView.JS.push_focus(to: ...)`.
///
/// # Errors
///
/// See [`Error`].
pub fn push_focus(to: Option<&str>) -> Result<(), Error> {
    exec("push_focus", &SelectorArgs { to })
}

/// Pops the previously pushed focus off LiveView's focus stack.
///
/// No-op if the stack is empty. Equivalent to
/// `Phoenix.LiveView.JS.pop_focus/0`.
///
/// # Errors
///
/// See [`Error`].
pub fn pop_focus() -> Result<(), Error> {
    exec("pop_focus", &NoArgs {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::{encode_command, NoArgs};
    use serde_json::Value;

    #[test]
    fn focus_with_selector() {
        let args = SelectorArgs {
            to: Some("#first-name"),
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("focus", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "focus");
        assert_eq!(parsed[0][1]["to"], "#first-name");
    }

    #[test]
    fn focus_without_selector_omits_to() {
        let args = SelectorArgs { to: None };
        let parsed: Value =
            serde_json::from_str(&encode_command("focus", &args).unwrap()).unwrap();
        assert!(parsed[0][1].as_object().unwrap().is_empty());
    }

    #[test]
    fn pop_focus_takes_empty_args() {
        let parsed: Value =
            serde_json::from_str(&encode_command("pop_focus", &NoArgs {}).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "pop_focus");
        assert!(parsed[0][1].as_object().unwrap().is_empty());
    }

    #[test]
    fn non_wasm_stubs_to_ok() {
        assert!(pop_focus().is_ok());
    }
}
