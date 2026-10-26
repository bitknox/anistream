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
    implementation(project(":i18n"))
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
    // JNA's JVM build, with this Mac's native library, for the shots that start the engine.
    testImplementation(libs.jna.jvm)
}

// The Play on TV shots drive the fixtures' fake TVs on this Mac's build of the testing engine,
// android/.engine/<engine>/host, through JNA, under a HOME of their own as the store's tests are.
AnistreamSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        val home = layout.buildDirectory.dir("test-home/${this.name}").get().asFile
        val userHome = System.getProperty("user.home")
        systemProperty("jna.library.path", library.path)
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        // The bindings free the engine's objects with Android's SystemCleaner, which Robolectric
        // builds on the JDK's own cleaner.
        jvmArgs("--add-exports=java.base/jdk.internal.ref=ALL-UNNAMED")
        environment("HOME", home.path)
        environment("ANISTREAM_TOKEN_STORAGE", "file")
        doFirst {
            check(home.canonicalPath != File(userHome).canonicalPath) { "the tests' HOME would be the user's own" }
            home.deleteRecursively()
            home.mkdirs()
        }
    }
}
