// A module with Compose in it.
plugins {
    id("org.jetbrains.kotlin.plugin.compose")
}

pluginManager.withPlugin("com.android.library") {
    extensions.configure<com.android.build.api.dsl.LibraryExtension> {
        buildFeatures { compose = true }
    }
}
pluginManager.withPlugin("com.android.application") {
    extensions.configure<com.android.build.api.dsl.ApplicationExtension> {
        buildFeatures { compose = true }
    }
}

val libs = extensions.getByType<VersionCatalogsExtension>().named("libs")
fun lib(name: String) = libs.findLibrary(name).get()
dependencies {
    "implementation"(platform(lib("compose-bom")))
    "implementation"(lib("compose-ui"))
    "implementation"(lib("compose-foundation"))
    "implementation"(lib("compose-material3"))
    "implementation"(lib("compose-ui-tooling-preview"))
    "debugImplementation"(lib("compose-ui-tooling"))
}
