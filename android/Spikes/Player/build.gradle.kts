plugins {
    alias(libs.plugins.android.application) apply false
    // AGP 9 compiles Kotlin itself; naming the plugin here picks the Kotlin it uses.
    alias(libs.plugins.kotlin.android) apply false
}
