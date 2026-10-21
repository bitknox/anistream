// The window rendered to a PNG from inside the process, which needs no Screen Recording
// permission: AppKit draws the window's views into a bitmap of their own.

import AppKit
import Darwin

enum Snapshot {
    enum Problem: Error, CustomStringConvertible {
        case noWindow
        case noBitmap
        case noPNG

        var description: String {
            switch self {
            case .noWindow: "there is no window to render"
            case .noBitmap: "the window gave no bitmap to render into"
            case .noPNG: "the bitmap could not be written as a PNG"
            }
        }
    }

    /// The app's main window: the visible one that is not a panel.
    @MainActor
    static var window: NSWindow? {
        NSApp.windows.first { $0.isVisible && !($0 is NSPanel) }
    }

    /// `CGWindowListCreateImage`, which the SDK marks unavailable in favour of ScreenCaptureKit
    /// but which still renders a process's own window, glass included, without asking for Screen
    /// Recording, which ScreenCaptureKit does. Looked up at run time; `nil` where it is gone.
    private typealias CreateImage = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
    private static let createImage: CreateImage? = {
        guard let handle = dlopen("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_NOW),
              let symbol = dlsym(handle, "CGWindowListCreateImage")
        else { return nil }
        return unsafeBitCast(symbol, to: CreateImage.self)
    }()

    /// `window` as the window server composites it, at its backing scale.
    @MainActor
    static func composited(_ window: NSWindow) -> CGImage? {
        // kCGWindowListOptionIncludingWindow, and kCGWindowImageBoundsIgnoreFraming with
        // kCGWindowImageBestResolution.
        let image = createImage?(.null, 1 << 3, UInt32(window.windowNumber), (1 << 0) | (1 << 3))
        return image?.takeRetainedValue()
    }

    /// Render `window` to `url`: as the window server composites it when that is allowed, and
    /// drawn by AppKit otherwise, which leaves out what the window server draws itself, such as
    /// glass.
    @MainActor
    static func write(_ window: NSWindow, to url: URL) throws {
        if let image = composited(window).flatMap({ withSheets($0, of: window) }), image.width > 1 {
            let bitmap = NSBitmapImageRep(cgImage: image)
            guard let png = bitmap.representation(using: .png, properties: [:]) else { throw Problem.noPNG }
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true
            )
            try png.write(to: url)
            return
        }
        try drawn(window, to: url)
    }

    /// `image` of `window` with the sheet over it, and that sheet's own, where the window server
    /// draws them: a sheet is a window of its own, which a capture of one window leaves out.
    @MainActor
    static func withSheets(_ image: CGImage, of window: NSWindow) -> CGImage? {
        var sheets: [NSWindow] = []
        var next = window.attachedSheet
        while let sheet = next {
            sheets.append(sheet)
            next = sheet.attachedSheet
        }
        guard !sheets.isEmpty else { return image }
        let scale = window.backingScaleFactor
        guard let context = CGContext(
            data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return image }
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        for sheet in sheets {
            guard let picture = composited(sheet) else { continue }
            // Both in screen points, from the bottom left, as Core Graphics draws.
            let x = (sheet.frame.minX - window.frame.minX) * scale
            let y = (sheet.frame.minY - window.frame.minY) * scale
            context.draw(picture, in: CGRect(x: x, y: y, width: CGFloat(picture.width), height: CGFloat(picture.height)))
        }
        return context.makeImage()
    }

    /// Render `window`, its toolbar and sidebar with it, at its backing scale, to `url`, as
    /// AppKit draws its views.
    @MainActor
    static func drawn(_ window: NSWindow, to url: URL) throws {
        // The frame view holds the toolbar and the content beneath it.
        guard let view = window.contentView?.superview ?? window.contentView else { throw Problem.noWindow }
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        let bounds = view.bounds
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: bounds) else { throw Problem.noBitmap }
        view.cacheDisplay(in: bounds, to: bitmap)
        guard let png = bitmap.representation(using: .png, properties: [:]) else { throw Problem.noPNG }
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true
        )
        try png.write(to: url)
    }
}
