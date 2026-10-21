// mpv's values, as mpv_node is: what the shim hands over as JSON, read and written here.

package tv.anistream.player

/**
 * One of mpv's values, the shapes an mpv_node takes. The shim writes each as JSON, and reads JSON
 * back into a node, so this is what both sides of it are made of.
 */
sealed interface MpvValue {
    data object None : MpvValue
    data class Flag(val value: Boolean) : MpvValue
    data class Int64(val value: Long) : MpvValue
    data class Real(val value: Double) : MpvValue
    data class Text(val value: String) : MpvValue
    data class List(val items: kotlin.collections.List<MpvValue>) : MpvValue
    data class Map(val entries: kotlin.collections.Map<String, MpvValue>) : MpvValue

    /** A number, whole or not. */
    val double: Double?
        get() = when (this) {
            is Int64 -> value.toDouble()
            is Real -> value
            else -> null
        }

    /** A whole number, or a number with nothing after the point. */
    val long: Long?
        get() = when (this) {
            is Int64 -> value
            is Real -> if (value % 1.0 == 0.0) value.toLong() else null
            else -> null
        }

    val bool: Boolean? get() = (this as? Flag)?.value
    val string: String? get() = (this as? Text)?.value

    /** A list's items; nothing for anything else. */
    val array: kotlin.collections.List<MpvValue> get() = (this as? List)?.items.orEmpty()

    /** A map's entry, or [None]. */
    operator fun get(key: String): MpvValue = (this as? Map)?.entries?.get(key) ?: None

    /** As JSON, the way the shim and the engine read it. */
    val json: String get() = StringBuilder().also { write(it) }.toString()

    private fun write(out: StringBuilder) {
        when (this) {
            None -> out.append("null")
            is Flag -> out.append(value)
            is Int64 -> out.append(value)
            is Real -> if (value.isFinite()) out.append(number(value)) else out.append("null")
            is Text -> quote(value, out)
            is List -> {
                out.append('[')
                items.forEachIndexed { i, item ->
                    if (i > 0) out.append(',')
                    item.write(out)
                }
                out.append(']')
            }
            is Map -> {
                out.append('{')
                var first = true
                for ((key, value) in entries) {
                    if (!first) out.append(',')
                    first = false
                    quote(key, out)
                    out.append(':')
                    value.write(out)
                }
                out.append('}')
            }
        }
    }

    companion object {
        /** [json] read, or `null` when it is not one whole JSON value. */
        fun parse(json: String): MpvValue? = try {
            val reader = Reader(json)
            val value = reader.value()
            reader.space()
            if (reader.at == json.length) value else null
        } catch (_: IllegalArgumentException) {
            null
        }

        /** A whole number as one, so `1.0` reads as mpv writes it. */
        private fun number(value: Double): String =
            if (value % 1.0 == 0.0 && kotlin.math.abs(value) < 1e15) value.toLong().toString() else value.toString()

        private fun quote(text: String, out: StringBuilder) {
            out.append('"')
            for (ch in text) {
                when (ch) {
                    '"' -> out.append("\\\"")
                    '\\' -> out.append("\\\\")
                    '\n' -> out.append("\\n")
                    '\r' -> out.append("\\r")
                    '\t' -> out.append("\\t")
                    else -> if (ch < ' ') out.append("\\u%04x".format(ch.code)) else out.append(ch)
                }
            }
            out.append('"')
        }
    }

    private class Reader(val text: String) {
        var at = 0

        fun space() {
            while (at < text.length && text[at] in " \n\r\t") at++
        }

        fun value(): MpvValue {
            space()
            require(at < text.length) { "the end of the text" }
            return when (val ch = text[at]) {
                '"' -> Text(string())
                '{' -> map()
                '[' -> list()
                't' -> word("true", Flag(true))
                'f' -> word("false", Flag(false))
                'n' -> word("null", None)
                else -> if (ch == '-' || ch.isDigit()) number() else throw IllegalArgumentException("$ch at $at")
            }
        }

        private fun word(word: String, value: MpvValue): MpvValue {
            require(text.startsWith(word, at)) { "not $word at $at" }
            at += word.length
            return value
        }

        private fun number(): MpvValue {
            val start = at
            while (at < text.length && text[at] in "+-0123456789.eE") at++
            val number = text.substring(start, at)
            if (number.none { it in ".eE" }) number.toLongOrNull()?.let { return Int64(it) }
            return Real(number.toDoubleOrNull() ?: throw IllegalArgumentException("not a number: $number"))
        }

        private fun string(): String {
            at++
            val out = StringBuilder()
            while (true) {
                require(at < text.length) { "an open string" }
                val ch = text[at++]
                when (ch) {
                    '"' -> return out.toString()
                    '\\' -> {
                        require(at < text.length) { "an open escape" }
                        when (val esc = text[at++]) {
                            '"', '\\', '/' -> out.append(esc)
                            'b' -> out.append('\b')
                            'f' -> out.append('\u000c')
                            'n' -> out.append('\n')
                            'r' -> out.append('\r')
                            't' -> out.append('\t')
                            'u' -> {
                                require(at + 4 <= text.length) { "a short escape" }
                                out.append(text.substring(at, at + 4).toInt(16).toChar())
                                at += 4
                            }
                            else -> throw IllegalArgumentException("\\$esc")
                        }
                    }
                    else -> out.append(ch)
                }
            }
        }

        private fun list(): MpvValue {
            at++
            val items = mutableListOf<MpvValue>()
            space()
            if (at < text.length && text[at] == ']') {
                at++
                return List(items)
            }
            while (true) {
                items += value()
                space()
                require(at < text.length) { "an open list" }
                when (text[at++]) {
                    ',' -> continue
                    ']' -> return List(items)
                    else -> throw IllegalArgumentException("a list at ${at - 1}")
                }
            }
        }

        private fun map(): MpvValue {
            at++
            val entries = linkedMapOf<String, MpvValue>()
            space()
            if (at < text.length && text[at] == '}') {
                at++
                return Map(entries)
            }
            while (true) {
                space()
                require(at < text.length && text[at] == '"') { "a key at $at" }
                val key = string()
                space()
                require(at < text.length && text[at++] == ':') { "a colon" }
                entries[key] = value()
                space()
                require(at < text.length) { "an open map" }
                when (text[at++]) {
                    ',' -> continue
                    '}' -> return Map(entries)
                    else -> throw IllegalArgumentException("a map at ${at - 1}")
                }
            }
        }
    }
}
