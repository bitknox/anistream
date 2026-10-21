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
        // A release takes both from its tag (packaging/apps/version.sh), which CI passes as the
        // properties ANISTREAM_VERSION_CODE and ANISTREAM_VERSION_NAME; a local build keeps these.
        versionCode = providers.gradleProperty("ANISTREAM_VERSION_CODE").map(String::toInt).getOrElse(1)
        versionName = providers.gradleProperty("ANISTREAM_VERSION_NAME").getOrElse("1.1.0")
        ndk { abiFilters += "arm64-v8a" }
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

dependencies {
    implementation(project(":ui"))
    implementation(project(":player"))
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.process)
    implementation(libs.browser)
}
