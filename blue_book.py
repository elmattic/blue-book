# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "musicbrainzngs",
# ]
# ///

import argparse
import os
import pprint
import re
import subprocess
import sys
import tempfile
from enum import Enum
from pathlib import Path

import musicbrainzngs

__version__ = "0.1.0"

# Define the default: ~/.blue-book"
DEFAULT_OUTPUT = Path.home() / ".blue-book"

# A two-level hierarchy: Artist/Album/Tracknum - Title.Suffix
DIR_TEMPLATE = "{artist}/{album}"
FILE_TEMPLATE = "{tracknum:02d} - {title}.{suffix}"

# Identify our tool to MusicBrainz
musicbrainzngs.set_useragent(
    os.path.basename(__file__), __version__, "https://github.com/elmattic/blue-book"
)


class AudioFormat(Enum):
    # Value format: (ffmpeg_codec, file_extension)
    FLAC = ("flac", "flac")
    ALAC = ("alac", "m4a")

    @classmethod
    def from_str(cls, label):
        try:
            return cls[label.upper()]
        except KeyError:
            raise argparse.ArgumentTypeError(f"Invalid format: {label}")

    @property
    def codec(self):
        return self.value[0]

    @property
    def suffix(self):
        return self.value[1]


def extract_cdtoc() -> tuple[str, str, list[int]] | None:
    """Runs riprip --no-rip and parses the CDTOC from the output."""
    print("Scanning disc for CDTOC...")
    try:
        result = subprocess.run(
            ["riprip", "--no-rip"], capture_output=True, text=True, check=True
        )

        cdtoc_pat = r"CDTOC:.*?([0-9A-F]+(?:\+[0-9A-F]+)+)"
        cddb_pat = r"CDDB:.*?([0-9a-f]{8})"

        cdtoc = re.search(cdtoc_pat, result.stderr, re.IGNORECASE).group(1)
        cddb = re.search(cddb_pat, result.stderr, re.IGNORECASE).group(1)

        if cdtoc and cddb:
            pattern = re.compile(r"\d{2}\s+\d+\s+\d+\s+(\d+)")
            # Grabs the length from every match and converts to int
            lengths = [int(m.group(1)) for m in pattern.finditer(result.stderr)]

            return (cdtoc, cddb, lengths)

        print("Could not find CDTOC or CDDB in riprip output.")
        return None

    except FileNotFoundError:
        print("Error: 'riprip' utility not found. Please install it.")
        return None

    except subprocess.CalledProcessError as e:
        if e.stderr:
            print(e.stderr.strip())
        return None


def get_releases_by_toc(toc_string: str, lengths: list[int]) -> list | None:
    # Step 1: Split and convert hex to int
    parts = [int(x, 16) for x in toc_string.split("+")]

    # Step 2: Extract the key components
    num_tracks = parts[0]
    # We calculate the Audio Lead-out by adding the total audio length to the start offset.
    # This ignores 'Data Track' padding/pre-gaps that would otherwise cause a duration mismatch.
    track_offsets = parts[1 : num_tracks + 1]
    audio_lead_out = track_offsets[0] + sum(lengths)

    # Step 3: Format the TOC query
    # Format: "FirstTrack LastTrack AudioLeadOut Offset1 Offset2..."
    toc_query = f"1 {num_tracks} {audio_lead_out} " + " ".join(map(str, track_offsets))
    print(toc_query)
    print("")

    try:
        # Step 4: Search by TOC
        result = musicbrainzngs.get_releases_by_discid(
            id=None,
            toc=toc_query,
            includes=["artists", "artist-credits", "recordings", "labels"],
        )

        # Step 5: Extract the release list
        # If the query is successful, return the list (even if empty)
        if result and "release-list" in result:
            return result["release-list"]

        return []

    except Exception as e:
        print(f"Lookup failed: {e}")
        return None


# def get_releases_by_discid(disc_id: str) -> list | None:
#     print(f"Querying MusicBrainz by Disc ID: {disc_id}")

#     try:
#         result = musicbrainzngs.get_releases_by_discid(
#             id=disc_id, toc=None, includes=["artists", "artist-credits", "recordings"]
#         )

#         # Note: When searching by ID, MB usually returns 'disc' -> 'release-list'
#         if result and "disc" in result:
#             return result["disc"].get("release-list", [])

#         # Some versions of the API/Library return 'release-list' at the top level
#         if "release-list" in result:
#             return result["release-list"]

#         return []

#     except musicbrainzngs.ResponseError as e:
#         print(f"Disc ID not found or error: {e}")
#         return []
#     except Exception as e:
#         print(f"Lookup failed: {e}")
#         return None


def find_best_release(releases: list, args: argparse.Namespace) -> dict | None:
    if not releases:
        return None

    country = args.country.upper() if args.country else None

    filtered = [
        r
        for r in releases
        if (args.barcode is None or r.get("barcode") == args.barcode)
        and (country is None or r.get("country") == country)
    ]

    return filtered


