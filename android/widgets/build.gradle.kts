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
    // JNA's JVM build, with this machine's native library: the record's marks are the engine's words.
    testImplementation(libs.jna.jvm)
}

// The record's marks are the engine's own words, read from this Mac's build of the testing engine,
// android/.engine/<engine>/host, through JNA.
AnistreamSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        systemProperty("jna.library.path", library.path)
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        // The bindings free the engine's objects with Android's SystemCleaner, which Robolectric
        // builds on the JDK's own cleaner.
        jvmArgs("--add-exports=java.base/jdk.internal.ref=ALL-UNNAMED")
    }
}
