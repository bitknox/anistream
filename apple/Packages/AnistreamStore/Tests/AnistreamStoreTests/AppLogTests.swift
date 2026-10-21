// What the app's log may say of an address.

@testable import AnistreamStore
import Testing

struct LogTests {
    @Test func anAddressInALineKeepsItsSchemeAndHostAndLosesItsToken() {
        let line = "Opening failed or was aborted: https://cdn.example/a/b.m3u8?token=s3cret&e=1 (403)"
        #expect(AppLog.redacting(line) == "Opening failed or was aborted: https://cdn.example (403)")
        #expect(AppLog.redacting("tcp: http://127.0.0.1:7878/mend/abc0123/seg-4.ts failed") == "tcp: http://127.0.0.1:7878 failed")
        #expect(AppLog.redacting("a from 'https://user:pw@cdn.example/x' and b") == "a from 'https://cdn.example' and b")
        #expect(AppLog.redacting("no address here") == "no address here")
        #expect(!AppLog.redacting("one https://a.example/1?sig=x then https://b.example/2?sig=y").contains("sig="))
    }
}
