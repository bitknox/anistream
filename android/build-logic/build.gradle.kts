plugins {
    `kotlin-dsl`
}

dependencies {
    // On the classpath of every module's build: the Android plugin, and the Kotlin release the
    // app is compiled with, which takes the place of the one the Android plugin brings.
    implementation(libs.android.gradle.plugin)
    implementation(libs.kotlin.gradle.plugin)
    implementation(libs.compose.gradle.plugin)
    // The screens' screenshot tests.
    implementation(libs.roborazzi.gradle.plugin)
}
