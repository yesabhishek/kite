// Rebuild Kite's vector identity and desktop icon containers on macOS.
// Usage: swift scripts/make-icon.swift [output-directory]
// Artwork is defined here as flat vector paths. No downloaded assets are needed.
import AppKit
import CoreText
import Foundation

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let output = CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
    : root.appendingPathComponent("assets/package", isDirectory: true)
let brand = root.appendingPathComponent("assets/brand", isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
try FileManager.default.createDirectory(at: brand, withIntermediateDirectories: true)

let paper = "#F4F2EC"
let ink = "#232323"
let orange = "#DE4B2C"
let muted = "#706D66"
let rule = "#D8D5CD"

struct Shape {
    let path: CGPath
    let fill: String?
    let stroke: String?
    let lineWidth: CGFloat
}
struct Drawing {
    let width: Int
    let height: Int
    var shapes: [Shape] = []

    mutating func path(_ points: [(CGFloat, CGFloat)], fill: String? = nil, stroke: String? = nil, lineWidth: CGFloat = 1, close: Bool = true) {
        let path = CGMutablePath()
        if let first = points.first {
            path.move(to: CGPoint(x: first.0, y: first.1))
            for point in points.dropFirst() { path.addLine(to: CGPoint(x: point.0, y: point.1)) }
            if close { path.closeSubpath() }
        }
        shapes.append(Shape(path: path, fill: fill, stroke: stroke, lineWidth: lineWidth))
    }
    mutating func rect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, fill: String, radius: CGFloat = 0, stroke: String? = nil, lineWidth: CGFloat = 1) {
        let path = CGPath(roundedRect: CGRect(x: x, y: y, width: w, height: h), cornerWidth: radius, cornerHeight: radius, transform: nil)
        shapes.append(Shape(path: path, fill: fill, stroke: stroke, lineWidth: lineWidth))
    }
    mutating func line(_ x1: CGFloat, _ y1: CGFloat, _ x2: CGFloat, _ y2: CGFloat, color: String = rule, width: CGFloat = 1) {
        path([(x1, y1), (x2, y2)], stroke: color, lineWidth: width, close: false)
    }
    mutating func text(_ string: String, x: CGFloat, baseline: CGFloat, size: CGFloat, color: String = ink, font: String = "HelveticaNeue", tracking: CGFloat = 0, right: Bool = false) {
        let font = CTFontCreateWithName(font as CFString, size, nil)
        let attributes: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTKernAttributeName as String): tracking,
        ]
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: string, attributes: attributes))
        let start = right ? x - CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) : x
        let path = CGMutablePath()
        for case let run as CTRun in CTLineGetGlyphRuns(line) as NSArray {
            let count = CTRunGetGlyphCount(run)
            var glyphs = [CGGlyph](repeating: 0, count: count)
            var positions = [CGPoint](repeating: .zero, count: count)
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
            CTRunGetPositions(run, CFRange(location: 0, length: 0), &positions)
            for index in 0..<count {
                if let glyph = CTFontCreatePathForGlyph(font, glyphs[index], nil) {
                    let position = positions[index]
                    path.addPath(glyph, transform: CGAffineTransform(a: 1, b: 0, c: 0, d: -1, tx: start + position.x, ty: baseline - position.y))
                }
            }
        }
        shapes.append(Shape(path: path, fill: color, stroke: nil, lineWidth: 0))
    }
    mutating func mark(x: CGFloat, y: CGFloat, size: CGFloat, monochrome: Bool = false, color: String = ink) {
        // Four planes share an optical two-unit seam. The low orange plane is
        // Kite's single brand accent. The silhouette remains intact in one ink.
        let scale = size / 100
        for (points, fill) in [
            ([(50.4, 8.0), (13.0, 42.0), (47.6, 42.0)], color),
            ([(53.5, 8.0), (88.0, 42.0), (50.8, 42.0)], color),
            ([(15.0, 45.0), (47.2, 45.0), (43.2, 92.0)], color),
            ([(50.4, 45.0), (86.0, 45.0), (46.4, 92.0)], monochrome ? color : orange),
        ] {
            path(points.map { (x + CGFloat($0.0) * scale, y + CGFloat($0.1) * scale) }, fill: fill)
        }
    }
    func svg(label: String) -> String {
        func number(_ value: CGFloat) -> String {
            let formatted = String(format: "%.3f", Double(value))
            return formatted.replacingOccurrences(of: #"\.?0+$"#, with: "", options: .regularExpression)
        }
        func data(_ path: CGPath) -> String {
            var commands = [String]()
            path.applyWithBlock { element in
                let e = element.pointee
                let p = e.points
                switch e.type {
                case .moveToPoint: commands.append("M\(number(p[0].x)) \(number(p[0].y))")
                case .addLineToPoint: commands.append("L\(number(p[0].x)) \(number(p[0].y))")
                case .addQuadCurveToPoint: commands.append("Q\(number(p[0].x)) \(number(p[0].y)) \(number(p[1].x)) \(number(p[1].y))")
                case .addCurveToPoint: commands.append("C\(number(p[0].x)) \(number(p[0].y)) \(number(p[1].x)) \(number(p[1].y)) \(number(p[2].x)) \(number(p[2].y))")
                case .closeSubpath: commands.append("Z")
                @unknown default: break
                }
            }
            return commands.joined(separator: " ")
        }
        let paths = shapes.map { shape -> String in
            let stroke = shape.stroke.map { " stroke=\"\($0)\" stroke-width=\"\(number(shape.lineWidth))\"" } ?? ""
            return "  <path d=\"\(data(shape.path))\" fill=\"\(shape.fill ?? "none")\"\(stroke)/>"
        }.joined(separator: "\n")
        return """
        <svg xmlns="http://www.w3.org/2000/svg" width="\(width)" height="\(height)" viewBox="0 0 \(width) \(height)" role="img" aria-label="\(label)">
          <title>\(label)</title>
        \(paths)
        </svg>

        """
    }
    func png(pixels: Int? = nil) throws -> Data {
        let scale = CGFloat(pixels ?? width) / CGFloat(width)
        let pixelWidth = Int(CGFloat(width) * scale)
        let pixelHeight = Int(CGFloat(height) * scale)
        let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!
        guard let context = CGContext(data: nil, width: pixelWidth, height: pixelHeight, bitsPerComponent: 8, bytesPerRow: pixelWidth * 4, space: colorSpace, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            fatalError("Cannot create \(pixelWidth)x\(pixelHeight) bitmap")
        }
        context.translateBy(x: 0, y: CGFloat(pixelHeight))
        context.scaleBy(x: scale, y: -scale)
        context.setAllowsAntialiasing(true)
        func cgColor(_ hex: String) -> CGColor {
            let value = UInt32(hex.dropFirst(), radix: 16)!
            return CGColor(colorSpace: colorSpace, components: [CGFloat((value >> 16) & 255) / 255, CGFloat((value >> 8) & 255) / 255, CGFloat(value & 255) / 255, 1])!
        }
        for shape in shapes {
            if let fill = shape.fill {
                context.addPath(shape.path)
                context.setFillColor(cgColor(fill))
                context.fillPath()
            }
            if let stroke = shape.stroke {
                context.addPath(shape.path)
                context.setStrokeColor(cgColor(stroke))
                context.setLineWidth(shape.lineWidth)
                context.strokePath()
            }
        }
        guard let image = context.makeImage(), let data = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { fatalError("Cannot encode PNG") }
        return data
    }
    func saveSVG(_ name: String, label: String, directory: URL = brand) throws {
        try Data(svg(label: label).utf8).write(to: directory.appendingPathComponent(name))
    }
}

