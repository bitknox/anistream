// The screens: the navigation bar and the destinations behind it (AnistreamUI on Apple).
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
    // The screenshot tests: each board's screen drawn on the JVM from a recorded scene.
    id("anistream.screenshots")
}

android {
    namespace = "tv.anistream.ui"
}

dependencies {
    api(project(":store"))
    api(project(":design"))
    implementation(libs.navigation.compose)
    implementation(libs.adaptive.layout)
    implementation(libs.adaptive.navigation)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.activity.compose)

    testImplementation(project(":testkit"))
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    // The activity a compose rule's content is set in.
    testImplementation(libs.compose.ui.test.manifest)
}
