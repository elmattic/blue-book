# blue-book

blue-book is a bit-perfect CD ripper and archival utility. It guarantees sector-perfect accuracy via **AccurateRip** verification, automating the pipeline from raw disc data to fully tagged, lossless FLAC or ALAC archives.

# Dependencies:

## riprip

```bash
brew install libcdio

export BINDGEN_EXTRA_CLANG_ARGS="-I/opt/homebrew/include"
export CFLAGS="-I/opt/homebrew/include"
export LDFLAGS="-L/opt/homebrew/lib"
export PKG_CONFIG_PATH="/opt/homebrew/opt/libcdio/lib/pkgconfig"

cargo install --git https://github.com/Blobfolio/riprip.git --bin riprip --tag v0.5.5
```

For Linux installation instructions, refer to the [Rip Rip Hooray!](https://github.com/Blobfolio/riprip?tab=readme-ov-file#installation) repository.

## ffmpeg

```bash
brew install ffmpeg
```

## Python Environment

This project uses **PEP 723** inline metadata. We recommend using [uv](https://github.com/astral-sh/uv) for a seamless experience—it will automatically handle Python versioning and dependencies without needing a virtualenv.

# Usage:

Insert a CD and run:

```bash
uv run blue_book.py
```

Feel free to experiment with different flags, but you’ll typically want to run a dry run first using `--skip`. This allows you to finalize your tags (fetched on [MusicBrainz](https://musicbrainz.org/)) before starting the rip.

Generally, you can provide the barcode and country of the release to improve accuracy. Once you're all set, simply re-run the command without the `--skip` flag.

```console
uv run blue_book.py -b 077774620727 -c GB -f alac
Scanning disc for CDTOC...
1 12 194284 182 16990 22032 35915 48827 64632 82890 93060 130592 146982 162240 188795

Field                | Value
------------------------------------------------------------
Release ID           | 38723ee2-aaac-4fdd-a566-3da5e5e2c575
Album Title          | A Night at the Opera
Artist               | Queen
Country              | GB
Date                 | 1987
Status               | Official
Quality              | normal
Barcode              | 077774620727
Format               | Jewel Case
Label                | EMI
Catalog#             | CDP 7 46207 2

Tracklist:
------------------------------------------------------------
 1. Death on Two Legs (Dedicated to…) (3:44)
 2. Lazing on a Sunday Afternoon (1:07)
 3. I’m in Love With My Car (3:05)
 4. You’re My Best Friend (2:52)
 5. ’39 (3:30)
 6. Sweet Lady (4:03)
 7. Seaside Rendezvous (2:15)
 8. The Prophet’s Song (8:20)
 9. Love of My Life (3:38)
10. Good Company (3:23)
11. Bohemian Rhapsody (5:54)
12. God Save the Queen (1:12)

Starting ripping process with 5 passes...
-----------------------
PIONEER DVD-RW DVR-215D
-----------------------

CDTOC:       C+B6+425E+5610+8C4B+BEBB+FC78+143CA+16B84+1FE20+23E26+279C0+2E17B+2F6CC
AccurateRip: 012-0012169f-00ae1095-960a1c0c
CDDB:        960a1c0c
CUETools:    ceboFUES5TLH6UEl_g0885cbYo0-
MusicBrainz: 4t8cmsDJx5c4Whz348aldu1EshA-

NO   FIRST    LAST  LENGTH          
----------------------------------------
00       0      31      32          HTOA
01      32   16839   16808              
02   16840   21881    5042              
03   21882   35764   13883              
04   35765   48676   12912              
05   48677   64481   15805              
06   64482   82739   18258              
07   82740   92909   10170              
08   92910  130441   37532              
09  130442  146831   16390              
10  146832  162089   15258              
11  162090  188644   26555              
12  188645  194101    5457              
AA  194102                      LEAD-OUT
----------------------------------------


Rip Rip…
  Tracks:       0..=12
  Read Offset:  48
  Cache Bust:   Disabled
  Verification: AccurateRip/CTDB cf. 3+
                C2 Error Pointers (Sample)
                Re-Read Consistency 2+
                Re-Read Contention 2×
                Subchannel Sync
  Rip Passes:   5
  Read Order:   Normal
  Verbose:      No
  Destination:  ./riprip/960a1c0c##.wav
…Hooray? [Y/n] 

Accurate: Track #01 has been successfully rescued.
Accurate: Track #02 has been successfully rescued.
Accurate: Track #03 has been successfully rescued.
Accurate: Track #04 has been successfully rescued.
Accurate: Track #05 has been successfully rescued.
Accurate: Track #06 has been successfully rescued.
Accurate: Track #07 has been successfully rescued.
Accurate: Track #08 has been successfully rescued.
Accurate: Track #09 has been successfully rescued.
Accurate: Track #10 has been successfully rescued.
Accurate: Track #12 has been successfully rescued.


Ripped: 13 tracks, 5 passes, in 10 minutes and 36.55 seconds.
Status: Recovery is (roughly) 99.995% – 99.996% complete.
        ########################################################################
        114,131,976       0            0            0
              3,567 + 1,390 + 15,628,199 + 98,498,820 samples


The fruits of your labor:
  /home/elmattic/Test/_riprip/960a1c0c.cue
  /home/elmattic/Test/_riprip/960a1c0c_00.wav            *         *
  /home/elmattic/Test/_riprip/960a1c0c_01.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_02.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_03.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_04.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_05.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_06.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_07.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_08.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_09.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_10.wav        99+99       999
  /home/elmattic/Test/_riprip/960a1c0c_11.wav                       
  /home/elmattic/Test/_riprip/960a1c0c_12.wav        99+99       999
                                                 AccurateRip  CUETools  (12/13)

* HTOA tracks cannot be verified w/ AccurateRip or CTDB,
  but this rip rates likely, which is the next best thing!

Converting 12 files using ALAC...
size=   23731KiB time=00:03:44.53 bitrate= 865.8kbits/s speed= 417x    
size=    6573KiB time=00:01:07.22 bitrate= 800.9kbits/s speed= 413x    
size=   21631KiB time=00:03:05.10 bitrate= 957.3kbits/s speed= 430x    
size=   18571KiB time=00:02:52.16 bitrate= 883.7kbits/s speed= 436x    
size=   23032KiB time=00:03:30.73 bitrate= 895.3kbits/s speed= 451x    
size=   27040KiB time=00:04:03.44 bitrate= 909.9kbits/s speed= 448x    
size=   14321KiB time=00:02:15.60 bitrate= 865.2kbits/s speed= 445x    
size=   50223KiB time=00:08:20.42 bitrate= 822.2kbits/s speed= 437x    
size=   18263KiB time=00:03:38.53 bitrate= 684.6kbits/s speed= 455x    
size=   19324KiB time=00:03:23.44 bitrate= 778.1kbits/s speed= 441x    
size=   34760KiB time=00:05:54.06 bitrate= 804.2kbits/s speed= 444x    
size=    6404KiB time=00:01:12.76 bitrate= 721.0kbits/s speed= 445x

Success! Files located in: /home/elmattic/.blue-book/Queen/A Night at the Opera
```

# Limitations & Future Work:

- **Reconstruction:** The current method of reconstructing albums via CUE sheets is rigid, particularly regarding how track indexes are merged. We should introduce user-controlled modes, such as:
  - An "as-is" mode that ignores hidden tracks.
  - An option to export a single FLAC file with an embedded or external CUE sheet.

- **Configuration:** To prevent "flag creep," the tool should support a TOML configuration file. This allows for organized settings across different sections, while ensuring CLI arguments still retain override priority.

- **Tagging:** If metadata is missing or ambiguous, the tool should provide a prompt for manual correction. Ideally, it could even suggest these corrections back to the MusicBrainz db.

- **Codec:** While the primary focus is on archival accuracy, the tool should eventually support lossy transcodes such as Ogg Vorbis, AAC, etc.

- **Multi-disc:** Currently lacks native handling for multi-medium sets.

- **Labels:** Lacks multi-label display.
