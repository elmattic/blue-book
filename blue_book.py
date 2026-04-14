# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "musicbrainzngs",
#     "mutagen",
# ]
# ///

import argparse
import os
import pprint
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import musicbrainzngs
from mutagen.flac import FLAC

__version__ = "0.1.0"

# Define the default: ~/.blue-book"
DEFAULT_OUTPUT = Path.home() / ".blue-book"

# Identify our tool to MusicBrainz
musicbrainzngs.set_useragent(
    os.path.basename(__file__), __version__, "https://github.com/elmattic/blue-book"
)


def extract_cdtoc() -> str | None:
    """Runs riprip --no-rip and parses the CDTOC from the output."""
    print("Scanning disc for CDTOC...")
    try:
        result = subprocess.run(
            ["riprip", "--no-rip"], capture_output=True, text=True, check=True
        )

        pattern = r"([0-9A-F]+(?:\+[0-9A-F]+)+)"
        match = re.search(pattern, result.stderr, re.IGNORECASE)

        if match:
            cdtoc = match.group(1)
            return cdtoc

        print("Could not find CDTOC in riprip output.")
        return None

    except FileNotFoundError:
        print("Error: 'riprip' utility not found. Please install it.")
        return None

    except subprocess.CalledProcessError as e:
        print(f"Error scanning disc: {e}")
        return None


def get_releases_by_toc(toc_string) -> list | None:
    # Step 1: Split and convert hex to int
    parts = [int(x, 16) for x in toc_string.split("+")]

    # Step 2: Extract the key components
    last_track = parts[0]
    lead_out = parts[-1]
    track_offsets = parts[1:-1]

    # Step 3: Format the TOC query
    # Format: "FirstTrack LastTrack LeadOut Offset1 Offset2..."
    toc_query = f"1 {last_track} {lead_out} " + " ".join(map(str, track_offsets))
    print(toc_query)
    print("")

    try:
        # Step 4: Search by TOC
        result = musicbrainzngs.get_releases_by_discid(
            id=None, toc=toc_query, includes=["artists", "artist-credits", "recordings"]
        )

        # Step 5: Extract the release list
        # If the query is successful, return the list (even if empty)
        if result and "release-list" in result:
            return result["release-list"]

        return []

    except Exception as e:
        print(f"Lookup failed: {e}")
        return None


def find_best_release(releases: list, args: argparse.Namespace) -> dict | None:
    if not releases:
        return None

    filtered = [
        r
        for r in releases
        if (not args.barcode or r.get("barcode") == args.barcode)
        and (not args.country or r.get("country", "").upper() == args.country.upper())
    ]

    return filtered if filtered else releases


def print_release_table(releases: list) -> None:
    release = releases[-1]

    # Get Artist (checking the phrase first, then the list)
    artist_name = release.get("artist-credit-phrase")
    if not artist_name and release.get("artist-credit"):
        # Fallback: Join names from the credit list
        artist_name = "".join(
            [c["artist"]["name"] for c in release["artist-credit"] if "artist" in c]
        )

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
            tracks[track.get("number")] = track_info

    return {
        "album_title": album_title,
        "artist": artist_name,
        "tracks": tracks,
    }


def process_track(wav_path: str, flac_path: str, track_info: dict) -> None:
    """Converts a single WAV to FLAC and applies tags."""
    subprocess.run(
        ["ffmpeg", "-i", wav_path, "-compression_level", "8", flac_path, "-y"],
        capture_output=True,
        check=True,
    )

    # Tagging
    audio = FLAC(flac_path)
    audio["title"] = track_info["title"]
    audio["artist"] = track_info["artist"]
    audio["album"] = track_info["album"]
    audio["date"] = track_info["date"]
    audio["tracknumber"] = str(track_info["tracknumber"])
    audio["tracktotal"] = str(track_info["tracktotal"])
    audio.save()


def rip_and_encode(release: dict, passes: int = 10) -> None:
    meta = get_metadata(release)

    print(f"Starting riprip with {passes} passes...")
    try:
        subprocess.run(["riprip", "--passes", str(passes)], check=True)
    except subprocess.CalledProcessError as e:
        print(f"Error ripping disc: {e}")
        return

    output_path = DEFAULT_OUTPUT.joinpath(
        f"{meta.get('artist')} - {meta.get('album_title')}"
    )
    print(output_path)

    output_path.mkdir(parents=True, exist_ok=True)

    riprip_dir = Path("_riprip")

    wav_files = sorted(list(riprip_dir.glob("*.wav")))

    if not wav_files:
        print("No WAV files found in _riprip.")
        return

    print(f"Encoding {len(wav_files)} tracks to FLAC...")

    for i, wav_path in enumerate(wav_files):
        flac_path = output_path / f"{wav_path.stem}.flac"

        try:
            subprocess.run(
                [
                    "ffmpeg",
                    "-i",
                    str(wav_path),
                    "-compression_level",
                    "8",
                    str(flac_path),
                    "-y",
                ],
                capture_output=True,
                check=True,
            )
        except subprocess.CalledProcessError:
            print(f"FFmpeg failed on {wav_path.name}")
            continue

        wav_path.unlink()

    print(f"\nSuccess! Files located in: {output_path}")


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
    parser.add_argument("--toc", type=str, help="manually provide a TOC string")
    parser.add_argument(
        "-b",
        "--barcode",
        type=str,
        help="Filter release by barcode (e.g., 689230001720)",
    )
    parser.add_argument(
        "-c",
        "--country",
        type=str,
        help="Filter release by country code (e.g., US, GB)",
    )
    args = parser.parse_args()

    cdtoc = args.toc or extract_cdtoc()
    if not cdtoc:
        sys.exit(1)

    releases = get_releases_by_toc(cdtoc)

    if args.verbose:
        pprint.pprint(releases, indent=2, width=40)

    if releases:
        releases = find_best_release(releases, args)
        if len(releases) > 1:
            print(
                f"Warning: Found {len(releases)} matching releases, using the last one.\n"
            )
        print_release_table(releases)
        print_tracks(releases)
        print("")

        rip_and_encode(releases[-1], DEFAULT_OUTPUT)
    else:
        print("Error: No releases found for this TOC.")
        sys.exit(1)


if __name__ == "__main__":
    main()
