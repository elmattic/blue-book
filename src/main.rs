use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use heck::ToTitleCase;
use musicbrainz_rs::entity::discid::Discid;
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::entity::release_group::ReleaseGroup;
use musicbrainz_rs::prelude::*;
use serde::{Deserialize, Serialize};

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

async fn get_releases_by_discid(id: &str) -> anyhow::Result<Vec<Release>> {
    println!("{}", id);
    println!();

    let discid = Discid::fetch()
        .id(id)
        .with_artists()
        .with_artist_credits()
        .with_recordings()
        .with_labels()
        .with_release_groups()
        .execute_async()
        .await
        .with_context(|| format!("failed to fetch releases for discid {id}"))?;

    Ok(discid.releases.unwrap_or_default())
}

fn find_best_release(config: &Config, releases: Vec<Release>) -> Vec<Release> {
    if releases.is_empty() {
        return Vec::new();
    }

    let args = &config.filter;

    releases
        .into_iter()
        .filter(|r| {
            // barcode filter
            let barcode_ok = match &args.barcode {
                None => true,
                Some(search) => {
                    let barcode = r.barcode.as_deref().unwrap_or("");

                    if search.is_empty() {
                        barcode.is_empty()
                    } else {
                        barcode.contains(search)
                    }
                }
            };

            // country filter
            let country_ok = match &args.country {
                None => true,
                Some(c) => r.country.as_deref() == Some(c.as_str()),
            };

            // date filter
            let date_ok = match &args.date {
                None => true,
                Some(d) => {
                    let date = r.date.clone().map(|d| d.0).unwrap_or("".into());
                    date.contains(d)
                }
            };

            barcode_ok && country_ok && date_ok
        })
        .collect()
}

fn bold_substring(text: &str, sub: Option<&str>, verbose: bool) -> String {
    if !verbose {
        return text.to_string();
    }

    let Some(sub) = sub.filter(|s| !s.is_empty()) else {
        return text.to_string();
    };

    text.replace(sub, &format!("\x1b[1m{}\x1b[0m", sub))
}

fn get_artist_name(release: &Release) -> Option<String> {
    let credits = release.artist_credit.as_ref()?;

    Some(credits.iter().fold(String::new(), |mut acc, c| {
        acc.push_str(&c.artist.name);
        acc.push_str(c.joinphrase.as_deref().unwrap_or(""));
        acc
    }))
}

fn original_date(release: &Release) -> Option<String> {
    release
        .release_group
        .as_ref()?
        .first_release_date
        .as_ref()?
        .0
        .get(0..4)
        .map(String::from)
}

async fn get_genre(release: &Release) -> anyhow::Result<Option<String>> {
    let Some(rg_ref) = &release.release_group else {
        return Ok(None);
    };
    let rg_id = &rg_ref.id;

    let rg_data = ReleaseGroup::fetch()
        .id(rg_id)
        .with_tags()
        .execute_async()
        .await
        .with_context(|| format!("failed to fetch release group {rg_id}"))?;

    let genre = rg_data.tags.and_then(|mut tags| {
        tags.sort_by(|a, b| b.count.cmp(&a.count));
        tags.first().map(|t| t.name.to_title_case())
    });

    Ok(genre)
}

async fn print_release_table(config: &Config, releases: &[Release]) -> anyhow::Result<()> {
    let Some(release) = releases.last() else {
        return Ok(());
    };

    let args = &config.filter;

    let artist_name = get_artist_name(release).or_else(|| {
        release.artist_credit.as_ref().map(|credits| {
            credits
                .iter()
                .filter_map(|c| Some(c.artist.name.clone()))
                .collect::<Vec<_>>()
                .join("")
        })
    });

    let format = release
        .media
        .as_ref()
        .and_then(|m| m.first())
        .and_then(|medium| medium.format.clone());

    let packaging = release
        .packaging
        .as_ref()
        .map(|p| serde_plain::to_string(p).unwrap_or_default());

    const NA: &str = "N/A";

    let fields = vec![
        ("Release ID", release.id.clone()),
        ("Artist", artist_name.unwrap_or(NA.into())),
        ("Album", release.title.clone()),
        ("Date", original_date(release).unwrap_or(NA.into())),
        ("Genre", get_genre(release).await?.unwrap_or(NA.into())),
        (
            "Status",
            release
                .status
                .clone()
                .map(|s| format!("{:?}", s))
                .unwrap_or(NA.into()),
        ),
        (
            "Format",
            format!(
                "{} ({})",
                format.unwrap_or(NA.into()),
                packaging.unwrap_or(NA.into())
            ),
        ),
    ];

    println!("{:<20} | {}", "Field", "Value");
    println!("{}", "-".repeat(60));

    for (k, v) in fields {
        println!("{:<20} | {}", k, v);
    }

    Ok(())
}

async fn run(config: &Config) -> anyhow::Result<()> {
    let mbid = "dlsh_eduZC8L7ghh6El2uFAkC88-";
    let releases = get_releases_by_discid(mbid).await?;

    let releases = find_best_release(config, releases);

    print_release_table(config, &releases).await?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    let config = Config::load_from_file(PathBuf::from("config.toml"))?;

    dbg!(&config);

    unsafe {
        // SAFETY: called at program startup before initializing the Tokio runtime,
        // so no other threads can access or mutate the environment concurrently.
        std::env::set_var(
            "MUSICBRAINZ_USER_AGENT",
            "blue-book/0.1.0 (https://github.com/elmattic/blue-book)",
        );
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run(&config).await })
}
