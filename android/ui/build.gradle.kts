// The screens: the navigation bar and the destinations behind it (AnistreamUI on Apple).
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
    id("anistream.engine.flavours")
}

android {
    namespace = "tv.anistream.ui"
}

dependencies {
    api(project(":store"))
    api(project(":design"))
    implementation(libs.navigation.compose)
    implementation(libs.adaptive.layout)
    implementation(libs.adaptive.navigation)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.activity.compose)
}
