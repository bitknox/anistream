// The app: the activity, the application that holds the engine, and each flavour's launch.
plugins {
    id("anistream.android.application")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
}

android {
    namespace = "tv.anistream.app"
    // The NDK strips the engine's library as the APK is packaged.
    ndkVersion = libs.versions.ndk.get()
    defaultConfig {
        versionCode = 1
        versionName = "1.1.0"
        ndk { abiFilters += "arm64-v8a" }
    }
    // The release key, from Gradle properties (~/.gradle/gradle.properties on the machine that
    // signs). Where they are not set, as in CI, the release build is left unsigned rather than
    // failing. The App Link AniList's sign-in comes back on is verified against this key.
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
    productFlavors {
        // The app flavour's release is the build to keep; the fixtures flavour is never for real use.
        named("app") {
            if (signsReleases) signingConfig = signingConfigs.getByName("release")
        }
    }
    testOptions {
        // Robolectric reads the app's own resources: the shortcuts, the strings.
        unitTests.isIncludeAndroidResources = true
    }
    buildTypes {
        release {
            isMinifyEnabled = false
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
    implementation(project(":player"))
    implementation(project(":widgets"))
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.process)
    implementation(libs.browser)
    implementation(libs.work.runtime)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.coroutines.test)
    testImplementation(libs.work.testing)
    // The scenes the engine recorded, for the settings it says.
    testImplementation(project(":testkit"))
}

tasks.withType<Test>().configureEach {
    // Robolectric's Android 16 reaches into the JDK's file descriptors, which the JDK keeps closed.
    jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED", "--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
    // No engine starts in these tests; HOME is still one of their own, never the user's.
    environment("HOME", layout.buildDirectory.dir("test-home/$name").get().asFile.path)
    environment("ANISTREAM_TOKEN_STORAGE", "file")
}
