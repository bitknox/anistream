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

dependencies {
    implementation(project(":ui"))
    implementation(project(":player"))
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.process)
    implementation(libs.browser)
}
