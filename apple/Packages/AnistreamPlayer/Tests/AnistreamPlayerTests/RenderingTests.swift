// What the build of mpv brings: VideoToolbox without copies, HDR, libass with its Core Text fix,
// and a framework that keeps everything but mpv's API to itself.

import AnistreamPlayer
import Darwin
import Foundation
import Testing

extension PlayerTests {
    @Suite struct RenderingTests {
        @Test func decodesHEVCMain10InHardwareWithoutCopies() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "hdr10.mkv")
            // `videotoolbox`, not `videotoolbox-copy`: frames stay IOSurfaces, mapped into Metal.
            let hwdec = try await eventually { () async throws -> String? in
                try? await client.get("hwdec-current").string
            }
            #expect(hwdec == "videotoolbox")
            let params = try await client.get("video-params")
            #expect(params["hw-pixelformat"].string == "p010")
            #expect(params["gamma"].string == "pq")
            #expect(params["primaries"].string == "bt.2020")
            await client.shutdown()
        }

        @Test func drawsHanWithCoreTextsCascadeInsteadOfBoxes() async throws {
            let (client, _) = try await startedClient { $0.logLevel = "v" }
            let recorder = LogRecorder(client)
            try await play(client, "subs.mkv")
            try await Task.sleep(for: .seconds(1.5))
            let image = try await client.screenshot()
            await client.shutdown()
            let lines = await recorder.lines().filter { $0.prefix.hasSuffix("ass") }
            let failures = lines.filter {
                $0.text.contains("failed to find any fallback") || $0.text.contains("Error opening font")
            }
            // Unpatched, every Han glyph here fails: PingFang, Core Text's pick, will not open.
            #expect(failures.isEmpty, "\(failures.map(\.text))")
            #expect(lines.contains { $0.text.contains("HiraginoSans") || $0.text.contains("Hiragino Sans") })
            #expect(variety(image) > 0.01)
        }

        @Test func usesTheFontsAFileCarries() async throws {
            let (client, _) = try await startedClient { $0.logLevel = "v" }
            let recorder = LogRecorder(client)
            try await play(client, "subs.mkv")
            try await Task.sleep(for: .seconds(1))
            await client.shutdown()
            let lines = await recorder.lines()
            // Dela Gothic One is not installed: this is the MKV's attachment.
            #expect(lines.contains {
                $0.text.contains("(Dela Gothic One") && $0.text.contains("DelaGothicOne-Regular")
            })
        }

        @Test func drawsSimplifiedHanWithTheFontTheIOSAppBundles() async throws {
            // iOS has no font FreeType opens with simplified-only characters (简): the app
            // bundles Noto Sans CJK SC, which the Mac test names here as the iOS app would.
            let fonts = try Media.fonts()
            let (client, _) = try await startedClient {
                $0.logLevel = "v"
                $0.subtitleFallback = SubtitleFont(directory: fonts, family: SubtitleFont.bundledFamily)
            }
            #expect(try await client.get("sub-font").string == "Noto Sans CJK SC")
            let recorder = LogRecorder(client)
            try await play(client, "subs.mkv")
            try await Task.sleep(for: .seconds(1))
            await client.shutdown()
            let lines = await recorder.lines()
            // First in line now, before Core Text's cascade, for the style whose font is missing
            // and for the Han that Helvetica Neue has not.
            #expect(lines.contains {
                $0.text.contains("Using default font family: (No Such Font Anistream")
                    && $0.text.contains("NotoSansCJKsc-Regular")
            })
            #expect(lines.contains {
                $0.text.contains("(Helvetica Neue, 400, 0)") && $0.text.contains("NotoSansCJKsc-Regular")
            })
            #expect(!lines.contains { $0.text.contains("failed to find any fallback") })
        }

        @Test func bundlesNoFallbackFontOnTheMac() {
            #if os(macOS)
            #expect(MpvConfiguration().subtitleFallback == nil)
            #endif
        }

        @Test func exportsNothingButTheClientAPI() throws {
            // Mpv.framework itself, not every image loaded: macOS's own LibreSSL answers SSL_new.
            var info = Dl_info()
            let create = try #require(dlsym(UnsafeMutableRawPointer(bitPattern: -2), "mpv_create"))
            try #require(dladdr(create, &info) != 0)
            let path = String(cString: info.dli_fname)
            #expect(path.hasSuffix("Mpv.framework/Versions/A/Mpv") || path.hasSuffix("Mpv.framework/Mpv"))
            let framework = try #require(dlopen(path, RTLD_NOLOAD | RTLD_FIRST))
            defer { dlclose(framework) }
            #expect(dlsym(framework, "mpv_command_node") != nil)
            // OpenSSL, FFmpeg, libass, libplacebo and MoltenVK stay inside, out of the engine's way.
            for symbol in [
                "SSL_new", "OPENSSL_init_crypto", "gnutls_init", "av_log", "avcodec_open2",
                "ass_library_init", "pl_log_create", "vkCreateInstance",
            ] {
                #expect(dlsym(framework, symbol) == nil, "\(symbol) is visible")
            }
        }
    }
}
