// The rows of the source picker, in order, for the phone's sheet and the TV's panel alike: Automatic,
// then each source as the engine lists them, with a source's releases under it when it has several.

package tv.anistream.store

import tv.anistream.engine.core.SourceCandidate
import tv.anistream.engine.model.SourceNote
import tv.anistream.engine.model.SourceOption

/** One row of the picker. */
sealed interface PickerRow {
    /** A stable key for lists and focus. */
    val key: String

    /** Let the engine pick per episode. */
    data class Automatic(val chosen: Boolean) : PickerRow {
        override val key get() = "automatic"
    }

    /** One source with what it has of the title. Dimmed when it has a note instead of a count. */
    data class Source(val option: SourceOption, val chosen: Boolean) : PickerRow {
        override val key get() = "source/${option.providerId}"
        val dimmed: Boolean get() = option.note != null

        /** A source that is no longer installed cannot be picked. */
        val selectable: Boolean get() = option.note != SourceNote.Gone
    }

    /** The heading over one source's releases. */
    data class ReleasesHeading(val providerId: String) : PickerRow {
        override val key get() = "releases/$providerId"
    }

    /** One release of a source. */
    data class Release(val option: SourceOption, val candidate: SourceCandidate, val chosen: Boolean) : PickerRow {
        override val key get() = "release/${option.providerId}/${candidate.id}"
    }
}

/** The picker's rows for [options], or just Automatic while nothing has come. */
fun pickerRows(options: List<SourceOption>?): List<PickerRow> {
    val automatic = options?.none { it.pinned } ?: false
    val rows = mutableListOf<PickerRow>(PickerRow.Automatic(automatic))
    for (option in options.orEmpty()) {
        val shown = option.pinned && option.pinnedRelease?.let { pin -> option.releases.any { it.id == pin } } == true
        rows += PickerRow.Source(option, chosen = option.pinned && !shown)
        if (option.releases.size > 1) {
            rows += PickerRow.ReleasesHeading(option.providerId)
            for (release in option.releases) {
                rows += PickerRow.Release(option, release, chosen = option.pinned && option.pinnedRelease == release.id)
            }
        }
    }
    return rows
}
