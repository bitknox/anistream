// The two builds of the app, by the engine inside: `app`, the engine the app ships, and
// `fixtures`, the engine's `testing` build on the test kit's fake AniList and fixture titles, for
// screenshots and tests, never for real use. Every module that reaches the engine has both, so a
// variant of the app takes the same engine all the way down.
pluginManager.withPlugin("com.android.library") {
    extensions.configure<com.android.build.api.dsl.LibraryExtension> {
        flavorDimensions += AnistreamSdk.ENGINE
        productFlavors {
            create("app") { dimension = AnistreamSdk.ENGINE }
            create("fixtures") { dimension = AnistreamSdk.ENGINE }
        }
    }
}
// Each test class in a JVM of its own. The engine's native library keeps one table of callbacks
// per interface for the whole process, set by whichever copy of the Kotlin bindings loads last,
// while each copy keeps its own map of the objects behind those callbacks. A JVM that runs a plain
// JUnit test and a Robolectric one loads two copies, the app's class loader's and the sandbox's
// (and each graphics mode has a sandbox of its own), so a host's callbacks reached the other
// copy's empty map: TvShots' tvSender then failed with "not enough bytes remaining", every test
// of the class, or once an earlier class's engine was freed. The app itself has one copy.
tasks.withType<Test>().configureEach {
    forkEvery = 1
}
pluginManager.withPlugin("com.android.application") {
    extensions.configure<com.android.build.api.dsl.ApplicationExtension> {
        flavorDimensions += AnistreamSdk.ENGINE
        productFlavors {
            create("app") {
                dimension = AnistreamSdk.ENGINE
                applicationId = "tv.anistream.app"
            }
            create("fixtures") {
                dimension = AnistreamSdk.ENGINE
                applicationId = "tv.anistream.fixtures"
            }
        }
    }
}
