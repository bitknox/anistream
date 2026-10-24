import java.io.File

// The TV app: Android TV and Google TV, on Android 11 and up, 32-bit and 64-bit ARM. A separate
// APK from the phone's, sharing the engine, the store, the design system and the player. Its
// screens are Compose for TV's, drawn here; it uses neither :ui nor :player-ui.
plugins {
    id("anistream.android.application")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
    // The TV boards drawn on the JVM at 1920 × 1080, from the recorded scenes.
    id("anistream.screenshots")
}

android {
    namespace = "tv.anistream.tv"
    ndkVersion = libs.versions.ndk.get()
    defaultConfig {
        minSdk = AnistreamSdk.TV_MIN
        versionCode = providers.gradleProperty("ANISTREAM_VERSION_CODE").map(String::toInt).getOrElse(1)
        versionName = providers.gradleProperty("ANISTREAM_VERSION_NAME").getOrElse("2.0.0")
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }
    // The phone app's release key, from the same Gradle properties (~/.gradle/gradle.properties on
    // the machine that signs, the ANDROID_RELEASE_* secrets in CI). Where they are not set the
    // release build is left unsigned, for the release workflow to sign.
    val release = listOf("STORE_FILE", "STORE_PASSWORD", "KEY_ALIAS", "KEY_PASSWORD")
        .associateWith { providers.gradleProperty("ANISTREAM_RELEASE_$it").orNull }
    val signsReleases = release.values.all { !it.isNullOrBlank() }
    signingConfigs {
        if (signsReleases) {
            create("release") {
                storeFile = file(release.getValue("STORE_FILE")!!.replaceFirst("~", System.getProperty("user.home")))
                storePassword = release.getValue("STORE_PASSWORD")
                keyAlias = release.getValue("KEY_ALIAS")
                keyPassword = release.getValue("KEY_PASSWORD")
            }
        }
    }
    // Its own ids, so it installs beside the phone app on a device that takes both.
    productFlavors {
        named("app") {
            applicationId = "tv.anistream.tv"
            if (signsReleases) signingConfig = signingConfigs.getByName("release")
        }
        named("fixtures") { applicationId = "tv.anistream.tv.fixtures" }
    }
    packaging {
        // The engine's library is loaded straight out of the APK, page-aligned and uncompressed.
        jniLibs.useLegacyPackaging = false
    }
    testOptions {
        unitTests.isIncludeAndroidResources = true
        // Android's own classes answer with their defaults in a unit test, rather than throwing.
        unitTests.isReturnDefaultValues = true
    }
}

// The app's debug build installs beside its release one, as tv.anistream.tv.debug, so a TV with
// the real app keeps it, its sign-ins and its stores while a debuggable build is tried on it.
androidComponents {
    onVariants(selector().withBuildType("debug").withFlavor(AnistreamSdk.ENGINE to "app")) { variant ->
        variant.applicationId.set("tv.anistream.tv.debug")
    }
}

dependencies {
    implementation(project(":player"))
    implementation(project(":store"))
    implementation(project(":design"))
    implementation(libs.tv.material)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.process)
    // The pairing screen's QR code.
    implementation(libs.zxing.core)
    // The launcher's Watch Next row.
    implementation(libs.tvprovider)
    // Compiles src/main/baseline-prof.txt ahead of time on install, sideloaded or not.
    implementation(libs.profileinstaller)
    implementation(libs.core.ktx)
    implementation(libs.coroutines.android)

    testImplementation(project(":testkit"))
    testImplementation(testFixtures(project(":player")))
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    // The activity a compose rule's content is set in.
    testImplementation(libs.compose.ui.test.manifest)
    // JNA's JVM build, with this machine's native library: Search asks the engine's model for the
    // words of its filters, as the app does.
    testImplementation(libs.jna.jvm)
}

// The engine's model on this machine's build of it, android/.engine/<engine>/host; no engine runs.
AnistreamSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        systemProperty("jna.library.path", library.path)
        // JNA loads its native library itself, which Java now asks to be allowed.
        jvmArgs("--enable-native-access=ALL-UNNAMED")
    }
}

// The tests run under a home of their own, a new directory under build/, never the user's.
tasks.withType<Test>().configureEach {
    val home = layout.buildDirectory.dir("test-home/$name").get().asFile
    val userHome = System.getProperty("user.home")
    environment("HOME", home.path)
    environment("ANISTREAM_TOKEN_STORAGE", "file")
    doFirst {
        check(home.canonicalPath != File(userHome).canonicalPath) { "the tests' HOME would be the user's own" }
        home.deleteRecursively()
        home.mkdirs()
    }
}
