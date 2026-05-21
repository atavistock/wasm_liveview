//! The crate-wide [`Error`] type.

/// Failure modes for every outbound command and for [`crate::subscribe`].
///
/// Error messages are stable enough to log but not to pattern-match on --
/// match the variant.
#[derive(Debug)]
pub enum Error {
    /// `window` is not available (e.g. a web worker without DOM access).
    NoWindow,

    /// `window.document` is missing.
    NoDocument,

    /// `window.liveSocket` has not been set. The page hasn't loaded `app.js`
    /// yet or isn't running LiveView.
    NoLiveSocket,

    /// No `[data-phx-session]` element in the DOM, i.e. no LV is mounted.
    NoLiveViewRoot,

    /// `serde_json` could not (de)serialize a payload. Inner string is the
    /// serde message.
    Serialize(String),

    /// `liveSocket.execJS` threw. Inner string is the best-effort message
    /// from the JS `Error` object.
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
