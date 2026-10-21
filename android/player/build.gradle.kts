// The player: libmpv behind the hosted player protocol, with Compose controls. A shell until the
// Player track brings it over from its spike (AnistreamPlayer on Apple).
plugins {
    id("anistream.android.library")
    id("anistream.engine.flavours")
}

android {
    namespace = "tv.anistream.player"
}

dependencies {
    implementation(project(":store"))
}
