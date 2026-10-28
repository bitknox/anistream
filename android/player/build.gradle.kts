// The player: libmpv behind the engine's hosted player protocol, with the JNI shim, the media
// session and the model the screens draw from (AnistreamPlayer on Apple). The phone's screens are
// in :player-ui, so the TV can draw its own on the same player.
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
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
    // The tests read the words the person would, from the catalogue.
    testOptions { unitTests.isIncludeAndroidResources = true }

    defaultConfig {
        // The TV app plays here too, on Android 11 and on the 32-bit userspace most TV boxes run.
        minSdk = AnistreamSdk.TV_MIN
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
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
    // The scripted mpv and the rig on it, for this module's tests and :player-ui's.
    testFixtures { enable = true }
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
    implementation(project(":i18n"))
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.core.ktx)
    implementation(libs.media3.session)
    implementation(libs.media3.common)

    testFixturesImplementation(project(":store"))
    testFixturesImplementation(libs.coroutines.test)
    // The Compose compiler runs on the fixtures as on the module, so they need its runtime.
    testFixturesImplementation(platform(libs.compose.bom))
    testFixturesImplementation(libs.compose.ui)

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(project(":testkit"))
    testImplementation(libs.robolectric)
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

tasks.withType<Test>().configureEach {
    // Robolectric's Android 16 reaches into the JDK's file descriptors, which the JDK keeps closed.
    jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED", "--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
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
