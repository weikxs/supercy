import CoreGraphics
import Foundation

/// Image work on captured screenshots, with Core Graphics only.
public enum CaptureImage {
    /// The pixels `rect` of `image` copied into their own memory. `CGImage.cropping(to:)` alone
    /// would keep the whole screenshot alive behind the small crop; this copy lets it go.
    public static func crop(_ image: CGImage, to rect: CGRect) -> CGImage? {
        guard let part = image.cropping(to: rect) else { return nil }
        return copy(part, width: part.width, height: part.height)
    }

    /// `image` redrawn at `width` × `height` pixels in new memory.
    public static func copy(_ image: CGImage, width: Int, height: Int) -> CGImage? {
        guard width > 0, height > 0 else { return nil }
        let space = image.colorSpace.flatMap { $0.supportsOutput ? $0 : nil } ?? CGColorSpace(name: CGColorSpace.sRGB)
        guard let space, let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                                 bytesPerRow: 0, space: space,
                                                 bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()
    }

    /// `image` made smaller, if needed, to at most `maxWidth` × `maxHeight` pixels (aspect kept);
    /// the same image when it already fits.
    public static func fitting(_ image: CGImage, maxWidth: Int, maxHeight: Int) -> CGImage? {
        guard image.width > maxWidth || image.height > maxHeight else { return image }
        let factor = min(Double(maxWidth) / Double(image.width), Double(maxHeight) / Double(image.height))
        let width = max(1, Int((Double(image.width) * factor).rounded()))
        let height = max(1, Int((Double(image.height) * factor).rounded()))
        return copy(image, width: width, height: height)
    }

    /// The colour of pixel (`x`, `y`) (from the top-left), converted to sRGB.
    public static func color(of image: CGImage, x: Int, y: Int) -> PixelColor? {
        guard x >= 0, y >= 0, x < image.width, y < image.height,
              let pixel = image.cropping(to: CGRect(x: x, y: y, width: 1, height: 1)),
              let space = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
        var bytes = [UInt8](repeating: 0, count: 4)
        let drawn = bytes.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(data: buffer.baseAddress, width: 1, height: 1, bitsPerComponent: 8,
                                          bytesPerRow: 4, space: space,
                                          bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return false }
            context.draw(pixel, in: CGRect(x: 0, y: 0, width: 1, height: 1))
            return true
        }
        return drawn ? PixelColor(red: bytes[0], green: bytes[1], blue: bytes[2]) : nil
    }

    /// PNG data of `image`.
    public static func png(_ image: CGImage) -> Data? {
        SnapshotReader.pngData(image)
    }
}
