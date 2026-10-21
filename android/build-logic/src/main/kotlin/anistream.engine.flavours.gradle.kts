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
