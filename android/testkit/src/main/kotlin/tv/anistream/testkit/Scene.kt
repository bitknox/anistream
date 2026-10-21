// A scene: what the testing engine sent for one of its scenes, recorded once and replayed as often
// as a test likes, at the moment it was recorded.

package tv.anistream.testkit

import java.io.File
import java.time.Instant
import kotlin.reflect.typeOf
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import tv.anistream.engine.model.Event

/**
 * The events the testing engine sent for a scene, in order, and when they were recorded: the
 * engine's relative times ("out 2h ago") were counted from [recordedAt], so a screen replaying
 * them reads its clock there too.
 *
 * Art is not in it. The fixture titles' images are the Rust test kit's, which [FixtureArt] reads,
 * and the store asks for as its screens draw them.
 */
class Scene(val name: String, val recordedAt: Instant, val events: List<Event>) {
    /**
     * The scene as JSON, with the fake AniList's art at the fixtures' own address rather than the
     * port it happened to listen on, so recording again changes only what the engine sent.
     */
    fun toJson(): String = json.encodeToString(
        JsonElement.serializer(),
        JsonObject(
            mapOf(
                "scene" to JsonPrimitive(name),
                "recordedAt" to JsonPrimitive(recordedAt.toEpochMilli()),
                "events" to EventJson.encode(events, typeOf<List<Event>>()),
            ),
        ),
    ).replace(FixtureArt.SERVED, FixtureArt.HOST)

    companion object {
        @OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
        private val json = Json {
            prettyPrint = true
            prettyPrintIndent = " "
        }

        /** The scene recorded as [name]: `fresh` or `watching`. */
        fun named(name: String): Scene {
            val resource = Scene::class.java.getResource("scenes/$name.json")
                ?: error("no scene $name: android/scripts/scenes.sh records them")
            return parse(resource.readText())
        }

        fun parse(text: String): Scene {
            val root = Json.parseToJsonElement(text).jsonObject
            @Suppress("UNCHECKED_CAST")
            val events = EventJson.decode(root.getValue("events"), typeOf<List<Event>>()) as List<Event>
            return Scene(
                name = root.getValue("scene").jsonPrimitive.content,
                recordedAt = Instant.ofEpochMilli(root.getValue("recordedAt").jsonPrimitive.long),
                events = events,
            )
        }

        fun write(scene: Scene, directory: File) {
            directory.mkdirs()
            File(directory, "${scene.name}.json").writeText(scene.toJson() + "\n")
        }
    }
}

/**
 * The fixture titles' art, as the fake AniList serves it: the Rust test kit's drawings, a poster
 * and a wide one per title, at the paths the fixtures' image URLs name (`art.rs`).
 */
object FixtureArt {
    const val HOST = "https://img.example/"

    /** Where the fake AniList serves the same art from, on whatever port it took. */
    val SERVED = Regex("""http://127\.0\.0\.1:\d+/art/""")

    private val names = mapOf(
        900001u to "stardust",
        900002u to "library",
        900003u to "platform",
        900004u to "stardust-2",
        900005u to "gardener",
        900006u to "relay",
    )

    /** crates/anistream-testkit/fixtures/art, found from wherever the test runs in the repo. */
    val directory: File by lazy {
        System.getProperty("anistream.testkit.art")?.let(::File)
            ?: generateSequence(File("").absoluteFile) { it.parentFile }
                .map { File(it, "crates/anistream-testkit/fixtures/art") }
                .firstOrNull { it.isDirectory }
            ?: error("no crates/anistream-testkit/fixtures/art above ${File("").absolutePath}")
    }

    /**
     * The PNG for [url], or `null` for one the fixtures do not name: `cover/900001-xl.jpg` is the
     * poster, `banner/900001.jpg` and a still, `ep/900001/3.jpg`, the wide drawing.
     */
    fun bytes(url: String): ByteArray? {
        if (!url.startsWith(HOST)) return null
        val parts = url.removePrefix(HOST).split('/')
        val id = parts.getOrNull(1)?.split('-', '.')?.firstOrNull()?.toUIntOrNull() ?: return null
        val name = names[id] ?: return null
        val kind = when (parts[0]) {
            "cover" -> "poster"
            "banner", "ep" -> "wide"
            else -> return null
        }
        return File(directory, "$name-$kind.png").readBytes()
    }
}