var icon = Drawing(width: 1024, height: 1024)
icon.rect(64, 64, 896, 896, fill: paper, radius: 192, stroke: rule, lineWidth: 2)
icon.mark(x: 128, y: 124, size: 768)
try icon.saveSVG("kite.svg", label: "Kite app icon", directory: output)
try icon.png().write(to: output.appendingPathComponent("kite.png"))

for (name, monochrome, color) in [("mark.svg", false, ink), ("mark-ink.svg", true, ink), ("mark-paper.svg", true, paper)] {
    var mark = Drawing(width: 1000, height: 1000)
    mark.mark(x: 0, y: 0, size: 1000, monochrome: monochrome, color: color)
    try mark.saveSVG(name, label: "Kite geometric mark")
}
var lockup = Drawing(width: 525, height: 200)
lockup.mark(x: 0, y: 0, size: 192)
lockup.text("kite", x: 213, baseline: 158, size: 172, font: "HelveticaNeue-Bold", tracking: -4)
try lockup.saveSVG("wordmark.svg", label: "Kite wordmark")

func banner(width: Int, height: Int) -> Drawing {
    var art = Drawing(width: width, height: height)
    let w = CGFloat(width)
    let h = CGFloat(height)
    let inset: CGFloat = width == 1600 ? 80 : 64
    let titleTop: CGFloat = 138
    art.rect(0, 0, w, h, fill: paper)
    art.text("KITE / NATIVE CODE EDITOR", x: inset, baseline: 52, size: 16, color: muted, font: "Menlo-Regular", tracking: 0.5)
    art.text("OPEN SOURCE / DESKTOP", x: w - inset, baseline: 52, size: 16, color: muted, font: "Menlo-Regular", tracking: 0.5, right: true)
    art.line(inset, 78, w - inset, 78)
    art.mark(x: inset - 14, y: titleTop - 18, size: 156)
    art.text("kite", x: inset + 155, baseline: titleTop + 111, size: 168, font: "HelveticaNeue-Bold", tracking: -5)
    art.text("A small space", x: inset, baseline: titleTop + 223, size: 52, tracking: -1.2)
    art.text("for serious work.", x: inset, baseline: titleTop + 284, size: 52, tracking: -1.2)
    let markSize: CGFloat = width == 1600 ? 390 : 325
    let mx = w - inset - markSize
    let my: CGFloat = 155
    art.line(mx - 48, my - 10, mx - 48, h - 126)
    art.mark(x: mx, y: my, size: markSize)
    art.text("EDITOR + TERMINAL", x: mx + markSize / 2 + 6, baseline: h - 147, size: 15, color: muted, font: "Menlo-Regular", tracking: 0.4, right: true)
    art.line(inset, h - 88, w - inset, h - 88)
    art.text("01  EDIT", x: inset, baseline: h - 40, size: 16, font: "Menlo-Regular", tracking: 0.4)
    art.text("02  RUN", x: inset + 190, baseline: h - 40, size: 16, font: "Menlo-Regular", tracking: 0.4)
    art.text("03  MAKE", x: inset + 370, baseline: h - 40, size: 16, font: "Menlo-Regular", tracking: 0.4)
    art.text("BUILT WITH RUST", x: w - inset, baseline: h - 40, size: 16, color: muted, font: "Menlo-Regular", tracking: 0.4, right: true)
    return art
}
let readme = banner(width: 1600, height: 680)
try readme.saveSVG("readme-banner.svg", label: "Kite. A small space for serious work. Native code editor and terminal, built with Rust.")
try readme.png().write(to: brand.appendingPathComponent("readme-banner.png"))
let social = banner(width: 1280, height: 640)
try social.saveSVG("social-preview.svg", label: "Kite. A small space for serious work.")
try social.png().write(to: brand.appendingPathComponent("social-preview.png"))

