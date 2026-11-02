import AppKit
import ImageIO

/// The images pinned to the screen. Several can be open; each is independent. Not restored
/// after a restart (ADR-11).
final class PinsController: PinWindowDelegate {
    private var pins: [PinWindow] = []
    /// Copy and save are the capture output's (pasteboard, history, save panel).
    var onCopy: ((CGImage) -> Void)?
    var onSave: ((CGImage) -> Void)?
    /// Called after a pin closed (to hand its memory back).
    var onClosed: (() -> Void)?

    var count: Int { pins.count }

    /// Whether any pin lets clicks through (the menu bar menu turns it off again).
    var isAnyClickThrough: Bool { pins.contains { $0.isClickThrough } }

    func setClickThrough(_ on: Bool) {
        pins.forEach { $0.setClickThrough(on) }
    }

    /// Pins `image` (taken at `scale` pixels per point) where `frame` (AppKit coordinates) is,
    /// keeping no more pixels than the screen there shows at 100 %.
    func pin(_ image: CGImage, scale: CGFloat, frame: CGRect) {
        let screen = NSScreen.screens.first { $0.frame.intersects(frame) } ?? NSScreen.main
        let screenScale = screen?.backingScaleFactor ?? scale
        let kept = PinMetrics.keptPixels(pixels: CGSize(width: image.width, height: image.height), imageScale: scale,
                                         screenScale: screenScale)
        guard let bitmap = CaptureImage.fitting(image, maxWidth: Int(kept.width), maxHeight: Int(kept.height)) else {
            return
        }
        let pinScale = scale * CGFloat(bitmap.width) / CGFloat(max(image.width, 1))
        let pin = PinWindow(image: bitmap, scale: pinScale, frame: frame)
        pin.delegate = self
        pins.append(pin)
    }

    /// Pins an encoded image (a history item) in the middle of the screen with the pointer, at
    /// one image pixel per screen pixel, smaller if it would not fit.
    func pin(imageData: Data) {
        let mouse = NSEvent.mouseLocation
        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main,
              let source = CGImageSourceCreateWithData(imageData as CFData, nil),
              let decoded = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return }
        let scale = screen.backingScaleFactor
        let visible = screen.visibleFrame
        guard let image = CaptureImage.fitting(decoded, maxWidth: Int(visible.width * scale),
                                               maxHeight: Int(visible.height * scale)) else { return }
        let size = PinMetrics.size(pixels: CGSize(width: image.width, height: image.height), scale: scale, zoom: 1)
        let frame = CGRect(x: visible.midX - size.width / 2, y: visible.midY - size.height / 2,
                           width: size.width, height: size.height)
        pin(image, scale: scale, frame: frame)
    }

    func closeAll() {
        pins.forEach { $0.close() }
    }

    // MARK: PinWindowDelegate

    func pinWantsCopy(_ pin: PinWindow) {
        if let image = pin.image { onCopy?(image) }
    }

    func pinWantsSave(_ pin: PinWindow) {
        if let image = pin.image { onSave?(image) }
    }

    func pinDidClose(_ pin: PinWindow) {
        pins.removeAll { $0 === pin }
        onClosed?()
    }

    func pinWantsCloseAll(_ pin: PinWindow) {
        closeAll()
    }
}
