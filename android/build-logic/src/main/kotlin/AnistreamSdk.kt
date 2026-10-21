import org.gradle.api.JavaVersion

/** The levels every module builds at. */
object AnistreamSdk {
    /** Android 15: edge to edge and predictive back on by default, which the design assumes. */
    const val MIN = 35

    /** The newest SDK installed. */
    const val COMPILE = 37

    val JAVA = JavaVersion.VERSION_17

    /** The flavour dimension: which engine is inside. */
    const val ENGINE = "engine"

    /** Each flavour's engine build, as android/scripts/engine.sh names it. */
    val engines = mapOf("app" to "release", "fixtures" to "testing")
}
