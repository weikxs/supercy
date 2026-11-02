import AppKit
import ScreenCaptureKit

/// The Screen Recording permission. Only checked when the user starts a screenshot (or looks
/// at the permissions tab); macOS is asked only when the user chooses to open the settings.
enum ScreenCapturePermission {
    static var isAllowed: Bool { CGPreflightScreenCaptureAccess() }

    /// Lets macOS list Supercy under Screen Recording (its own prompt, the first time), then
    /// opens that pane of System Settings.
    static func openSettings() {
        if !CGPreflightScreenCaptureAccess() { _ = CGRequestScreenCaptureAccess() }
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            NSWorkspace.shared.open(url)
        }
    }
}

/// One display, frozen at the moment the screenshot started.
struct DisplayShot {
    let screen: NSScreen
    let displayID: CGDirectDisplayID
    /// The whole display, in pixels.
    let image: CGImage
    /// Image pixels per point.
    let scale: CGFloat
    /// Selectable windows on this display, front to back, in display space.
    let windows: [CaptureWindow]

    /// The display's own rectangle in display space.
    var bounds: CGRect { CGRect(origin: .zero, size: screen.frame.size) }
}

/// ScreenCaptureKit, used only while a screenshot is being taken: nothing here is kept
/// between screenshots.
enum ScreenCapturer {
    enum Failure: Error {
        case noDisplays
        case capture(Error?)
    }

    /// A display to capture, with everything about its screen read on the main thread.
    private struct Target {
        let screen: NSScreen
        let id: CGDirectDisplayID
        /// Points.
        let size: CGSize
        /// Pixels.
        let pixels: CGSize
        let windows: [CaptureWindow]
    }

    /// Takes a screenshot of every display; calls back on the main thread.
    static func captureDisplays(completion: @escaping (Result<[DisplayShot], Failure>) -> Void) {
        let windowList = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]] ?? []
        let targets = NSScreen.screens.compactMap { screen -> Target? in
            guard let id = displayID(of: screen) else { return nil }
            let size = screen.frame.size, scale = screen.backingScaleFactor
            let windows = CaptureWindow.selectable(from: windowList, ownProcess: getpid(), displayBounds: CGDisplayBounds(id))
            return Target(screen: screen, id: id, size: size,
                          pixels: CGSize(width: size.width * scale, height: size.height * scale), windows: windows)
        }
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, error in
            guard let content else {
                DispatchQueue.main.async { completion(.failure(.capture(error))) }
                return
            }
            let group = DispatchGroup()
            let collected = ShotCollector()
            for target in targets {
                guard let display = content.displays.first(where: { $0.displayID == target.id }) else { continue }
                let config = configuration(width: target.pixels.width, height: target.pixels.height)
                let filter = SCContentFilter(display: display, excludingWindows: [])
                group.enter()
                SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { image, error in
                    let shot = image.map { image in
                        DisplayShot(screen: target.screen, displayID: target.id, image: image,
                                    scale: CGFloat(image.width) / max(target.size.width, 1), windows: target.windows)
                    }
                    collected.add(shot, error: error)
                    group.leave()
                }
            }
            group.notify(queue: .main) {
                let (shots, failure) = collected.result
                if shots.isEmpty {
                    completion(.failure(failure.map { .capture($0) } ?? .noDisplays))
                } else {
                    completion(.success(shots))
                }
            }
        }
    }

    /// Takes a screenshot of one window alone (without its shadow, nothing in front of it), at
    /// its own pixel scale; calls back on the main thread with `nil` when it is gone.
    static func captureWindow(id: UInt32, completion: @escaping (CGImage?) -> Void) {
        SCShareableContent.getExcludingDesktopWindows(true, onScreenWindowsOnly: true) { content, _ in
            guard let window = content?.windows.first(where: { $0.windowID == id }) else {
                DispatchQueue.main.async { completion(nil) }
                return
            }
            let filter = SCContentFilter(desktopIndependentWindow: window)
            let scale = CGFloat(filter.pointPixelScale)
            let config = configuration(width: filter.contentRect.width * scale, height: filter.contentRect.height * scale)
            config.ignoreShadowsSingleWindow = true
            SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { image, _ in
                DispatchQueue.main.async { completion(image) }
            }
        }
    }

    private static func configuration(width: CGFloat, height: CGFloat) -> SCStreamConfiguration {
        let config = SCStreamConfiguration()
        config.width = max(1, Int(width.rounded()))
        config.height = max(1, Int(height.rounded()))
        config.showsCursor = false
        return config
    }

    static func displayID(of screen: NSScreen) -> CGDirectDisplayID? {
        (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value
    }
}

/// Gathers the displays' screenshots as they arrive from ScreenCaptureKit's queue.
private final class ShotCollector {
    private let lock = NSLock()
    private var shots: [DisplayShot] = []
    private var failure: Error?

    func add(_ shot: DisplayShot?, error: Error?) {
        lock.lock()
        defer { lock.unlock() }
        if let shot { shots.append(shot) } else { failure = error }
    }

    var result: ([DisplayShot], Error?) {
        lock.lock()
        defer { lock.unlock() }
        return (shots, failure)
    }
}
