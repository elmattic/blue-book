use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

pub const RIPRIP_PATH: &'static str = "_riprip";

pub const DEFAULT_FOLDER: &'static str = ".blue-book";

fn normalize_barcode(value: &str) -> Result<String, String> {
    let normalized: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    Ok(normalized)
}

fn to_uppercase(value: &str) -> Result<String, String> {
    Ok(value.to_uppercase())
}

#[derive(Parser, Debug)]
#[command(
    name = "blue-book",
    version,
    about = "Bit-perfect audio extraction and archival for CDs.",
    // Ensures that the help message is clean and modern
    disable_help_subcommand = true,
    infer_long_args = false
)]
pub struct Cli {
    /// Show raw data for debugging
    #[arg(short, long)]
    pub verbose: bool,

    /// Filter release by barcode (e.g., 689230001720, 0-77774-62072-7)
    #[arg(short, long, value_parser = normalize_barcode)]
    pub barcode: Option<String>,

    /// Filter release by country code (e.g., US, GB)
    #[arg(short, long, value_parser = to_uppercase)]
    pub country: Option<String>,

    /// Filter release by date (YYYY-MM-DD)
    #[arg(short, long)]
    pub date: Option<String>,

    /// Filter release by id
    #[arg(long)]
    pub id: Option<String>,

    /// Skip the ripping process
    #[arg(short, long)]
    pub skip: bool,

    /// Output audio format
    #[arg(short, long, value_enum)]
    pub format: Option<AudioFormat>,

    /// Manually provide a TOC string; if empty, the CDTOC is extracted via riprip
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "EXTRACT"
    )]
    pub toc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
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
    pub id: Option<String>,
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
            file: "{tracknumber:02} - {title}.{suffix}".into(),
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
        if path.as_ref().exists() {
            let content = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)?;

            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Merges CLI arguments into the existing configuration.
    pub fn merge_cli(&mut self, cli: Cli) {
        if let Some(barcode) = cli.barcode {
            self.filter.barcode = Some(barcode);
        }
        if let Some(country) = cli.country {
            self.filter.country = Some(country);
        }
        if let Some(date) = cli.date {
            self.filter.date = Some(date);
        }
        if let Some(id) = cli.id {
            self.filter.id = Some(id);
        }

        if cli.skip {
            self.rip.skip = true;
        }

        if let Some(format) = cli.format {
            self.encode.format = format;
        }
    }
}
