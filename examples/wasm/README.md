# subtitler WASM Demo

Run subtitler entirely in the browser — parse, convert, validate, and normalize
subtitles without a server.

## Build

```bash
# Install wasm-pack (one-time)
cargo install wasm-pack

# Build the WASM package (from the project root)
wasm-pack build --target web --no-default-features \
  --features "srt,vtt,ass,ssa,ttml,sbv,lrc,sami,mpl2,scc,ebu_stl,microdvd,subviewer,wasm"

# Copy output to the example directory
cp -r pkg/* examples/wasm/

# Serve locally
cd examples/wasm
python3 -m http.server 8000
# or: npx serve .
```

Open `http://localhost:8000` in your browser.

## What it does

- **Parse**: Auto-detect subtitle format and display content with metadata
- **Convert**: Convert between any supported format (SRT→VTT, ASS→SRT, etc.)
- **Validate**: Check for overlaps, negative durations, CPS issues
- **Normalize**: Strip HTML/ASS tags from subtitle text
- **Info**: Show format, subtitle count, total duration

Drag-and-drop or paste subtitle content. All processing happens locally in
the browser — no data is sent to any server.

## Supported formats

SRT, VTT, ASS, SSA, MicroDVD, SubViewer, TTML/IMSC, SBV, LRC, SAMI, MPL2, SCC, EBU STL

## File size

The WASM binary is ~600KB (uncompressed) and ~250KB (gzipped). 

## Adding more features

Build with additional features:

```bash
wasm-pack build --target web --no-default-features \
  --features "srt,vtt,ass,wasm,http"
```

Note: `http` and `io` features are not available on WASM (they require `reqwest`/`tokio`).
Use browser APIs (Fetch, FileReader) to load content, then pass it to the WASM functions.
