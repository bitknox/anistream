// A module with JVM screenshot tests: Robolectric draws its screens with the app's resources, and
// Roborazzi captures them to android/.shots/roborazzi/, beside the emulator's shots, which git
// ignores.
plugins {
    id("io.github.takahirom.roborazzi")
}

pluginManager.withPlugin("com.android.library") {
    extensions.configure<com.android.build.api.dsl.LibraryExtension> {
        testOptions {
            unitTests.isIncludeAndroidResources = true
        }
    }
}

extensions.configure<io.github.takahirom.roborazzi.RoborazziExtension> {
    outputDir.set(rootDir.resolve(".shots/roborazzi"))
}

// Robolectric's Android 16 reaches into the JDK's file descriptors, which the JDK keeps closed.
tasks.withType<Test>().configureEach {
    jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED", "--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
}