def print_release_table(releases: list) -> None:
    release = releases[-1]

    # Get Artist (checking the phrase first, then the list)
    artist_name = release.get("artist-credit-phrase")
    if not artist_name and release.get("artist-credit"):
        # Fallback: Join names from the credit list
        artist_name = "".join(
            [c["artist"]["name"] for c in release["artist-credit"] if "artist" in c]
        )

    label_info = release.get("label-info-list", [{}])[0]

    # We'll create a list of fields we want to display
    fields = [
        ("Release ID", release.get("id")),
        ("Album Title", release.get("title")),
        ("Artist", artist_name or "N/A"),
        ("Country", release.get("country", "N/A")),
        ("Date", release.get("date", "N/A")),
        ("Status", release.get("status", "N/A")),
        ("Quality", release.get("quality", "N/A")),
        ("Barcode", release.get("barcode", "N/A")),
        ("Format", release.get("packaging", "N/A")),
        (
            "Label",
            label_info.get("label", {}).get("name", "N/A"),
        ),
        (
            "Catalog#",
            label_info.get("catalog-number", "N/A"),
        ),
    ]

    print(f"{'Field':<20} | {'Value'}")
    print("-" * 60)
    for label, value in fields:
        print(f"{label:<20} | {value}")


def print_tracks(releases: list) -> None:
    release = releases[-1]

    # The 'artist-credit-phrase' at the release level for comparison
    album_artist = release.get("artist-credit-phrase")

    print("\nTracklist:")
    print("-" * 60)

    # Loop through the media and the tracks within them
    for medium in release.get("medium-list", []):
        for track in medium.get("track-list", []):
            # 1. Basic Info
            num = track.get("number")
            title = track["recording"].get("title")

            # 2. Length (convert ms to MM:SS)
            length_ms = track.get("length") or track["recording"].get("length")
            duration = ""
            if length_ms:
                minutes, seconds = divmod(int(length_ms) // 1000, 60)
                duration = f"{minutes}:{seconds:02d}"

            # 3. Guest Artists
            track_artist = track.get("artist-credit-phrase")

            # Printing with conditional formatting
            track_line = f"{num:>2}. {title}"
            if duration:
                track_line += f" ({duration})"
            # Only print featuring if it adds new information
            if track_artist and track_artist != album_artist:
                track_line += f" - Featuring: {track_artist}"

            print(track_line)


def get_metadata(release: dict) -> dict:
    """
    Extracts high-level metadata and a list of tracks for tagging.
    """
    artist_name = release.get("artist-credit-phrase")
    album_title = release.get("title")
    release_date = release.get("date")
    year = release_date[:4] if release_date else None

    tracks = {}

    # Iterate through mediums (CD1, CD2, etc.)
    for medium in release.get("medium-list"):
        for track in medium.get("track-list"):
            # Fallback for track titles
            title = track.get("title") or track.get("recording", {}).get("title")

            track_artist = track.get("artist-credit-phrase")

            track_info = {
                "title": title,
                "artist": track_artist,
                "album": album_title,
                "date": year,
                "tracknumber": track.get("number"),
                # Genre?
                "albumartist": artist_name,
                # Additions
                "tracktotal": len(medium.get("track-list")),
                "discnumber": medium.get("position"),
                "disctotal": len(release.get("medium-list")),
            }
            tracks[int(track.get("number"))] = track_info

    return {
        "album_title": album_title,
        "artist": artist_name,
        "tracks": tracks,
    }


def sanitize(text: str) -> str:
    """Removes or replaces characters that are illegal in file systems."""
    if not text:
        return "Unknown"
    # Replace slashes with hyphens; remove other illegal characters
    clean = re.sub(r"[\\/]", "-", str(text))
    clean = re.sub(r'[<>:"|?*]', "", clean)
    return clean.strip()


def get_album_path(root: Path, meta: dict, template: str) -> Path:
    """Uses the main album metadata to create the directory."""
    context = {
        "artist": sanitize(meta.get("artist")),
        "album": sanitize(meta.get("album_title")),
        "date": sanitize(meta.get("date")),
    }
    return root.joinpath(template.format(**context))


def get_track_path(album_dir: Path, info: dict, suffix: str, template: str) -> Path:
    """Uses the existing track 'info' dict to create the filename."""
    context = {
        "tracknum": int(info.get("tracknumber")),
        "title": sanitize(info.get("title")),
        "artist": sanitize(info.get("artist")),
        "suffix": suffix,
    }
    return album_dir.joinpath(template.format(**context))


def parse_riprip_cue(cue_path: Path) -> dict:
    tracks = {}
    current_file = None
    current_track = None

    with open(cue_path, "r") as f:
        for line in f:
            line = line.strip()

            if line.startswith("FILE"):
                current_file = re.findall(r'"(.*?)"', line)[0]
            elif line.startswith("TRACK"):
                current_track = line.split()[1]
                tracks[current_track] = []
            elif line.startswith("INDEX") and current_track:
                index_num = line.split()[1]
                tracks[current_track].append({"index": index_num, "file": current_file})

    return tracks


def create_track(
    wav_files: list[Path], file_out: Path, track_info: dict, args: argparse.Namespace
):
    """
    Merges one or more WAVs into a single FLAC and applies tags.
    """
    if len(wav_files) > 1:
        # Create a temporary file for the concat instructions
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            for wav in wav_files:
                # Use absolute paths and escape single quotes for safety
                f.write(f"file '{wav.absolute()}'\n")
            concat_file = f.name

        # Use the 'concat' demuxer instead of the 'concat' protocol
        ffmpeg_input = ["-f", "concat", "-safe", "0", "-i", concat_file]
    else:
        ffmpeg_input = ["-i", str(wav_files[0])]

    cmd = ["ffmpeg", "-hide_banner"]
    if args.verbose:
        cmd += ["-loglevel", "info"]
    else:
        cmd += ["-loglevel", "error", "-stats"]
    cmd += ffmpeg_input

    # Codec-specific flags
    if args.format == AudioFormat.FLAC:
        cmd += ["-compression_level", "8"]

    cmd += [
        "-c:a",
        args.format.codec,
        "-metadata",
        f"title={track_info['title']}",
        "-metadata",
        f"artist={track_info['artist']}",
        "-metadata",
        f"album={track_info['album']}",
        "-metadata",
        f"date={track_info['date']}",
        "-metadata",
        f"track={track_info['tracknumber']}",
        "-metadata",
        f"totaltracks={track_info['tracktotal']}",
        str(file_out),
        "-y",
    ]

    try:
        subprocess.run(cmd, check=True)
    finally:
        # Clean up the temp file if we made one
        if len(wav_files) > 1:
            os.remove(concat_file)


def create_album(
    cue_path: Path, meta: dict, album_path: Path, args: argparse.Namespace
) -> None:

    data = parse_riprip_cue(cue_path)

    for trk, info in data.items():
        # Extract files and sort them by index (00, then 01)
        # This ensures the Pre-gap is prepended to the Audio
        sorted_segments = sorted(info, key=lambda x: x["index"])
        wav_paths = [Path("_riprip") / item["file"] for item in sorted_segments]

        info = meta.get("tracks")[int(trk)]
        file_out = get_track_path(album_path, info, args.format.suffix, FILE_TEMPLATE)

        create_track(wav_paths, file_out, info, args)


def rip_and_encode(
    release: dict, passes: int, cddb: str, args: argparse.Namespace
) -> None:
    if not args.skip:
        print(f"Starting ripping process with {passes} passes...")
        try:
            subprocess.run(
                ["riprip", "--passes", str(passes)], input="y\n", text=True, check=True
            )
        except subprocess.CalledProcessError as e:
            print(f"Error ripping disc: {e}")
            return

    meta = get_metadata(release)

    album_path = get_album_path(DEFAULT_OUTPUT, meta, DIR_TEMPLATE)
    album_path.mkdir(parents=True, exist_ok=True)

    cue_path = Path("_riprip") / f"{cddb}.cue"

    if not cue_path.is_file():
        print("No cue file found in _riprip.")
        return

    create_album(cue_path, meta, album_path, args)

    print(f"\nSuccess! Files located in: {album_path}")


def main():
    parser = argparse.ArgumentParser(
        description="Bit-perfect audio extraction and archival for CDs.",
    )
    parser.add_argument(
        "-V", "--version", action="version", version=f"%(prog)s {__version__}"
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true", help="show raw data for debugging"
    )
    parser.add_argument(
        "-b",
        "--barcode",
        type=str,
        help="filter release by barcode (e.g., 689230001720)",
    )
    parser.add_argument(
        "-c",
        "--country",
        type=str,
        help="filter release by country code (e.g., US, GB)",
    )
    parser.add_argument(
        "-s",
        "--skip",
        action="store_true",
        help="skip the ripping process",
    )
    parser.add_argument(
        "-f",
        "--format",
        type=AudioFormat.from_str,
        choices=list(AudioFormat),
        default=AudioFormat.FLAC,
        help="output audio format",
    )
    args = parser.parse_args()

    option = extract_cdtoc()
    if not option:
        sys.exit(1)
    cdtoc, cddb, lengths = option

    releases = get_releases_by_toc(cdtoc, lengths)

    if args.verbose:
        pprint.pprint(releases, indent=2, width=40)

    if releases:
        releases = find_best_release(releases, args)
        if len(releases) > 1:
            print(
                f"Warning: Found {len(releases)} matching releases, using the last one.\n"
            )
        if releases:
            print_release_table(releases)
            print_tracks(releases)
            print("")
        else:
            print("No releases matched your specific filters.")
            return

        rip_and_encode(releases[-1], 5, cddb, args)
    else:
        print("Error: No releases found for this TOC.")
        sys.exit(1)


if __name__ == "__main__":
    main()
