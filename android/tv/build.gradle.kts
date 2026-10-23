// The TV app: Android TV and Google TV, on Android 11 and up, 32-bit and 64-bit ARM. A separate
// APK from the phone's, sharing the engine, the store, the design system and the player.
plugins {
    id("anistream.android.application")
    id("anistream.engine.flavours")
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
    // Its own ids, so it installs beside the phone app on a device that takes both.
    productFlavors {
        named("app") { applicationId = "tv.anistream.tv" }
        named("fixtures") { applicationId = "tv.anistream.tv.fixtures" }
    }
    packaging {
        // The engine's library is loaded straight out of the APK, page-aligned and uncompressed.
        jniLibs.useLegacyPackaging = false
    }
}

dependencies {
    implementation(project(":player"))
    implementation(libs.core.ktx)
    implementation(libs.coroutines.android)
}
