// Values as JSON, the way the engine's reports and its Set op carry them.

import AnistreamPlayer
import Testing

extension PlayerTests {
    @Suite struct ValueTests {
        @Test func writesJSONAsMpvsIPCDoes() {
            #expect(MpvValue.flag(true).json == "true")
            #expect(MpvValue.int(42).json == "42")
            #expect(MpvValue.double(1.5).json == "1.5")
            #expect(MpvValue.string("a/b").json == #""a/b""#)
            #expect(MpvValue.none.json == "null")
            #expect(MpvValue.double(.nan).json == "null")
            let tracks: MpvValue = [["id": 1, "type": "sub", "selected": true]]
            #expect(tracks.json == #"[{"id":1,"selected":true,"type":"sub"}]"#)
        }

        @Test func readsJSONBackToTheSameValue() throws {
            let value: MpvValue = [
                "start": "10", "volume": 40, "speed": 1.25, "mute": false, "list": ["a", .none],
            ]
            #expect(try MpvValue(json: value.json) == value)
            #expect(try MpvValue(json: "true") == .flag(true))
            #expect(try MpvValue(json: "3") == .int(3))
            #expect(try MpvValue(json: "3.0") == .double(3))
        }

        @Test func readsAsWhatItHolds() {
            let value: MpvValue = ["w": 1280, "aspect": 1.7777, "gamma": "pq", "on": true]
            #expect(value["w"].int == 1280)
            #expect(value["w"].double == 1280)
            #expect(value["aspect"].int == 1)
            #expect(value["gamma"].string == "pq")
            #expect(value["on"].bool == true)
            #expect(value["missing"] == .none)
            #expect(value.array.isEmpty)
        }
    }
}
