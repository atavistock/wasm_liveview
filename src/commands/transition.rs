use serde::Serialize;

use crate::error::Error;
use crate::exec::exec;

/// Three class lists describing a CSS transition.
///
/// Matches the three-tuple accepted by `Phoenix.LiveView.JS.transition/3`:
///
/// - [`transition`](Self::transition) is applied for the full duration of
///   the animation.
/// - [`start`](Self::start) is applied at `t = 0` and removed when the
///   animation ends.
/// - [`end`](Self::end) is applied at `t = duration` and left on the
///   element.
///
/// Any field may be an empty slice.
///
/// # Example
///
/// ```
/// use wasm_liveview::TransitionClasses;
///
/// let fade_in = TransitionClasses {
///     transition: &["transition-opacity", "duration-150"],
///     start: &["opacity-0"],
///     end: &["opacity-100"],
/// };
/// # let _ = fade_in;
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct TransitionClasses<'a> {
    /// Classes applied for the full duration of the transition.
    pub transition: &'a [&'a str],
    /// Classes applied at the start (`t = 0`) and removed at the end.
    pub start: &'a [&'a str],
    /// Classes applied at the end (`t = duration`) and left on the element.
    pub end: &'a [&'a str],
}

#[derive(Serialize)]
struct TransitionArgs<'a> {
    transition: [&'a [&'a str]; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<u32>,
}

/// Runs a CSS transition on an element.
///
/// `classes` is the three-bucket class list (see [`TransitionClasses`]).
/// `to` picks the target element (LiveView root when `None`). `time_ms` is
/// the duration in milliseconds, or LiveView's default of `200` when
/// `None`.
///
/// Equivalent to `Phoenix.LiveView.JS.transition(..., to: ..., time: ...)`.
///
/// # Errors
///
/// See [`Error`].
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// lv::transition(
///     lv::TransitionClasses {
///         transition: &["fade-in"],
///         start: &["opacity-0"],
///         end: &["opacity-100"],
///     },
///     Some("#board"),
///     Some(150),
/// )?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn transition(
    classes: TransitionClasses<'_>,
    to: Option<&str>,
    time_ms: Option<u32>,
) -> Result<(), Error> {
    exec(
        "transition",
        &TransitionArgs {
            transition: [classes.transition, classes.start, classes.end],
            to,
            time: time_ms,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::encode_command;
    use serde_json::Value;

    #[test]
    fn encodes_three_class_lists() {
        let classes = TransitionClasses {
            transition: &["fade-in"],
            start: &["opacity-0"],
            end: &["opacity-100"],
        };
        let args = TransitionArgs {
            transition: [classes.transition, classes.start, classes.end],
            to: None,
            time: Some(150),
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("transition", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][1]["transition"][0][0], "fade-in");
        assert_eq!(parsed[0][1]["transition"][1][0], "opacity-0");
        assert_eq!(parsed[0][1]["transition"][2][0], "opacity-100");
        assert_eq!(parsed[0][1]["time"], 150);
        assert!(parsed[0][1].get("to").is_none());
    }
}
