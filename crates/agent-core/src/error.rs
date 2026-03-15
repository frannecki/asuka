use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreError {
    pub status: u16,
    pub message: String,
}

impl CoreError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409, message)
    }

    pub fn not_found(entity: &str) -> Self {
        Self::new(404, format!("{entity} not found"))
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(502, message)
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for CoreError {}

impl From<diesel::ConnectionError> for CoreError {
    fn from(error: diesel::ConnectionError) -> Self {
        Self::new(500, format!("sqlite connection error: {error}"))
    }
}

impl From<diesel::result::Error> for CoreError {
    fn from(error: diesel::result::Error) -> Self {
        Self::new(500, format!("sqlite query error: {error}"))
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
