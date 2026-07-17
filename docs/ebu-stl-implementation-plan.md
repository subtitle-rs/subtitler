# EBU STL Implementation Plan

## Overview
EBU STL is a professional broadcast-grade binary subtitle format used by European Broadcasting Union.

## Key Technical Details

### Binary Structure
- **GSI Block** (1024 bytes): General Subtitle Information
  - Code page number
  - Disk format code
  - Display standard code
  - Language code (ISO 639-2)
  - Program titles (original and translated)
  - Creation and revision dates
  - Timecode information
  - Total number of subtitles

- **TTI Blocks** (128 bytes each): Text and Timing Information
  - Subtitle group number
  - Subtitle number
  - Extension block number
  - Cumulative status
  - Start timecode (SMPTE)
  - End timecode (SMPTE)
  - Vertical position
  - Justification code
  - Text field (112 bytes)

### Timecode Format
- SMPTE timecode: HH:MM:SS:FF
- Frame rates: 25 fps (PAL) or 29.97 fps (NTSC)
- Frame-accurate timing

### Text Encoding
- Character code tables (0-15)
- Supports multiple character sets (Latin, Cyrillic, Greek)
- Control codes for formatting (italic, colors)
- Positioning information

## Implementation Status

### Completed
✅ Basic structure definition
✅ GSI and TTI block structs
✅ Timecode conversion functions
✅ Format detection
✅ Basic parsing framework
✅ Unit tests

### TODO
⚠️ Complete GSI block parsing (all 1024 bytes)
⚠️ Complete TTI block parsing (all 128 bytes)
⚠️ Full character code table support
⚠️ Text field decoding with control codes
⚠️ Binary serialization (to_bytes)
⚠️ Integration with core modules
⚠️ CLI support
⚠️ Example files

## Estimated Effort
- **Complexity**: High (binary format)
- **Time**: 15-20 hours for complete implementation
- **Testing**: Extensive testing with real EBU STL files needed

## References
- EBU Tech 3264-E specification
- https://tech.ebu.ch/docs/tech/tech3264.pdf

## Recommendation
EBU STL is a complex binary format that requires:
1. Deep understanding of binary parsing
2. Extensive character set handling
3. Precise timecode conversion
4. Complete GSI/TTI block implementation
5. Professional-grade error handling

Consider implementing in stages:
1. **Stage 1**: Basic parsing and detection (current)
2. **Stage 2**: Complete GSI/TTI parsing
3. **Stage 3**: Character set support
4. **Stage 4**: Full serialization
5. **Stage 5**: Integration and testing