//! HLS Playlist Parsing and Mouflon Decryption
//!
//! Stripchat  HLS m3u8 ， URL  fMP4  URL。
//! Mouflon ： SHA-256  URL  XOR 。
//!
//! Parses Stripchat's HLS m3u8 playlists, extracting segment URLs and fMP4 init segment URLs.
//! Supports the Mouflon encryption system: XOR-decrypts segment URLs using SHA-256 keys.

use crate::core::error::{AppError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::LazyLock;

/// URL 。
/// Regex for extracting the encrypted string and sequence number from an encrypted URL.
static SEGMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"_([^_]+)_(\d+(?:_part\d+)?)\.mp4(?:[?#].*)?").unwrap());

/// HLS segment information
#[derive(Debug, Clone)]
pub struct HlsSegment {
    /// URL（）/ Full segment URL (decrypted)
    pub url: String,
    /// （）/ Segment sequence number (for deduplication)
    pub sequence: u32,
}

/// HLS m3u8 ， fMP4  URL。
/// Parse an HLS m3u8 playlist, returning the segment list and fMP4 init segment URL.
///
/// Parameters
/// m3u8 text content
/// Prefix for converting relative paths to absolute URLs
/// - `mouflon_keys`: Mouflon （pkey -> pdkey）/ Mouflon decryption key map (pkey -> pdkey)
///
/// Returns
/// Tuple of `(segments, init_url)`
pub fn parse_playlist(
    playlist: &str,
    url_prefix: &str,
    mouflon_keys: &HashMap<String, String>,
) -> Result<(Vec<HlsSegment>, Option<String>)> {
    let mut segments = Vec::new();
    let mut mp4_header_url = None;
    let mut current_pkey: Option<&str> = None;

    let lines: Vec<&str> = playlist.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        // Mouflon ， pkey
        // Parse Mouflon encryption tag to get the decryption key for the current pkey
        if line.contains("#EXT-X-MOUFLON:PSCH") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 {
                let pkey = parts[3];
                current_pkey = mouflon_keys.get(pkey).map(|s| s.as_str());
            }
        }

        // fMP4  URL（EXT-X-MAP）
        // Parse fMP4 init segment URL (EXT-X-MAP)
        if line.contains("EXT-X-MAP:URI")
            && let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            let header_path = &line[start + 1..start + 1 + end];
            mp4_header_url = Some(if header_path.starts_with("http") {
                header_path.to_string()
            } else {
                format!("{}/{}", url_prefix, header_path)
            });
        }

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Mouflon URI （ URL ）
        // Check if the previous line is a Mouflon URI tag (actual URL for encrypted segments is in the tag)
        let mouflon_uri_line = if i > 0 && lines[i - 1].starts_with("#EXT-X-MOUFLON:URI:") {
            Some(lines[i - 1])
        } else {
            None
        };

        let url = if let Some(mouflon_line) = mouflon_uri_line {
            let raw_url = mouflon_line.trim_start_matches("#EXT-X-MOUFLON:URI:");
            let encoded_url = if raw_url.starts_with("https://") {
                raw_url.to_string()
            } else if raw_url.starts_with("//") {
                format!("https:{}", raw_url)
            } else {
                format!("https://{}", raw_url)
            };

            // URL，
            // Decrypt URL if key is available, otherwise use as-is
            if let Some(key) = current_pkey {
                decrypt_segment_url(&encoded_url, key).unwrap_or(encoded_url)
            } else {
                encoded_url
            }
        } else if line.starts_with("http") {
            line.to_string()
        } else {
            format!("{}/{}", url_prefix, line)
        };

        let sequence = extract_sequence(&url).unwrap_or(segments.len() as u32);
        segments.push(HlsSegment { url, sequence });
    }

    Ok((segments, mp4_header_url))
}

/// URL  URL （）。
/// Extract the URL prefix from a full URL (removes the last path segment).
pub fn get_url_prefix(url: &str) -> String {
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() > 1 {
        parts[..parts.len() - 1].join("/")
    } else {
        url.to_string()
    }
}

/// SHA-256  Mouflon  URL  XOR 。
/// Decrypt a Mouflon-encrypted segment URL using XOR with a SHA-256 key.
///
/// ： → Base64 （）→ SHA-256(key) XOR  →  URL
/// Decryption flow: extract encrypted string → Base64 decode (reversed + padded) → SHA-256(key) XOR decrypt → replace in URL
fn decrypt_segment_url(encoded_url: &str, key: &str) -> Result<String> {
    let captures = SEGMENT_REGEX
        .captures(encoded_url)
        .ok_or_else(|| AppError::Other("Cannot parse encrypted URL".to_string()))?;

    let encrypted_str = captures.get(1).unwrap().as_str();

    // Reverse string and pad for Base64
    let mut reversed: String = encrypted_str.chars().rev().collect();
    while !reversed.len().is_multiple_of(4) {
        reversed.push('=');
    }

    let encrypted_bytes = STANDARD
        .decode(&reversed)
        .map_err(|e| AppError::Other(format!("Base64 decode error: {}", e)))?;

    // Use SHA-256(key) as XOR keystream
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let key_bytes = hasher.finalize();

    let decrypted: Vec<u8> = encrypted_bytes
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();

    let decrypted_str = String::from_utf8_lossy(&decrypted);
    Ok(encoded_url.replace(encrypted_str, &decrypted_str))
}

/// URL （ `_` 、`.` ）。
/// Extract the sequence number from a segment URL's filename (number after the last `_`, before `.`).
fn extract_sequence(url: &str) -> Option<u32> {
    let filename = url.split('/').next_back()?;
    let parts: Vec<&str> = filename.split('_').collect();
    let last = parts.last()?;
    let num_str = last.split('.').next()?;
    num_str.parse().ok()
}
