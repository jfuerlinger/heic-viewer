// Usage: swiftc -O tools/gen-fixtures.swift -o /tmp/gen-fixtures && /tmp/gen-fixtures testdata
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

// Generates small HEIC fixtures. Quadrants: TL red, TR green, BL blue, BR white.
let outDir = CommandLine.arguments[1]

func makeImage(w: Int, h: Int, space: CGColorSpace, bpc: Int, alpha: Bool,
               fill: (Int, Int) -> (Double, Double, Double, Double)) -> CGImage {
    let comps = 4
    let bytesPerComp = bpc / 8
    let rowBytes = w * comps * bytesPerComp
    var info: UInt32 = alpha ? CGImageAlphaInfo.premultipliedLast.rawValue : CGImageAlphaInfo.noneSkipLast.rawValue
    if bpc == 16 { info |= CGBitmapInfo.byteOrder16Little.rawValue }
    let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: bpc, bytesPerRow: rowBytes,
                        space: space, bitmapInfo: info)!
    let buf = ctx.data!
    for y in 0..<h {
        for x in 0..<w {
            let (r, g, b, a) = fill(x, y)
            let vals = [r * a, g * a, b * a, a]
            for c in 0..<4 {
                let off = y * rowBytes + (x * 4 + c) * bytesPerComp
                if bpc == 8 {
                    buf.storeBytes(of: UInt8((vals[c] * 255).rounded()), toByteOffset: off, as: UInt8.self)
                } else {
                    buf.storeBytes(of: UInt16((vals[c] * 65535).rounded()).littleEndian, toByteOffset: off, as: UInt16.self)
                }
            }
        }
    }
    return ctx.makeImage()!
}

func quadrants(w: Int, h: Int) -> (Int, Int) -> (Double, Double, Double, Double) {
    return { x, y in
        let left = x < w / 2, top = y < h / 2
        switch (top, left) {
        case (true, true): return (1, 0, 0, 1)
        case (true, false): return (0, 1, 0, 1)
        case (false, true): return (0, 0, 1, 1)
        default: return (1, 1, 1, 1)
        }
    }
}

func write(_ name: String, _ img: CGImage, _ props: [CFString: Any]) {
    let url = URL(fileURLWithPath: outDir).appendingPathComponent(name) as CFURL
    let dest = CGImageDestinationCreateWithURL(url, UTType.heic.identifier as CFString, 1, nil)!
    var p = props
    p[kCGImageDestinationLossyCompressionQuality] = 0.95
    CGImageDestinationAddImage(dest, img, p as CFDictionary)
    if !CGImageDestinationFinalize(dest) { print("FAILED: \(name)") } else { print("wrote \(name)") }
}

let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
let p3 = CGColorSpace(name: CGColorSpace.displayP3)!

write("quadrants_srgb.heic", makeImage(w: 96, h: 64, space: srgb, bpc: 8, alpha: false, fill: quadrants(w: 96, h: 64)), [:])
write("quadrants_orientation6.heic", makeImage(w: 96, h: 64, space: srgb, bpc: 8, alpha: false, fill: quadrants(w: 96, h: 64)),
      [kCGImagePropertyOrientation: 6])
write("p3_color.heic", makeImage(w: 64, h: 64, space: p3, bpc: 8, alpha: false, fill: { _, _ in (0.8, 0.2, 0.2, 1) }), [:])
write("alpha.heic", makeImage(w: 64, h: 64, space: srgb, bpc: 8, alpha: true, fill: { x, _ in x < 32 ? (1, 0, 0, 1) : (0, 0, 1, 0.5) }), [:])
write("with_thumbnail.heic", makeImage(w: 1024, h: 768, space: srgb, bpc: 8, alpha: false, fill: quadrants(w: 1024, h: 768)),
      [kCGImageDestinationEmbedThumbnail: true, kCGImageDestinationImageMaxPixelSize: 1024])
write("tenbit_p3.heic", makeImage(w: 64, h: 48, space: p3, bpc: 16, alpha: false, fill: quadrants(w: 64, h: 48)),
      [kCGImagePropertyDepth: 10])
write("with_thumbnail_orientation6.heic", makeImage(w: 1024, h: 768, space: srgb, bpc: 8, alpha: false, fill: quadrants(w: 1024, h: 768)),
      [kCGImageDestinationEmbedThumbnail: true, kCGImagePropertyOrientation: 6])
