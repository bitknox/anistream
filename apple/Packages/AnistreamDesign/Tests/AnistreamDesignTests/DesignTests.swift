// The design tokens hold the brief's values, the show-title face loads, and Nagare is the
// website's sprite.

@testable import AnistreamDesign
import SwiftUI
import Testing

/// A colour's sRGB components, 0 to 255.
func components(_ color: Color) -> [Int] {
    let resolved = color.resolve(in: EnvironmentValues())
    return [resolved.red, resolved.green, resolved.blue].map { Int((Double($0) * 255).rounded()) }
}

struct DesignTests {
    @Test func thePaletteIsTheBriefs() {
        #expect(components(Palette.ground) == [0x16, 0x1A, 0x2E])
        #expect(components(Palette.silk) == [0xE4, 0xE0, 0xD6])
        #expect(components(Palette.amber) == [0xF2, 0xA6, 0x4B])
        #expect(components(Palette.water) == [0x7F, 0xC7, 0xD9])
        #expect(components(Palette.vermilion) == [0xE0, 0x6B, 0x54])
        #expect(components(Color(hex: "#e4a15d")!) == [0xE4, 0xA1, 0x5D])
        #expect(Color(hex: "not a colour") == nil)
    }

    @Test func theShowTitleFaceIsThere() {
        #expect(ShowTitleFace.isAvailable)
    }

    @Test func nagareIsTwentyByThirtyTwoInItsOwnColours() {
        #expect(NagareSprite.rows.count == NagareSprite.height)
        for row in NagareSprite.rows {
            #expect(row.count == NagareSprite.width)
            for pixel in row where pixel != "." {
                #expect(NagareSprite.colors[pixel] != nil, "\(pixel) has a colour")
            }
        }
    }

    @Test func aPlateWithoutAColourIsTheRaisedOne() {
        #expect(CoverPlate.muted(nil) == Palette.raised)
        #expect(components(CoverPlate.muted(Palette.ground)) == [0x16, 0x1A, 0x2E])
    }
}
