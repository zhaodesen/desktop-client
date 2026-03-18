use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResponse<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<CommandError>,
}

impl<T> CommandResponse<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(CommandError::new(code, message)),
        }
    }
}
