//! Application Error Type Definitions
//!
//! `AppError` ， IO、、JSON ，
//! `serde::Serialize`  Tauri 。
//!
//! Defines a unified `AppError` enum covering IO, network, JSON parsing, and business logic errors,
//! with `serde::Serialize` implemented so it can be returned directly to the frontend via Tauri commands.

use std::fmt;

/// Unified application error type
#[derive(Debug)]
pub enum AppError {
    /// File system IO error
    Io(std::io::Error),
    /// HTTP network request error
    Reqwest(reqwest::Error),
    /// JSON serialization/deserialization error
    Json(serde_json::Error),
    /// Stream is offline
    StreamOffline(String),
    /// Streamer is already being recorded
    AlreadyRecording(String),
    /// Streamer is not currently being recorded
    NotRecording(String),
    /// User not found
    UserNotFound(String),
    /// Other errors
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Reqwest(e) => write!(f, "Network error: {}", e),
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::StreamOffline(s) => write!(f, "Stream offline: {}", s),
            Self::AlreadyRecording(s) => write!(f, "Already recording: {}", s),
            Self::NotRecording(s) => write!(f, "Not recording: {}", s),
            Self::UserNotFound(s) => write!(f, "User not found: {}", s),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

/// ， Tauri 。
/// Serializes to an error message string so Tauri commands can return errors directly to the frontend.
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Unified Result type alias for the application
pub type Result<T> = std::result::Result<T, AppError>;
