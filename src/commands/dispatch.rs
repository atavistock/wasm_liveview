use serde::Serialize;

use crate::error::Error;
use crate::exec::exec;

#[derive(Serialize)]
struct DispatchArgs<'a, Detail: Serialize + ?Sized> {
    event: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a Detail>,
    bubbles: bool,
}

/// Dispatches a DOM `CustomEvent` named `event` on an element.
///
/// `to` picks the target element via CSS selector (LiveView root when
/// `None`). The event always bubbles.
///
/// Equivalent to `Phoenix.LiveView.JS.dispatch(event, to: ...)`.
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
/// // Fire a `wasm:tick` event on the LiveView root.
/// lv::dispatch("wasm:tick", None)?;
///
/// // Or target a specific element.
/// lv::dispatch("wasm:tick", Some("#board"))?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn dispatch(event: &str, to: Option<&str>) -> Result<(), Error> {
    exec(
        "dispatch",
        &DispatchArgs::<()> {
            event,
            to,
            detail: None,
            bubbles: true,
        },
    )
}

/// Like [`dispatch`], but attaches a serializable `detail` payload that
/// becomes `event.detail` on the dispatched `CustomEvent`.
///
/// # Errors
///
/// [`Error::Serialize`] if `detail` can't be JSON-encoded; otherwise as
/// [`dispatch`].
///
/// # Examples
///
/// Ad-hoc JSON:
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// lv::dispatch_with(
///     "wasm:score",
///     Some("#score"),
///     &serde_json::json!({ "delta": 5 }),
/// )?;
/// # Ok::<(), lv::Error>(())
/// ```
///
/// Typed payload:
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// #[derive(serde::Serialize)]
/// struct ScoreDelta { delta: i32 }
///
/// lv::dispatch_with("wasm:score", Some("#score"), &ScoreDelta { delta: 5 })?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn dispatch_with<Detail>(
    event: &str,
    to: Option<&str>,
    detail: &Detail,
) -> Result<(), Error>
where
    Detail: Serialize + ?Sized,
{
    exec(
        "dispatch",
        &DispatchArgs {
            event,
            to,
            detail: Some(detail),
            bubbles: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::encode_command;
    use serde_json::Value;

    #[test]
    fn without_detail_omits_detail_key() {
        let args = DispatchArgs::<()> {
            event: "wasm:tick",
            to: Some("#board"),
            detail: None,
            bubbles: true,
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("dispatch", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "dispatch");
        assert_eq!(parsed[0][1]["event"], "wasm:tick");
        assert_eq!(parsed[0][1]["to"], "#board");
        assert_eq!(parsed[0][1]["bubbles"], true);
        assert!(parsed[0][1].get("detail").is_none());
    }

    #[test]
    fn with_detail_serializes_payload() {
        let payload = serde_json::json!({ "delta": 5 });
        let args = DispatchArgs {
            event: "wasm:score",
            to: None,
            detail: Some(&payload),
            bubbles: true,
        };
        let parsed: Value =
            serde_json::from_str(&encode_command("dispatch", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][1]["detail"]["delta"], 5);
        assert!(parsed[0][1].get("to").is_none());
    }

    #[test]
    fn non_wasm_stubs_to_ok() {
        assert!(dispatch("wasm:noop", None).is_ok());
    }
}
