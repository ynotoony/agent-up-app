// 生成 AgentUp Harness 应用图标：indigo 圆角方块 + 白色"进展闪电"标。
// 运行：swift scripts/make-icon.swift build/icon_1024.png
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let size = 1024.0
let ctx = CGContext(
    data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
    space: CGColorSpace(name: CGColorSpace.sRGB)!,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
)!

let bg = CGRect(x: 0, y: 0, width: size, height: size)

// macOS 图标留白：内容约占 82.4%（圆角矩形plate）
let plateSize = size * 0.824
let plate = CGRect(x: (size - plateSize) / 2, y: (size - plateSize) / 2, width: plateSize, height: plateSize)
let radius = plateSize * 0.225

// 底色：indigo 到深 indigo 的纵向渐变
ctx.saveGState()
let platePath = CGPath(roundedRect: plate, cornerWidth: radius, cornerHeight: radius, transform: nil)
ctx.addPath(platePath)
ctx.clip()
let colors = [
    CGColor(red: 0.42, green: 0.43, blue: 0.94, alpha: 1),
    CGColor(red: 0.33, green: 0.35, blue: 0.86, alpha: 1),
] as CFArray
let gradient = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB)!, colors: colors, locations: [0, 1])!
ctx.drawLinearGradient(gradient, start: CGPoint(x: size / 2, y: size), end: CGPoint(x: size / 2, y: 0), options: [])
ctx.restoreGState()

// 闪电（进展/交付）标：白色，居中
func boltPath(scale: CGFloat, offset: CGPoint) -> CGPath {
    // 以 512x512 设计坐标定义闪电轮廓
    let points: [CGPoint] = [
        CGPoint(x: 292, y: 488),   // 顶
        CGPoint(x: 148, y: 268),
        CGPoint(x: 244, y: 268),
        CGPoint(x: 204, y: 76),    // 折点
        CGPoint(x: 368, y: 296),
        CGPoint(x: 268, y: 296),
        CGPoint(x: 320, y: 488),   // 回到顶右
    ]
    let path = CGMutablePath()
    let transformed = points.enumerated().map { _, p in
        CGPoint(x: (p.x - 256) * scale + offset.x, y: (p.y - 256) * scale + offset.y)
    }
    path.move(to: transformed[0])
    for i in 1..<transformed.count {
        path.addLine(to: transformed[i])
    }
    path.closeSubpath()
    return path
}

ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
ctx.addPath(boltPath(scale: plateSize / 512 * 0.92, offset: CGPoint(x: size / 2, y: size / 2)))
ctx.fillPath()

// 右下角小圆点：Agent 的"活动脉冲"意象
let dotRadius = plateSize * 0.052
let dotCenter = CGPoint(x: plate.maxX - plateSize * 0.17, y: plate.minY + plateSize * 0.17)
ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.92))
ctx.fillEllipse(in: CGRect(
    x: dotCenter.x - dotRadius, y: dotCenter.y - dotRadius,
    width: dotRadius * 2, height: dotRadius * 2
))

let image = ctx.makeImage()!
let outPath = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "build/icon_1024.png"
try! FileManager.default.createDirectory(
    onPathIsEmpty: (outPath as NSString).deletingLastPathComponent
)
let dest = CGImageDestinationCreateWithURL(
    URL(fileURLWithPath: outPath) as CFURL, UTType.png.identifier as CFString, 1, nil
)!
CGImageDestinationAddImage(dest, image, nil)
CGImageDestinationFinalize(dest)
print("icon written: \(outPath)")

extension FileManager {
    func createDirectory(onPathIsEmpty dir: String) throws {
        if !dir.isEmpty {
            try createDirectory(atPath: dir, withIntermediateDirectories: true)
        }
    }
}
