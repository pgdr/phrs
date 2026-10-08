#[derive(Debug)]
pub struct PhError {
    pub message: String,
    pub code: u8,
}

impl PhError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }
}
impl From<std::io::Error> for PhError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}
impl From<polars::error::PolarsError> for PhError {
    fn from(value: polars::error::PolarsError) -> Self {
        Self::new(value.to_string())
    }
}
