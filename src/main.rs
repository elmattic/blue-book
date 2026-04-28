use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Flac
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterConfig {
    pub barcode: Option<String>,
    pub country: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct EncodeConfig {
    pub format: AudioFormat,
}

impl Default for EncodeConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }
}

fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    let config = Config::load_from_file(PathBuf::from("config.toml"))?;

    dbg!(&config);

    Ok(())
}
