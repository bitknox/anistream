// The home-screen widgets: Continue watching and Airing today, drawn with Glance from the record
// of them the app keeps (the engine's WidgetSnapshot).
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
    // The widgets drawn on the JVM beside their boards.
    id("anistream.screenshots")
}

android {
    namespace = "tv.anistream.widgets"
}

dependencies {
    api(project(":ui"))
    api(libs.glance.appwidget)
    implementation(libs.work.runtime)

    testImplementation(project(":testkit"))
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.coroutines.test)
    testImplementation(libs.glance.appwidget.testing)
}
