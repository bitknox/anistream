// What the app's tests share, and nothing the app ships: the engine's events recorded as scenes,
// which the screenshot tests replay into the store, and the fixture titles' art, read from the
// Rust test kit beside it. Only ever a test dependency.
plugins {
    id("anistream.android.library")
    id("anistream.engine.flavours")
}

android {
    namespace = "tv.anistream.testkit"
}

dependencies {
    api(project(":engine"))
    implementation(libs.kotlin.reflect)
    implementation(libs.kotlinx.serialization.json)

    testImplementation(libs.junit)
}
