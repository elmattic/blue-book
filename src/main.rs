use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Flac,
    Alac,
}

impl AudioFormat {
    /// Returns the ffmpeg codec string
    pub fn codec(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "flac",
            AudioFormat::Alac => "alac",
        }
    }

    /// Returns the file extension
    pub fn suffix(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "flac",
            AudioFormat::Alac => "m4a",
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FilterConfig {
    pub barcode: Option<String>,
    pub country: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RipConfig {
    pub skip: bool,
    pub passes: u32,
    pub device: Option<PathBuf>,
}

impl Default for RipConfig {
    fn default() -> Self {
        Self {
            skip: false,
            passes: 5,
            device: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncodeConfig {
    pub format: AudioFormat,
}

impl Default for EncodeConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::Flac,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlacConfig {
    pub compression_level: u32,
    pub cue_sheet: bool,
}

impl Default for FlacConfig {
    fn default() -> Self {
        Self {
            compression_level: 8,
            cue_sheet: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub dir: String,
    pub file: String,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            dir: "{artist}/{album}".into(),
            file: "{tracknumber:02d} - {title}.{suffix}".into(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub rip: RipConfig,
    #[serde(default)]
    pub encode: EncodeConfig,
    #[serde(default)]
    pub flac: FlacConfig,
    #[serde(default)]
    pub template: TemplateConfig,
}

fn main() {
    println!("Hello, world!");
}
