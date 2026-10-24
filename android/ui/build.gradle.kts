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
    // JNA's JVM build, with this Mac's native library: Search asks the engine's model for the
    // words of its filters, as the app does.
    testImplementation(libs.jna.jvm)
}

// The engine's model on this Mac's build of it, android/.engine/<engine>/host; no engine runs.
AnistreamSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        systemProperty("jna.library.path", library.path)
        // JNA loads its native library itself, which Java now asks to be allowed.
        jvmArgs("--enable-native-access=ALL-UNNAMED")
    }
}
