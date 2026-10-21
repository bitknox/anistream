// The anistream Android app: Kotlin and Jetpack Compose on the engine, through UniFFI.
pluginManagement {
    includeBuild("build-logic")
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

rootProject.name = "anistream"

// The modules follow the Apple app's packages: the engine's bindings and library, the store that
// holds the engine and its state, the design system, the screens, the player, and the app.
include(":engine", ":store", ":design", ":ui", ":player", ":app")
