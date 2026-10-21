// The player spike: one activity that plays through libmpv (android/scripts/mpv.sh) and the JNI
// shim, to answer the plan's spike questions AS1–AS9. See README.md.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "PlayerSpike"
include(":app")
