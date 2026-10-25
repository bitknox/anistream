// The app: the activity, the application that holds the engine, and each flavour's launch.
plugins {
    id("anistream.android.application")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
}

/**
 * The update test's keystore (android/scripts/update-test.sh): with it, the app flavour is signed
 * with a key the test made, never the release key, so one test build installs over another on an
 * emulator; it carries src/updateTest, the receiver that offers it a release, and fetches over
 * plain HTTP from this Mac. Never set for a build to keep.
 */
val updateTestKey: String? = providers.gradleProperty("ANISTREAM_UPDATE_TEST_KEYSTORE").orNull?.takeIf { it.isNotBlank() }
/** The test key's passwords, which keep nothing: the key is made for the test and thrown away. */
val UPDATE_TEST_PASSWORD = "update-test"

android {
    namespace = "tv.anistream.app"
    // The NDK strips the engine's library as the APK is packaged.
    ndkVersion = libs.versions.ndk.get()
    defaultConfig {
        // A release takes both from its tag (packaging/apps/version.sh), which CI passes as the
        // properties ANISTREAM_VERSION_CODE and ANISTREAM_VERSION_NAME; a local build keeps these.
        versionCode = providers.gradleProperty("ANISTREAM_VERSION_CODE").map(String::toInt).getOrElse(1)
        versionName = providers.gradleProperty("ANISTREAM_VERSION_NAME").getOrElse("2.0.0")
        ndk { abiFilters += "arm64-v8a" }
        // The feature videos' flows, src/androidTestFixtures (website/scripts/videos).
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    // The release key, from Gradle properties (~/.gradle/gradle.properties on the machine that
    // signs). Where they are not set, as in CI, the release build is left unsigned rather than
    // failing. The App Link AniList's sign-in comes back on is verified against this key.
    val release = listOf("STORE_FILE", "STORE_PASSWORD", "KEY_ALIAS", "KEY_PASSWORD")
        .associateWith { providers.gradleProperty("ANISTREAM_RELEASE_$it").orNull }
    val signsReleases = release.values.all { !it.isNullOrBlank() }
    signingConfigs {
        // An update test's build never reads the release key.
        if (signsReleases && updateTestKey == null) {
            create("release") {
                storeFile = file(release.getValue("STORE_FILE")!!.replaceFirst("~", System.getProperty("user.home")))
                storePassword = release.getValue("STORE_PASSWORD")
                keyAlias = release.getValue("KEY_ALIAS")
                keyPassword = release.getValue("KEY_PASSWORD")
            }
        }
        // The update test's key (android/scripts/update-test.sh), which the test makes for itself.
        updateTestKey?.let { key ->
            create("updateTest") {
                storeFile = file(key)
                storePassword = UPDATE_TEST_PASSWORD
                keyAlias = "update-test"
                keyPassword = UPDATE_TEST_PASSWORD
            }
        }
    }
    productFlavors {
        // The app flavour's release is the build to keep; the fixtures flavour is never for real use.
        named("app") {
            if (updateTestKey != null) {
                signingConfig = signingConfigs.getByName("updateTest")
            } else if (signsReleases) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
    buildFeatures { buildConfig = true }
    defaultConfig {
        // Whether this is an update test's build: plain HTTP for the APK it fetches from this Mac.
        buildConfigField("boolean", "UPDATE_TEST", (updateTestKey != null).toString())
    }
    testOptions {
        // Robolectric reads the app's own resources: the shortcuts, the strings.
        unitTests.isIncludeAndroidResources = true
    }
    buildTypes {
        release {
            // R8 shrinks and optimizes the app around the engine's bindings, which it keeps whole
            // (proguard-rules.pro): Compose and the rest start much faster optimized.
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    packaging {
        // The engine's library is loaded straight out of the APK, page-aligned and uncompressed.
        jniLibs.useLegacyPackaging = false
    }
}

/**
 * The launcher's static shortcuts, res/xml/shortcuts.xml, written from shortcuts.xml beside this
 * file with the variant's application id in it: the intents name the package that opens them, and
 * the system reads that as written, not as a resource.
 */
abstract class ShortcutsXml : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val template: RegularFileProperty

    @get:Input
    abstract val applicationId: Property<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun write() {
        val xml = outputDir.get().dir("xml").asFile.apply { mkdirs() }.resolve("shortcuts.xml")
        xml.writeText(template.get().asFile.readText().replace("\${applicationId}", applicationId.get()))
    }
}

androidComponents {
    onVariants(selector().withFlavor(AnistreamSdk.ENGINE to "app")) { variant ->
        if (updateTestKey != null) {
            variant.sources.manifests.addStaticManifestFile("src/updateTest/AndroidManifest.xml")
            variant.sources.kotlin?.addStaticSourceDirectory("src/updateTest/kotlin")
        }
    }
    onVariants { variant ->
        val shortcuts = tasks.register<ShortcutsXml>("write${variant.name.replaceFirstChar(Char::uppercase)}Shortcuts") {
            template.set(layout.projectDirectory.file("shortcuts.xml"))
            applicationId.set(variant.applicationId)
        }
        variant.sources.res?.addGeneratedSourceDirectory(shortcuts, ShortcutsXml::outputDir)
    }
}

dependencies {
    implementation(project(":ui"))
    implementation(project(":player-ui"))
    implementation(project(":widgets"))
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.process)
    // The TV's media session while Play on TV drives one.
    implementation(libs.media3.session)
    implementation(libs.media3.common)
    implementation(libs.browser)
    implementation(libs.work.runtime)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.coroutines.test)
    testImplementation(libs.work.testing)
    // The scenes the engine recorded, for the settings it says.
    testImplementation(project(":testkit"))

    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.test.uiautomator)
}

tasks.withType<Test>().configureEach {
    // Robolectric's Android 16 reaches into the JDK's file descriptors, which the JDK keeps closed.
    jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED", "--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
    // No engine starts in these tests; HOME is still one of their own, never the user's.
    environment("HOME", layout.buildDirectory.dir("test-home/$name").get().asFile.path)
    environment("ANISTREAM_TOKEN_STORAGE", "file")
}
