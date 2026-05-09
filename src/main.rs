use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use clap::{Parser, ValueEnum};
use heck::ToTitleCase;
use musicbrainz_rs::entity::artist_credit::ArtistCredit;
use musicbrainz_rs::entity::discid::Discid;
use musicbrainz_rs::entity::release::Media;
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::entity::release_group::ReleaseGroup;
use musicbrainz_rs::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const RIPRIP_PATH: &'static str = "_riprip";

const DEFAULT_FOLDER: &'static str = ".blue-book";

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
struct Config {
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
    fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }

    /// Merges CLI arguments into the existing configuration.
    fn merge_cli(&mut self, cli: Cli) {
        if let Some(barcode) = cli.barcode {
            self.filter.barcode = Some(barcode);
        }
        if let Some(country) = cli.country {
            self.filter.country = Some(country);
        }
        if let Some(date) = cli.date {
            self.filter.date = Some(date);
        }

        if cli.skip {
            self.rip.skip = true;
        }

        if let Some(format) = cli.format {
            self.encode.format = format;
        }
    }
}

#[derive(Debug)]
pub struct DiscInfo {
    pub cdtoc: String,
    pub cddb: String,
    pub discid: String,
    pub track_lengths: Vec<u32>,
}

/// Runs riprip --no-rip and parses the CDTOC from the output.
fn extract_cdtoc() -> anyhow::Result<DiscInfo> {
    println!("Scanning disc for MusicBrainz...");

    let output = Command::new("riprip").arg("--no-rip").output()?;

    let stderr_text = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        bail!("{}", stderr_text);
    }

    let re_cdtoc = Regex::new(r"CDTOC:.*?([0-9A-F]+(?:\+[0-9A-F]+)+)")?;
    let re_cddb = Regex::new(r"CDDB:.*?([0-9a-f]{8})")?;
    let re_discid = Regex::new(r"MusicBrainz:.*?([a-zA-Z0-9._-]{27,28})")?;
    let re_lengths = Regex::new(r"(?m)^\s*\d{2}\s+\d+\s+\d+\s+(\d+)")?;

    let extract = |re: &Regex, label: &str| {
        re.captures(&stderr_text)
            .map(|c| c[1].to_string())
            .with_context(|| format!("Could not find {label}"))
    };

    let cdtoc = extract(&re_cdtoc, "CDTOC")?;
    let cddb = extract(&re_cddb, "CDDB")?;
    let discid = extract(&re_discid, "MusicBrainz")?;

    let track_lengths: Vec<u32> = re_lengths
        .captures_iter(&stderr_text)
        .filter_map(|cap| cap[1].parse().ok())
        .collect();

    if track_lengths.is_empty() {
        bail!("No track lengths found in riprip output.");
    }

    Ok(DiscInfo {
        cdtoc,
        cddb,
        discid,
        track_lengths,
    })
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

