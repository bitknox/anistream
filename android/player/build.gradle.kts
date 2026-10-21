// The player: libmpv behind the engine's hosted player protocol, with the JNI shim, the media
// session and the player's screens (AnistreamPlayer on Apple).
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
    // The player's boards drawn on the JVM, as the screens' are.
    id("anistream.screenshots")
}

// libmpv and FFmpeg's shared libraries, LGPL, from android/scripts/mpv.sh.
val mpvDir = rootDir.resolve(".mpv/current")
val testMediaDir = layout.buildDirectory.dir("generated/playerTestMedia")
val licencesDir = layout.buildDirectory.dir("generated/playerLicences")

// CI does not run mpv.sh. With ANISTREAM_PLAYER_NATIVE=off the module builds without libmpv and
// the shim, so the Kotlin compiles and its unit tests run, and the APK it makes cannot play.
val native = System.getenv("ANISTREAM_PLAYER_NATIVE") != "off"

android {
    namespace = "tv.anistream.player"
    ndkVersion = libs.versions.ndk.get()

    defaultConfig {
        ndk { abiFilters += "arm64-v8a" }
        if (native) {
            externalNativeBuild {
                cmake { arguments += listOf("-DANDROID_STL=none", "-DMPV_DIR=${mpvDir.path}") }
            }
        }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    if (native) {
        externalNativeBuild {
            cmake {
                path = file("src/main/cpp/CMakeLists.txt")
                version = libs.versions.cmake.get()
            }
        }
    }
    sourceSets {
        named("main") {
            if (native) jniLibs.directories.add(mpvDir.resolve("jniLibs").path)
            // What the LGPL and the other licences ask to ship with the libraries, copied in by playerLicences.
            assets.directories.add(licencesDir.get().asFile.path)
        }
        // The instrumented tests' episode, from android/scripts/player-media.sh, copied in by playerTestMedia.
        named("androidTestFixtures") { assets.directories.add(testMediaDir.get().asFile.path) }
    }

    packaging {
        // The shim's CMake imports libmpv and libavcodec, which are in jniLibs as well.
        jniLibs.pickFirsts += listOf("**/libmpv.so", "**/libavcodec.so")
        jniLibs.useLegacyPackaging = false
    }
}

dependencies {
    implementation(project(":store"))
    // The frame's player slot, the art and the page's shared transitions.
    implementation(project(":ui"))
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.core.ktx)
    implementation(libs.media3.session)
    implementation(libs.media3.common)

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(project(":testkit"))
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.compose.ui.test.manifest)

    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.coroutines.test)
}

// Fails the build when libmpv is missing, or older than android/scripts/mpv.sh says it should be.
if (native) {
    val mpvCheck = tasks.register<Exec>("mpvCheck") {
        description = "Checks that android/.mpv holds the libmpv mpv.sh builds."
        commandLine(rootDir.resolve("scripts/mpv.sh").path, "--check")
    }
    tasks.named("preBuild") { dependsOn(mpvCheck) }
}

// assets/licences/player in the APK: each library's licence as mpv.sh collected them, with
// android/mpv/NOTICE among them, and this module's NOTICE, which both credit mpv-android.
val playerLicences = tasks.register<Sync>("playerLicences") {
    description = "Copies the player libraries' licences into the APK's assets."
    from(mpvDir.resolve("licenses"))
    from(file("NOTICE")) { rename { "NOTICE-player" } }
    into(licencesDir.map { it.dir("licences/player") })
}

val playerTestMedia = tasks.register<Sync>("playerTestMedia") {
    description = "Copies the instrumented tests' episode from android/.mpv/media."
    from(rootDir.resolve(".mpv/media")) { include("episode.mkv") }
    into(testMediaDir)
}

tasks.configureEach {
    when {
        name.startsWith("merge") && name.endsWith("AndroidTestAssets") -> dependsOn(playerTestMedia)
        (name.startsWith("merge") || name.startsWith("package")) && name.endsWith("Assets") -> dependsOn(playerLicences)
        // Lint reads every asset directory too: its model of a variant, and a release build's
        // lint-vital, which Gradle fails for reading these without the tasks that write them.
        name.contains("lint", ignoreCase = true) -> dependsOn(playerLicences, playerTestMedia)
    }
}
