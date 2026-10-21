// The design system: the Obi & Silk colour scheme, the type, the shapes, the motion tokens,
// Nagare and the cover plate (AnistreamDesign on Apple). Nothing of the engine.
plugins {
    id("anistream.android.library")
    id("anistream.android.compose")
}

android {
    namespace = "tv.anistream.design"
    sourceSets {
        named("main") {
            // The two typefaces and their licences, from android/scripts/fonts.sh.
            res.directories.add("../.fonts/res")
            assets.directories.add("../.fonts/assets")
        }
    }
}

// Every build checks first that the fonts are there, and fails, saying what to run, when not.
val checkFonts = tasks.register<Exec>("checkFonts") {
    description = "Checks that android/scripts/fonts.sh has fetched the fonts."
    commandLine(rootDir.resolve("scripts/fonts.sh").path, "--check")
}
tasks.configureEach {
    if (name == "preBuild") dependsOn(checkFonts)
}
