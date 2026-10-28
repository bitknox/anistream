package tv.anistream.store

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.core.SettingValue
import tv.anistream.engine.model.ProviderOrigin
import tv.anistream.engine.model.ProviderRow
import tv.anistream.engine.model.SettingChange
import tv.anistream.engine.model.SettingKey
import tv.anistream.engine.model.SettingRow
import tv.anistream.engine.model.SyncState

/** The ports of the engine's settings and source-order rules, and what the accounts keep. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SettingsTest {
    private val context = RuntimeEnvironment.getApplication()
    private fun row(id: String, kind: String) = ProviderRow(
        id = id, kind = kind, state = "ready", latencyMs = null, lastError = null, usable = true,
        heldBack = false, name = id, version = null, hosts = emptyList(), origin = ProviderOrigin.BuiltIn,
        disabled = false, update = null,
    )

    private fun setting(key: SettingKey, value: String, change: SettingChange = SettingChange.Steps(back = true, forward = true)) =
        SettingRow(key, value, change)

    // The cases of `anistream_model::sources`'s own tests.

    @Test
    fun moving_a_plugin_names_the_plugins_on_screen_and_keeps_plugins_after_them() {
        val rows = listOf(row("torrent", "torrent"), row("alpha", "plugin"), row("beta", "plugin"))
        val order = listOf("torrent", "plugins")
        assertEquals(listOf("torrent", "beta", "alpha", "plugins"), SourceOrder.moved(order, rows, "beta", -1))
        assertEquals(listOf("alpha", "torrent", "beta", "plugins"), SourceOrder.moved(order, rows, "alpha", -1))
        assertEquals(listOf("alpha", "torrent", "beta", "plugins"), SourceOrder.moved(order, rows, "torrent", 1))
    }

    @Test
    fun a_move_steps_over_entries_with_no_row_and_keeps_them() {
        val rows = listOf(row("remote", "remote"), row("alpha", "plugin"))
        assertEquals(
            listOf("torrent", "alpha", "remote", "gamma", "plugins"),
            SourceOrder.moved(listOf("torrent", "remote", "gamma", "plugins"), rows, "alpha", -1),
        )
        assertEquals(listOf("plugins", "remote", "alpha"), SourceOrder.moved(listOf("alpha", "plugins", "remote"), rows, "alpha", 1))
    }

    @Test
    fun a_move_past_either_end_or_of_no_row_changes_nothing() {
        val rows = listOf(row("torrent", "torrent"), row("alpha", "plugin"))
        val order = listOf("torrent", "plugins", "remote")
        assertNull(SourceOrder.moved(order, rows, "torrent", -1))
        assertNull(SourceOrder.moved(order, rows, "alpha", 1))
        assertNull(SourceOrder.moved(order, rows, "nothing", 1))
    }

    @Test
    fun a_drag_writes_what_as_many_steps_would() {
        val rows = listOf(row("alpha", "plugin"), row("beta", "plugin"), row("torrent", "torrent"))
        val order = listOf("plugins", "torrent")
        assertEquals(listOf("torrent", "alpha", "beta", "plugins"), SourceOrder.movedTo(order, rows, "torrent", 0))
        assertNull(SourceOrder.movedTo(order, rows, "alpha", 0))
    }

    @Test
    fun the_rows_sort_by_the_order_with_unnamed_plugins_where_plugins_stands() {
        val rows = listOf(row("torrent", "torrent"), row("alpha", "plugin"), row("beta", "plugin"))
        assertEquals(listOf("beta", "alpha", "torrent"), SourceOrder.sorted(rows, listOf("beta", "plugins", "torrent")).map { it.id })
        assertEquals(listOf("torrent", "alpha", "beta"), SourceOrder.sorted(rows, listOf("torrent")).map { it.id })
    }

    @Test
    fun a_row_stands_on_the_option_its_words_name() {
        assertEquals(SettingValue.Int(1080), SettingsCatalog.current(setting(SettingKey.QUALITY, "1080p"))?.value)
        assertEquals(SettingValue.Str("natural"), SettingsCatalog.current(setting(SettingKey.UPSCALING, "natural · ArtCNN"))?.value)
        // Sub or dub leads with the saved choice, and the session's beside it.
        assertEquals(SettingValue.Str("sub"), SettingsCatalog.current(setting(SettingKey.TRANSLATION, "sub · dub this session"))?.value)
        assertEquals(SettingValue.Str("anilist"), SettingsCatalog.current(setting(SettingKey.TITLES, "AniList's choice"))?.value)
        assertEquals(SettingValue.Int(3), SettingsCatalog.current(setting(SettingKey.KEEP_NEXT_EPISODES, "3 episodes"))?.value)
        assertEquals(SettingValue.Float(1.0), SettingsCatalog.current(setting(SettingKey.SEED_RATIO, "until ratio 1.0"))?.value)
        assertEquals("524 kB/s", SettingsCatalog.uploadLabel(512))
        assertTrue(SettingsCatalog.isOn(setting(SettingKey.AUTO_NEXT, "on")))
        assertFalse(SettingsCatalog.editable(setting(SettingKey.VPN_MODE, "external", SettingChange.Fixed)))
    }

    @Test
    fun sizes_read_as_the_terminal_says_them() {
        assertEquals("0 B", Wording.bytes(context, 0))
        assertEquals("140 MB", Wording.bytes(context, 140_300_000))
        assertEquals("9.4 GB", Wording.bytes(context, 9_400_000_000))
        assertEquals("3.4 GB", Wording.bytes(context, 3_400_000_000))
    }

    @Test
    fun a_sync_with_nothing_left_to_send_is_when_it_last_synced() {
        var now = 1_000L
        val accounts = AccountsModel(clock = { now })
        val state = SyncState(
            tracker = "anilist", connected = true, user = "Mika", storage = "file", storageDegraded = false,
            outbox = 2u, deferred = 0u, needsReauth = false, last = "pushed 1", pullError = null,
        )
        accounts.apply(state)
        assertNull(accounts.syncedAt.value["anilist"])
        now = 5_000L
        accounts.apply(state.copy(outbox = 0u))
        assertEquals(5_000L, accounts.syncedAt.value["anilist"])
        accounts.apply(state.copy(connected = false))
        assertNull(accounts.syncedAt.value["anilist"])
    }
}
