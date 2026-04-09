//! `[[op, args]]` wire-format encoding plus the `liveSocket.execJS`
//! trampoline. See the crate README.

use serde::Serialize;

use crate::error::Error;

/// Encodes a single `Phoenix.LiveView.JS` command as the `[[op, args]]`
/// JSON string that `liveSocket.execJS` expects.
pub fn encode_command<Args>(op: &str, args: &Args) -> Result<String, Error>
where
    Args: Serialize + ?Sized,
{
    Ok(serde_json::to_string(&[(op, args)])?)
}

/// Encodes `[[op, args]]` and ships it through `liveSocket.execJS`.
pub fn exec<Args>(op: &str, args: &Args) -> Result<(), Error>
where
    Args: Serialize + ?Sized,
{
    exec_js(&encode_command(op, args)?)
}

/// Zero-sized stand-in for commands that take no options. Serializes to
/// `{}`, which is what LiveView expects for ops like `pop_focus`.
#[derive(Serialize)]
pub struct NoArgs {}

#[cfg(target_arch = "wasm32")]
fn exec_js(command: &str) -> Result<(), Error> {
    use wasm_bindgen::{JsCast, JsValue};

    crate::cache::with_live_view(|exec_js, live_socket, root| {
        exec_js
            .call2(live_socket, root.unchecked_ref(), &JsValue::from_str(command))
            .map_err(|error| Error::ExecFailed(js_error_message(&error)))?;
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn exec_js(_command: &str) -> Result<(), Error> {
    Ok(())
}

/// Best-effort conversion of a JS exception value into a readable message:
/// unwraps `Error.message` or a direct string, falling back to Debug.
#[cfg(target_arch = "wasm32")]
fn js_error_message(error: &wasm_bindgen::JsValue) -> String {
    use wasm_bindgen::JsCast;

    if let Some(js_error) = error.dyn_ref::<js_sys::Error>() {
        return js_error.message().into();
    }
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_wraps_op_and_args_in_double_array() {
        let output = encode_command("pop_focus", &NoArgs {}).unwrap();
        assert_eq!(output, r#"[["pop_focus",{}]]"#);
    }

    #[test]
    fn encode_with_nested_args() {
        #[derive(Serialize)]
        struct Args<'a> {
            href: &'a str,
        }
        let output = encode_command("navigate", &Args { href: "/x" }).unwrap();
        assert_eq!(output, r#"[["navigate",{"href":"/x"}]]"#);
    }

    #[test]
    fn non_wasm_exec_stubs_to_ok() {
        assert!(exec("noop", &NoArgs {}).is_ok());
    }
}