// Modern ICNS and ICO containers hold PNG variants, rendered directly from the
// vector paths at every size rather than downsampling one raster master.
func big32(_ value: UInt32) -> Data {
    var big = value.bigEndian
    return withUnsafeBytes(of: &big) { Data($0) }
}
let macSizes: [(String, Int)] = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512)]
var blocks = Data()
for (type, size) in macSizes {
    let layer = try icon.png(pixels: size)
    blocks.append(Data(type.utf8))
    blocks.append(big32(UInt32(layer.count + 8)))
    blocks.append(layer)
}
var icns = Data("icns".utf8)
icns.append(big32(UInt32(blocks.count + 8)))
icns.append(blocks)
try icns.write(to: output.appendingPathComponent("kite.icns"))

let sizes = [16, 24, 32, 48, 64, 128, 256]
let layers = try sizes.map { try icon.png(pixels: $0) }
var ico = Data()
func append16(_ value: UInt16) {
    var little = value.littleEndian
    withUnsafeBytes(of: &little) { ico.append(contentsOf: $0) }
}
func append32(_ value: UInt32) {
    var little = value.littleEndian
    withUnsafeBytes(of: &little) { ico.append(contentsOf: $0) }
}
append16(0)
append16(1)
append16(UInt16(sizes.count))
var offset = UInt32(6 + sizes.count * 16)
for (index, size) in sizes.enumerated() {
    ico.append(contentsOf: [size == 256 ? 0 : UInt8(size), size == 256 ? 0 : UInt8(size), 0, 0])
    append16(1)
    append16(32)
    append32(UInt32(layers[index].count))
    append32(offset)
    offset += UInt32(layers[index].count)
}
for layer in layers { ico.append(layer) }
try ico.write(to: output.appendingPathComponent("kite.ico"))
print("Exported Kite SVG, PNG, ICNS, ICO, wordmark, README banner, and social preview.")
