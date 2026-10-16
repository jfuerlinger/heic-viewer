# Test fixtures

Generated on macOS with Apple ImageIO via `tools/gen-fixtures.swift`
(`swiftc -O tools/gen-fixtures.swift -o /tmp/gen && /tmp/gen testdata`). Released under the
project's MIT license.

Quadrant pattern (unrotated): top-left red, top-right green, bottom-left blue, bottom-right white.

| File | Content |
|---|---|
| `quadrants_srgb.heic` | 96x64, 8 bit, nclx (unspecified → sRGB) |
| `quadrants_orientation6.heic` | same pixels, `irot` = rotate 90° clockwise → displayed 64x96 |
| `p3_color.heic` | 64x64 flat Display-P3 color (0.8, 0.2, 0.2), ICC profile |
| `alpha.heic` | 64x64, left opaque red, right blue with 50 % alpha |
| `with_thumbnail.heic` | 1024x768 with embedded 320x240 thumbnail |
| `with_thumbnail_orientation6.heic` | same, rotated (thumbnail also carries the rotation) |
| `tenbit_p3.heic` | 64x48, 10 bit HEVC Main10, Display-P3 ICC |
| `corrupt_truncated.heic` | first 900 bytes of a valid file: header parses, image data missing |
