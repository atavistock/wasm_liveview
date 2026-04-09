//! The crate-wide [`Error`] type.

/// Failure modes for every outbound command and for [`crate::subscribe`].
///
/// All variants are returned from [`Result`]s; none are meant to be constructed
/// by callers. Error messages are stable enough to log but not to pattern-match
/// on -- match the variant itself.
#[derive(Debug)]
pub enum Error {
    /// `window` is not available. Typically only seen in non-browser JS
    /// environments (for example a web worker without DOM access).
    NoWindow,

    /// `window.document` is missing. Same conditions as [`Error::NoWindow`].
    NoDocument,

    /// `window.liveSocket` has not been set. The page either has not loaded
    /// `app.js` yet or is not running LiveView at all.
    NoLiveSocket,

    /// No element with a `data-phx-session` attribute was found in the DOM.
    /// Every LiveView root carries this attribute; its absence means no LV
    /// is currently mounted on the page.
    NoLiveViewRoot,

    /// `serde_json` could not serialize (or deserialize) the payload. The
    /// inner string is the underlying serde message.
    Serialize(String),

    /// `liveSocket.execJS` threw a JS exception. The inner string is the
    /// best-effort message extracted from the JS `Error` object.
    ExecFailed(String),
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Serialize(error.to_string())
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoWindow => write!(formatter, "no browser window"),
            Error::NoDocument => write!(formatter, "no document on window"),
            Error::NoLiveSocket => write!(formatter, "window.liveSocket not initialized"),
            Error::NoLiveViewRoot => write!(formatter, "no [data-phx-session] element found"),
            Error::Serialize(message) => {
                write!(formatter, "could not serialize payload: {message}")
            }
            Error::ExecFailed(message) => {
                write!(formatter, "liveSocket.execJS threw: {message}")
            }
        }
    }
}

impl std::error::Error for Error {}
