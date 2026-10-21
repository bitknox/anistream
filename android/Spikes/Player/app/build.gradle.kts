plugins {
    alias(libs.plugins.android.application)
}

// The repo's root, three levels above this Gradle project (android/Spikes/Player).
val repo = rootDir.resolve("../../..").canonicalFile
// -Pmpv=vulkan builds against `android/scripts/mpv.sh --vulkan`'s libmpv (AS4) instead.
val vulkan = providers.gradleProperty("mpv").orNull == "vulkan"
val mpvDir = repo.resolve(if (vulkan) "android/.mpv/vulkan" else "android/.mpv/current")

android {
    namespace = "tv.anistream.spike.player"
    compileSdk = 37
    buildToolsVersion = libs.versions.buildTools.get()
    ndkVersion = libs.versions.ndk.get()

    defaultConfig {
        applicationId = "tv.anistream.spike.player"
        minSdk = 35
        targetSdk = 37
        versionCode = 1
        versionName = "spike"
        ndk { abiFilters += "arm64-v8a" }
        externalNativeBuild {
            cmake { arguments += listOf("-DANDROID_STL=none", "-DMPV_DIR=${mpvDir.path}") }
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = libs.versions.cmake.get()
        }
    }

    sourceSets["main"].apply {
        // libmpv.so and FFmpeg's libraries, from android/scripts/mpv.sh.
        jniLibs.directories += mpvDir.resolve("jniLibs").path
        // -PengineLibs=<dir with arm64-v8a/libanistream_ffi.so>: the engine beside mpv (AS1).
        providers.gradleProperty("engineLibs").orNull?.let { jniLibs.directories += it }
        // The engine's shaders, copied in by spikeShaders below.
        assets.directories += layout.buildDirectory.dir("generated/spikeAssets").get().asFile.path
    }

    packaging {
        // The shim's CMake imports libmpv and libavcodec, which AGP packages too.
        jniLibs.pickFirsts += listOf("**/libmpv.so", "**/libavcodec.so")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
}

dependencies {
    implementation(libs.media3.session)
    implementation(libs.media3.common)
}

// assets/shaders/ in the APK: the vendored shaders and their licences, as the engine has them.
val spikeShaders = tasks.register<Sync>("spikeShaders") {
    from(repo.resolve("assets/shaders"))
    into(layout.buildDirectory.dir("generated/spikeAssets/shaders"))
}

// Fails the build when libmpv is missing or older than android/scripts/mpv.sh says it should be.
val mpvCheck = tasks.register<Exec>("mpvCheck") {
    commandLine(listOf(repo.resolve("android/scripts/mpv.sh").path, "--check") + if (vulkan) listOf("--vulkan") else emptyList())
}

tasks.named("preBuild") { dependsOn(spikeShaders, mpvCheck) }
