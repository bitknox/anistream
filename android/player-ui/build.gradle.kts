// The player's screens: the frame, its chrome, cards and sheets, and picture in picture, on the
// phone's design. The TV draws its own on the same player.
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
    // The player's boards drawn on the JVM, as the screens' are.
    id("anistream.screenshots")
}

android {
    namespace = "tv.anistream.player.ui"
}

dependencies {
    api(project(":player"))
    implementation(project(":store"))
    // The frame's player slot, the art and the page's shared transitions.
    implementation(project(":ui"))
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.core.ktx)
    implementation(libs.media3.session)
    implementation(libs.media3.common)

    testImplementation(testFixtures(project(":player")))
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(project(":testkit"))
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.compose.ui.test.manifest)
}
