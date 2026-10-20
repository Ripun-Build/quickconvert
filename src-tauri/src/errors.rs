//! App-wide error type. Serialises to `{ code, message }` for the UI (mirrored in `src/lib/types.ts`).
//!
//! `message` comes from `Display` and is shown to the user as-is, so it must be plain language
//! (SPECS §13). Technical detail goes in the variant's fields, which only appear in `Debug` output.

use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Couldn't read or save this file.")]
    Io(#[from] std::io::Error),

    #[error("QuickConvert can't open .{extension} files yet.")]
    Unsupported { extension: String },

    #[error("This file looks damaged and couldn't be read.")]
    Corrupt { detail: String },

    #[error("This PDF is password-protected.")]
    PasswordProtected,

    #[error("Cancelled.")]
    Cancelled,

    #[error("Something went wrong. Please try again.")]
    Internal { detail: String },
}

impl AppError {
    /// Stable machine-readable code (matches `ErrorCode` in `src/lib/types.ts`).
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Unsupported { .. } => "unsupported",
            AppError::Corrupt { .. } => "corrupt",
            AppError::PasswordProtected => "password_protected",
            AppError::Cancelled => "cancelled",
            AppError::Internal { .. } => "internal",
        }
    }
}

impl AppError {
    pub fn internal(detail: impl Into<String>) -> Self {
        AppError::Internal {
            detail: detail.into(),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::internal(err.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn to_json(err: AppError) -> serde_json::Value {
        serde_json::to_value(err).unwrap()
    }

    #[test]
    fn serialises_to_code_and_message() {
        assert_eq!(
            to_json(AppError::PasswordProtected),
            json!({ "code": "password_protected", "message": "This PDF is password-protected." })
        );
    }

    #[test]
    fn unsupported_names_the_extension() {
        let err = AppError::Unsupported {
            extension: "xyz".into(),
        };
        assert_eq!(
            to_json(err),
            json!({ "code": "unsupported", "message": "QuickConvert can't open .xyz files yet." })
        );
    }

    #[test]
    fn io_errors_convert_with_question_mark() {
        fn read() -> Result<(), AppError> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
            Ok(())
        }
        let value = to_json(read().unwrap_err());
        assert_eq!(value["code"], "io");
    }

    #[test]
    fn technical_detail_is_not_shown_to_the_user() {
        let err = AppError::Internal {
            detail: "pdfium: FPDF_LoadDocument failed".into(),
        };
        assert_eq!(
            to_json(err),
            json!({ "code": "internal", "message": "Something went wrong. Please try again." })
        );
    }
}
