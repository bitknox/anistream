// The engine's events as JSON, so a scene recorded from the testing engine can be replayed into the
// store without it. Read and written by reflection over the generated bindings, so a field the
// engine adds is recorded with no change here.

package tv.anistream.testkit

import java.time.Duration
import java.time.Instant
import java.util.Base64
import kotlin.reflect.KClass
import kotlin.reflect.KType
import kotlin.reflect.full.memberProperties
import kotlin.reflect.full.primaryConstructor
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * Values of the bindings' types as JSON, and back, by their Kotlin types.
 *
 * - Records are objects of their fields, by name.
 * - A sealed type's variant carries its name in `"@type"`; a variant without fields is just that.
 * - Enums are their names, and unsigned numbers and ULongs decimal, as are Longs past 2^53.
 * - Lists are arrays, maps arrays of `[key, value]` pairs, byte arrays base64.
 * - Durations and instants are their ISO-8601 forms.
 */
object EventJson {
    private const val TYPE = "@type"

    fun encode(value: Any?, type: KType): JsonElement {
        if (value == null) return JsonNull
        val declared = type.classifier as? KClass<*>
        return when (value) {
            is String -> JsonPrimitive(value)
            is Boolean -> JsonPrimitive(value)
            is Int, is Short, is Byte -> JsonPrimitive(value as Number)
            is Long -> JsonPrimitive(value.toString())
            is Double -> if (value.isFinite()) JsonPrimitive(value) else JsonPrimitive(value.toString())
            is Float -> if (value.isFinite()) JsonPrimitive(value) else JsonPrimitive(value.toString())
            is UInt, is UShort, is UByte, is ULong -> JsonPrimitive(value.toString())
            is ByteArray -> JsonPrimitive(Base64.getEncoder().encodeToString(value))
            is Duration, is Instant -> JsonPrimitive(value.toString())
            is Enum<*> -> JsonPrimitive(value.name)
            is List<*> -> {
                val element = type.arguments.firstOrNull()?.type ?: error("a list without its element type")
                JsonArray(value.map { encode(it, element) })
            }
            is Map<*, *> -> {
                val key = type.arguments[0].type ?: error("a map without its key type")
                val item = type.arguments[1].type ?: error("a map without its value type")
                JsonArray(value.map { (k, v) -> JsonArray(listOf(encode(k, key), encode(v, item))) })
            }
            else -> record(value, variantOf = declared?.takeIf { it != value::class })
        }
    }

    /** A record's fields, with its variant's name when [variantOf] is the sealed type it is one of. */
    private fun record(value: Any, variantOf: KClass<*>?): JsonElement {
        val klass = value::class
        val fields = linkedMapOf<String, JsonElement>()
        if (variantOf != null) fields[TYPE] = JsonPrimitive(variantName(klass))
        if (klass.objectInstance == null) {
            val constructor = klass.primaryConstructor ?: error("${klass.qualifiedName} has no primary constructor")
            val properties = klass.memberProperties.associateBy { it.name }
            for (parameter in constructor.parameters) {
                val name = parameter.name ?: continue
                val property = properties[name] ?: error("${klass.qualifiedName}.$name is not a property")
                fields[name] = encode(property.getter.call(value), parameter.type)
            }
        }
        return JsonObject(fields)
    }

    /** A variant's name inside its sealed type: `Content` for `Event.Content`. */
    private fun variantName(klass: KClass<*>): String = klass.simpleName ?: error("an anonymous variant")

    @Suppress("UNCHECKED_CAST")
    fun decode(json: JsonElement, type: KType): Any? {
        if (json is JsonNull) {
            check(type.isMarkedNullable) { "null for $type" }
            return null
        }
        val klass = type.classifier as? KClass<*> ?: error("no class for $type")
        return when (klass) {
            String::class -> json.jsonPrimitive.content
            Boolean::class -> json.jsonPrimitive.content.toBooleanStrict()
            Int::class -> json.jsonPrimitive.content.toInt()
            Short::class -> json.jsonPrimitive.content.toShort()
            Byte::class -> json.jsonPrimitive.content.toByte()
            Long::class -> json.jsonPrimitive.content.toLong()
            Double::class -> json.jsonPrimitive.content.toDouble()
            Float::class -> json.jsonPrimitive.content.toFloat()
            UInt::class -> json.jsonPrimitive.content.toUInt()
            UShort::class -> json.jsonPrimitive.content.toUShort()
            UByte::class -> json.jsonPrimitive.content.toUByte()
            ULong::class -> json.jsonPrimitive.content.toULong()
            ByteArray::class -> Base64.getDecoder().decode(json.jsonPrimitive.content)
            Duration::class -> Duration.parse(json.jsonPrimitive.content)
            Instant::class -> Instant.parse(json.jsonPrimitive.content)
            List::class -> {
                val element = type.arguments.first().type!!
                json.jsonArray.map { decode(it, element) }
            }
            Map::class -> {
                val key = type.arguments[0].type!!
                val item = type.arguments[1].type!!
                json.jsonArray.associate { pair -> decode(pair.jsonArray[0], key) to decode(pair.jsonArray[1], item) }
            }
            else -> when {
                klass.java.isEnum -> klass.java.enumConstants!!.first { (it as Enum<*>).name == json.jsonPrimitive.content }
                klass.isSealed -> {
                    val name = json.jsonObject[TYPE]?.jsonPrimitive?.content ?: error("no $TYPE for a ${klass.simpleName}")
                    val variant = variants(klass).firstOrNull { variantName(it) == name }
                        ?: error("${klass.simpleName} has no variant $name")
                    construct(variant, json.jsonObject)
                }
                else -> construct(klass, json.jsonObject)
            }
        }
    }

    private fun variants(klass: KClass<*>): List<KClass<*>> =
        klass.sealedSubclasses.flatMap { if (it.isSealed) variants(it) else listOf(it) }

    private fun construct(klass: KClass<*>, json: JsonObject): Any {
        klass.objectInstance?.let { return it }
        val constructor = klass.primaryConstructor ?: error("${klass.qualifiedName} has no primary constructor")
        // A field the scene was recorded without, from a newer engine, is null when it can be.
        val arguments = constructor.parameters.mapNotNull { parameter ->
            val value = json[parameter.name]
            when {
                value != null -> parameter to decode(value, parameter.type)
                parameter.isOptional -> null
                parameter.type.isMarkedNullable -> parameter to null
                else -> error("${klass.qualifiedName}.${parameter.name} is missing: record the scenes again")
            }
        }.toMap()
        return constructor.callBy(arguments)
    }
}