fn find_best_release(releases: Vec<Release>, config: &Config) -> Vec<Release> {
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
                        barcode.contains(&search.replace("-", "").replace(" ", ""))
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

async fn print_release_table(releases: &[Release], config: &Config) -> anyhow::Result<()> {
    let Some(release) = releases.last() else {
        return Ok(());
    };

    let args = &config.filter;

    let artist_name = artist_credit_phrase(&release.artist_credit);

    let format = release
        .media
        .as_ref()
        .and_then(|m| m.first())
        .and_then(|medium| medium.format.clone());

    let packaging = release
        .packaging
        .as_ref()
        .map(|p| serde_plain::to_string(p).unwrap_or_default());

    let label_info = release.label_info.as_ref().and_then(|list| list.first());

    let label_name = label_info
        .and_then(|info| info.label.as_ref())
        .map(|label| label.name.clone());

    let catalog_number = label_info.and_then(|info| info.catalog_number.clone());

    const NA: &str = "N/A";

    let fields = vec![
        ("Release ID", Some(release.id.clone())),
        ("Artist", artist_name),
        ("Album", Some(release.title.clone())),
        ("Date", original_date(release)),
        ("Genre", get_genre(release).await?),
        ("Status", release.status.clone().map(|s| format!("{:?}", s))),
        (
            "Format",
            Some(format!(
                "{} ({})",
                format.unwrap_or(NA.into()),
                packaging.unwrap_or(NA.into())
            )),
        ),
        ("Label", label_name),
        ("Catalog#", catalog_number),
        ("Barcode", release.barcode.as_ref().cloned()),
        ("Country", release.country.as_ref().cloned()),
        ("Released", release.date.as_ref().map(|ds| ds.0.clone())),
    ];

    println!("{:<20} | {}", "Field", "Value");
    println!("{}", "-".repeat(60));
    for (f, v) in fields {
        println!("{:<20} | {}", f, v.as_deref().unwrap_or(NA));
    }

    Ok(())
}

fn has_disc_id(media: &Media, disc_id: &str) -> bool {
    media
        .discs
        .as_ref()
        .map(|discs| discs.iter().any(|d| d.id == disc_id))
        .unwrap_or(false)
}

fn artist_credit_phrase(artist_credits: &Option<Vec<ArtistCredit>>) -> Option<String> {
    artist_credits.as_ref().map(|credits| {
        credits
            .iter()
            .map(|ac| {
                // Combine the artist name with their specific join phrase
                let name = &ac.artist.name;
                let join = ac.joinphrase.as_deref().unwrap_or("");
                format!("{}{}", name, join)
            })
            .collect::<String>()
    })
}

fn print_tracks(releases: &[Release], discid: &str) -> anyhow::Result<()> {
    let release = releases.last().context("no releases found")?;

    // The 'artist-credit-phrase' at the release level for comparison
    let album_artist = &release.artist_credit;

    println!("\nTracklist:");
    println!("{}", "-".repeat(60));

    let media = release.media.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);

    // Loop through the media and the tracks within them
    for medium in media {
        if !has_disc_id(medium, discid) {
            continue;
        }

        let tracks = medium.tracks.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);

        for track in tracks {
            // 1. Basic Info
            let num = &track.number;
            let recording = track.recording.as_ref().context("no recording")?;
            let title = &recording.title;

            // 2. Length (convert ms to MM:SS)
            let length_ms = track.length.or(recording.length);
            let duration = length_ms
                .map(|ms| {
                    let total_seconds = ms / 1000;
                    let (minutes, seconds) = (total_seconds / 60, total_seconds % 60);
                    format!("{minutes}:{seconds:02}")
                })
                .unwrap_or_default();

            // 3. Guest Artists
            let track_artist = &track.artist_credit;

            // Printing with conditional formatting
            let mut track_line = format!("{num:>2}. {title}");
            // Only print featuring if it adds new information
            if track_artist != album_artist {
                if let Some(track_artist) = artist_credit_phrase(&track_artist) {
                    track_line.push_str(&format!(" - {track_artist}"));
                }
            }
            if !duration.is_empty() {
                track_line.push_str(&format!(" ({duration})"));
            }

            println!("{}", track_line);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum MetaData {
    Map(BTreeMap<String, MetaData>),
    Value(String),
}

impl MetaData {
    pub fn new_map() -> Self {
        MetaData::Map(BTreeMap::new())
    }

    pub fn with_value(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let MetaData::Map(ref mut map) = self {
            map.insert(key.into(), MetaData::Value(value.into()));
        }
        self
    }

    pub fn with_map(mut self, key: impl Into<String>, other: BTreeMap<String, MetaData>) -> Self {
        if let MetaData::Map(ref mut map) = self {
            map.insert(key.into(), MetaData::Map(other));
        }
        self
    }

    pub fn get_value(&self, key: &str) -> Option<&String> {
        if let MetaData::Map(map) = self {
            if let Some(MetaData::Value(value)) = map.get(key) {
                return Some(value);
            }
        }
        None
    }

    pub fn get_map(&self, key: &str) -> Option<&MetaData> {
        if let MetaData::Map(map) = self {
            let entry = map.get(key);
            // Ensure the entry we found is actually a Map variant
            if let Some(MetaData::Map(_)) = entry {
                return entry;
            }
        }
        None
    }

    pub fn hidden_track(track: &MetaData) -> Option<Self> {
        let MetaData::Map(map) = track else {
            return None;
        };

        let mut map_copy = map.clone();

        map_copy.insert("title".to_string(), MetaData::Value("[hidden]".to_string()));

        let new_num_str = map_copy
            .get("tracknumber")
            .and_then(|meta| {
                if let MetaData::Value(s) = meta {
                    Some(s)
                } else {
                    None
                }
            })
            .and_then(|s| s.parse::<i32>().ok())
            .map(|num| (num - 1).to_string())?;

        map_copy.insert("tracknumber".to_string(), MetaData::Value(new_num_str));

        Some(MetaData::Map(map_copy))
    }
}

/// Extracts high-level metadata and a list of tracks for tagging.
async fn get_metadata(release: &Release, discid: &str) -> anyhow::Result<MetaData> {
    let album_title = release.title.clone();
    let album_artist = artist_credit_phrase(&release.artist_credit).context("no artist credit")?;
    let genre = get_genre(release).await?.context("no genre")?;
    let year = original_date(release).context("no original date")?;

    let mut tracks = BTreeMap::new();
    let media = release.media.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    for medium in media {
        if !has_disc_id(medium, discid) {
            continue;
        }

        let track_list = medium.tracks.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
        let track_total = track_list.len().to_string();
        let disc_number = medium.position.unwrap_or(1).to_string();
        let disc_total = media.len().to_string();

        for track in track_list {
            let track_artist =
                artist_credit_phrase(&track.artist_credit).context("no artist credit")?;

            let track_meta = MetaData::new_map()
                .with_value("title", track.title.clone())
                .with_value("album", album_title.clone())
                .with_value("artist", track_artist)
                .with_value("date", year.clone())
                .with_value("genre", genre.clone())
                .with_value("tracknumber", track.number.clone())
                .with_value("albumartist", album_artist.clone())
                // Additions
                .with_value("tracktotal", track_total.clone())
                .with_value("discnumber", disc_number.clone())
                .with_value("disctotal", disc_total.clone());

            tracks.insert(track.number.clone(), track_meta);
        }
    }

    Ok(MetaData::new_map()
        .with_value("albumtitle", album_title)
        .with_value("artist", album_artist)
        .with_map("tracks", tracks))
}

/// Removes or replaces characters that are illegal in file systems.
fn sanitize(text: &str) -> String {
    let clean = text.replace(['\\', '/'], "-");

    let mut result = clean;
    result.retain(|c| !r#"<>:"|?*"#.contains(c));

    result.trim().to_string()
}

/// Uses the album metadata to create the directory.
fn get_album_path(root: &Path, meta: &MetaData, template: &str) -> PathBuf {
    let mut template = template.to_string();

    let replacements = [
        ("{artist}", "artist"),
        ("{album}", "album_title"),
        ("{date}", "date"),
    ];

    for (placeholder, meta_key) in replacements {
        // If the key is missing or isn't a Value, we default to an empty string.
        let value = meta.get_value(meta_key).cloned().unwrap_or_default();
        template = template.replace(placeholder, &sanitize(&value));
    }

    root.join(template)
}

/// Uses the track metadata to create the filename.
fn get_track_path(
    album_dir: &Path,
    track_meta: &MetaData,
    format: &AudioFormat,
    template: &str,
) -> PathBuf {
    let mut template = template.to_string();

    let replacements = [
        ("{discnumber}", "discnumber"),
        ("{disctotal}", "disctotal"),
        ("{tracknumber}", "tracknumber"),
        ("{title}", "title"),
        ("{artist}", "artist"),
        ("{albumartist}", "albumartist"),
        ("{suffix}", format.suffix()),
    ];

    for (placeholder, meta_key) in replacements {
        let val = track_meta
            .get_value(meta_key)
            .map(|s| s.as_str())
            .unwrap_or_default();
        template = template.replace(placeholder, &sanitize(val));
    }

    album_dir.join(template)
}

#[derive(Debug)]
struct CueIndex {
    pub index: String,
    pub file: String,
}

fn parse_riprip_cue(cue_path: &Path) -> anyhow::Result<HashMap<String, Vec<CueIndex>>> {
    let mut tracks: HashMap<String, Vec<CueIndex>> = HashMap::new();
    let mut current_file: Option<String> = None;
    let mut current_track: Option<String> = None;

    let file_re = Regex::new(r#""([^"]+)""#)?;

    let file = File::open(cue_path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?.trim().to_string();

        if line.starts_with("FILE") {
            if let Some(caps) = file_re.captures(&line) {
                current_file = Some(caps[1].to_string());
            }
        } else if line.starts_with("TRACK") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let track_num = parts[1].to_string();
                current_track = Some(track_num.clone());
                tracks.insert(track_num, Vec::new());
            }
        } else if line.starts_with("INDEX") {
            if let (Some(track), Some(file_name)) = (&current_track, &current_file) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Some(indices) = tracks.get_mut(track) {
                        indices.push(CueIndex {
                            index: parts[1].to_string(),
                            file: file_name.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(tracks)
}

/// Merges one or more WAVs into a single file and applies tags.
fn create_track(
    wav_files: &[PathBuf],
    file_out: &Path,
    track_info: &MetaData,
    config: &Config,
    verbose: bool,
) -> anyhow::Result<()> {
    let encode = &config.encode;

    let (_temp_path, ffmpeg_input) = if wav_files.len() > 1 {
        // Create a temporary file for the concat instructions
        let mut tmp =
            NamedTempFile::with_suffix(".txt").context("Failed to create temporary file")?;

        for wav in wav_files {
            // Use absolute paths and escape single quotes for safety
            writeln!(tmp, "file '{}'", wav.canonicalize().unwrap().display())?;
        }

        // Use the 'concat' demuxer instead of the 'concat' protocol
        let ffmpeg_input = vec![
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            tmp.path().to_string_lossy().to_string(),
        ];

        (Some(tmp.into_temp_path()), ffmpeg_input)
    } else if let Some(first) = wav_files.first() {
        let ffmpeg_input = vec!["-i".into(), first.to_string_lossy().to_string()];

        (None, ffmpeg_input)
    } else {
        bail!("No input files provided");
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner");
    if verbose {
        cmd.args(["-loglevel", "info"]);
    } else {
        cmd.args(["-loglevel", "error", "-stats"]);
    }
    cmd.args(&ffmpeg_input);

    // Codec-specific flags
    if let AudioFormat::Flac = encode.format {
        cmd.args([
            "-compression_level",
            &config.flac.compression_level.to_string(),
        ]);
    }

    let mapping = [
        ("album", "album"),
        ("artist", "artist"),
        ("date", "date"),
        ("genre", "genre"),
        ("title", "title"),
        ("track", "tracknumber"),
        ("totaltracks", "tracktotal"),
        ("disc", "discnumber"),
        ("disctotal", "disctotal"),
    ];
    for (key, meta_key) in mapping {
        if let Some(value) = track_info.get_value(meta_key) {
            cmd.args(["-metadata", &format!("{}={}", key, value)]);
        }
    }

    cmd.arg(file_out).arg("-y");

    let status = cmd
        .status()
        .context("Error: 'ffmpeg' utility not found. Please install it.")?;

    if !status.success() {
        bail!("FFmpeg failed with exit code: {}", status);
    }

    Ok(())
}

fn create_album(
    cue_path: &Path,
    meta: &MetaData,
    album_path: &Path,
    config: &Config,
    verbose: bool,
) -> anyhow::Result<()> {
    let encode = &config.encode;
    let template = &config.template;

    let data = parse_riprip_cue(cue_path)?;

    println!(
        "Creating {} tracks using {}...",
        data.len(),
        encode.format.codec()
    );
    for (trk, info) in data {
        // Extract files and sort them by index (00, then 01)
        // This ensures the Pre-gap is prepended to the Audio
        let mut sorted_segments = info;
        sorted_segments.sort_by_key(|item| item.index.clone());

        let wav_paths: Vec<PathBuf> = sorted_segments
            .iter()
            .map(|item| PathBuf::from(RIPRIP_PATH).join(&item.file))
            .collect();

        let tracks_map = meta.get_map("tracks").unwrap();

        if wav_paths.len() == 2 {
            let track_meta = tracks_map.get_map("1").unwrap();

            let file_out = get_track_path(album_path, track_meta, &encode.format, &template.file);
            create_track(
                &[wav_paths[1].clone()],
                &file_out,
                track_meta,
                config,
                verbose,
            )?;

            let track_meta_copy = &MetaData::hidden_track(track_meta).unwrap();

            let file_out =
                get_track_path(album_path, track_meta_copy, &encode.format, &template.file);
            create_track(
                &[wav_paths[0].clone()],
                &file_out,
                track_meta_copy,
                config,
                verbose,
            )?;
        } else {
            let track_meta = tracks_map.get_map(&trk.trim_start_matches('0')).unwrap();
            let file_out = get_track_path(album_path, track_meta, &encode.format, &template.file);

            create_track(&wav_paths, &file_out, track_meta, config, verbose)?;
        }
    }

    Ok(())
}

async fn rip_and_encode(
    release: &Release,
    cddb: &str,
    discid: &str,
    config: &Config,
    verbose: bool,
) -> anyhow::Result<()> {
    let passes = config.rip.passes;
    let device = &config.rip.device;
    let template = &config.template;

    if !config.rip.skip {
        println!("Starting ripping process with {passes} passes...");

        let mut cmd = Command::new("riprip");
        cmd.arg("--passes").arg(passes.to_string());

        if let Some(device) = device {
            if device.exists() {
                cmd.arg("--dev").arg(device);
            }
        }

        // Set up pipes to send "y\n" to stdin
        let mut child = cmd.stdin(Stdio::piped()).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(b"y\n")?;
        }

        let status = child.wait()?;
        if !status.success() {
            eprintln!("Error ripping disc: process exited with {}", status);
            return Ok(());
        }
    }

    let meta = get_metadata(release, discid).await?;

    let default_output = home::home_dir()
        .map(|p| p.join(DEFAULT_FOLDER))
        .context("failed to get home dir")?;

    let album_path = get_album_path(&default_output, &meta, &template.dir);
    fs::create_dir_all(&album_path)?;

    let cue_path = PathBuf::from(RIPRIP_PATH).join(format!("{cddb}.cue"));

    if !cue_path.is_file() {
        println!("No cue file found in _riprip.");
        return Ok(());
    }

    create_album(&cue_path, &meta, &album_path, config, verbose)?;

    // if config.flac.cue_sheet {
    //     if let Some(tracks) = meta.get("tracks") {
    //         create_cue_sheet(&cue_path, &album_path, tracks, config)?;
    //     }
    // }

    println!("\nSuccess! Files located in: {}", album_path.display());

    Ok(())
}

async fn run(config: &Config, verbose: bool) -> anyhow::Result<()> {
    let info = extract_cdtoc()?;

    let releases = get_releases_by_discid(&info.discid).await?;

    if !releases.is_empty() {
        let releases = find_best_release(releases, config);
        if releases.len() > 1 {
            println!(
                "Warning: Found {} matching releases, using the last one.\n",
                releases.len()
            )
        }
        if !releases.is_empty() {
            print_release_table(&releases, config).await?;
            print_tracks(&releases, &info.discid)?;
            println!("");
        } else {
            println!("No releases matched your specific filters.");
            return Ok(());
        }

        rip_and_encode(
            releases.last().unwrap(),
            &info.cddb,
            &info.discid,
            config,
            verbose,
        )
        .await?;
    } else {
        bail!("Error: No releases found for this TOC.")
    }

    Ok(())
}

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
struct Cli {
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

fn main() -> anyhow::Result<()> {
    let mut config = Config::load_from_file(PathBuf::from("config.toml"))?;
    let cli = Cli::parse();
    let verbose = cli.verbose;
    let toc = cli.toc.clone();
    config.merge_cli(cli);

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
    if let Err(e) = rt.block_on(async { run(&config, verbose).await }) {
        eprintln!("{:#}", e);
        std::process::exit(1);
    }
    Ok(())
}
