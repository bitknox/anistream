// The app module: Android 15 and up, built against and targeting the newest SDK.
plugins {
    id("com.android.application")
}

android {
    compileSdk = AnistreamSdk.COMPILE
    defaultConfig {
        minSdk = AnistreamSdk.MIN
        targetSdk = AnistreamSdk.COMPILE
    }
    compileOptions {
        sourceCompatibility = AnistreamSdk.JAVA
        targetCompatibility = AnistreamSdk.JAVA
    }
    buildTypes {
        // The feature videos' build (website/scripts/videos/android.sh): the debug build's key and
        // settings, but not debuggable, so ART compiles it as it does a release and Compose keeps
        // the display's rate on the recording. The libraries take their release build.
        create("video") {
            initWith(getByName("debug"))
            isDebuggable = false
            matchingFallbacks += "release"
        }
    }
    // android.sh runs the flows against the video build with -Panistream.testBuildType=video.
    testBuildType = providers.gradleProperty("anistream.testBuildType").getOrElse("debug")
}

// Only the fixtures flavour is filmed: no video build of the app flavour, whose ids are the real
// app's.
androidComponents {
    beforeVariants(selector().withBuildType("video").withFlavor(AnistreamSdk.ENGINE to "app")) { it.enable = false }
}
