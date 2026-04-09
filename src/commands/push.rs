use serde::Serialize;

use crate::error::Error;
use crate::exec::exec;

#[derive(Serialize)]
struct PushArgs<'a, Payload: Serialize + ?Sized> {
    event: &'a str,
    value: &'a Payload,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<&'a str>,
}

/// Pushes `event` with `payload` to the root LiveView.
///
/// The payload is JSON-serialized and delivered to the server as if a
/// `phx-click={JS.push("event", value: payload)}` had fired. The server
/// handles it in `handle_event/3`. This call is fire-and-forget: there is
/// no reply callback.
///
/// Equivalent to `Phoenix.LiveView.JS.push(event, value: payload)`.
///
/// # Errors
///
/// Returns [`Error::Serialize`] if `payload` cannot be JSON-encoded, plus
/// the usual browser-bridge errors. See [`Error`].
///
/// # Examples
///
/// Ad-hoc JSON with [`serde_json::json!`]:
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// lv::push_event("submit_word", &serde_json::json!({
///     "word": "TRY",
///     "route": [0, 1, 2],
/// }))?;
/// # Ok::<(), lv::Error>(())
/// ```
///
/// Typed payload via [`derive(Serialize)`](serde::Serialize) -- no `json!`
/// allocation, field names checked at compile time:
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// #[derive(serde::Serialize)]
/// struct Submit<'a> {
///     word: &'a str,
///     route: &'a [usize],
/// }
///
/// lv::push_event("submit_word", &Submit { word: "TRY", route: &[0, 1, 2] })?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn push_event<Payload>(event: &str, payload: &Payload) -> Result<(), Error>
where
    Payload: Serialize + ?Sized,
{
    exec(
        "push",
        &PushArgs {
            event,
            value: payload,
            target: None,
        },
    )
}

/// Pushes `event` with `payload` to a specific `phx-target`.
///
/// `target` is either a LiveComponent CID (as a string, for example `"1"`)
/// or a CSS selector such as `"#chat"`. Use this when the handling
/// `handle_event/3` lives on a [`Phoenix.LiveComponent`] rather than the
/// root LiveView.
///
/// Equivalent to `Phoenix.LiveView.JS.push(event, value: payload, target: ...)`.
///
/// # Errors
///
/// Same as [`push_event`].
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// #[derive(serde::Serialize)]
/// struct ChatSend<'a> { text: &'a str }
///
/// lv::push_event_to("#chat", "send", &ChatSend { text: "hi" })?;
/// # Ok::<(), lv::Error>(())
/// ```
///
/// [`Phoenix.LiveComponent`]: https://hexdocs.pm/phoenix_live_view/Phoenix.LiveComponent.html
pub fn push_event_to<Payload>(
    target: &str,
    event: &str,
    payload: &Payload,
) -> Result<(), Error>
where
    Payload: Serialize + ?Sized,
{
    exec(
        "push",
        &PushArgs {
            event,
            value: payload,
            target: Some(target),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::encode_command;
    use serde_json::Value;

    #[derive(Serialize)]
    struct Submit<'a> {
        word: &'a str,
        route: &'a [usize],
    }

    #[test]
    fn without_target() {
        let submit = Submit {
            word: "TRY",
            route: &[0, 1, 2],
        };
        let args = PushArgs {
            event: "submit_word",
            value: &submit,
            target: None,
        };
        let parsed: Value = serde_json::from_str(&encode_command("push", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "push");
        assert_eq!(parsed[0][1]["event"], "submit_word");
        assert_eq!(parsed[0][1]["value"]["word"], "TRY");
        assert_eq!(parsed[0][1]["value"]["route"][2], 2);
        assert!(parsed[0][1].get("target").is_none());
    }

    #[test]
    fn with_target() {
        let args = PushArgs {
            event: "noop",
            value: &serde_json::json!({}),
            target: Some("#form"),
        };
        let parsed: Value = serde_json::from_str(&encode_command("push", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][1]["target"], "#form");
    }

    #[test]
    fn non_wasm_stubs_to_ok() {
        assert!(push_event("ping", &serde_json::json!({})).is_ok());
    }
}
