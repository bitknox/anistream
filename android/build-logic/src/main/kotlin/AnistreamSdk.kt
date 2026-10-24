import org.gradle.api.JavaVersion

/** The levels every module builds at. */
object AnistreamSdk {
    /** Android 15: edge to edge and predictive back on by default, which the design assumes. */
    const val MIN = 35

    /**
     * Android 11, for the TV app and the modules it shares with the phone (the engine, the store,
     * the design system and the player): TV boxes stay on old releases, and Android TV 14 is the
     * newest the Chromecast with Google TV has.
     */
    const val TV_MIN = 30

    /** The newest SDK installed. */
    const val COMPILE = 37

    val JAVA = JavaVersion.VERSION_17

    /** The flavour dimension: which engine is inside. */
    const val ENGINE = "engine"

    /** Each flavour's engine build, as android/scripts/engine.sh names it. */
    val engines = mapOf("app" to "release", "fixtures" to "testing")
}
