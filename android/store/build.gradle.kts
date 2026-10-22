import java.io.File

// The engine as the app holds it, and the app's state: the client that reads the engine's events,
// the store that applies them to a model per feature, the launch, and the host the engine calls
// back into (AnistreamStore on Apple).
plugins {
    id("anistream.android.library")
    id("anistream.engine.flavours")
}

android {
    namespace = "tv.anistream.store"
}

dependencies {
    api(project(":engine"))
    api(libs.coroutines.android)

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    // JNA's JVM build, with this Mac's native library, for the tests that start the engine.
    testImplementation(libs.jna.jvm)
    // The scenes the recorder writes, as the screenshot tests read them.
    testImplementation(project(":testkit"))
}

// The unit tests start the engine on this Mac's build of it, android/.engine/<engine>/host, under
// a home of their own: HOME is a new directory under build/, never the user's, and tokens are
// files, so nothing a test does reaches the user's own anistream.
AnistreamSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        val home = layout.buildDirectory.dir("test-home/${this.name}").get().asFile
        val userHome = System.getProperty("user.home")
        systemProperty("jna.library.path", library.path)
        // JNA loads its native library itself, which Java now asks to be allowed.
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        environment("HOME", home.path)
        environment("ANISTREAM_TOKEN_STORAGE", "file")
        // android/scripts/scenes.sh: record the scenes into the test kit, every time it is run.
        if (providers.gradleProperty("anistream.scenes").isPresent) {
            systemProperty("anistream.scenes.out", rootDir.resolve("testkit/src/main/resources/tv/anistream/testkit/scenes").path)
            outputs.upToDateWhen { false }
        }
        doFirst {
            check(home.canonicalPath != File(userHome).canonicalPath) {
                "the tests' HOME would be the user's own"
            }
            home.deleteRecursively()
            home.mkdirs()
        }
    }
}
