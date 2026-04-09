use serde::Serialize;

use crate::error::Error;
use crate::exec::exec;

#[derive(Serialize)]
struct ExecArgs<'a> {
    attr: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
}

/// Runs the JS command chain stored in a `data-*` attribute.
///
/// The attribute named `attr` on the element at `to` (or the LiveView root
/// when `None`) must contain a JS command chain encoded by
/// `Phoenix.LiveView.JS`. That chain is then executed in place.
///
/// Mirrors `Phoenix.LiveView.JS.exec/2`, which is typically used to stash
/// a pre-built command on an element (for example `data-show={JS.show(...)}`)
/// and trigger it later without re-sending it over the wire.
///
/// # Errors
///
/// Returns [`Error::NoLiveSocket`], [`Error::NoLiveViewRoot`], or
/// [`Error::ExecFailed`]. See [`Error`].
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// // Run the command chain stored in `#modal`'s `data-show` attribute.
/// lv::exec_attr("data-show", Some("#modal"))?;
/// # Ok::<(), lv::Error>(())
/// ```
pub fn exec_attr(attr: &str, to: Option<&str>) -> Result<(), Error> {
    exec("exec", &ExecArgs { attr, to })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::encode_command;
    use serde_json::Value;

    #[test]
    fn encodes_attr_and_to() {
        let args = ExecArgs {
            attr: "data-show",
            to: Some("#modal"),
        };
        let parsed: Value = serde_json::from_str(&encode_command("exec", &args).unwrap()).unwrap();
        assert_eq!(parsed[0][0], "exec");
        assert_eq!(parsed[0][1]["attr"], "data-show");
        assert_eq!(parsed[0][1]["to"], "#modal");
    }

    #[test]
    fn non_wasm_stubs_to_ok() {
        assert!(exec_attr("data-show", Some("#modal")).is_ok());
    }
}
