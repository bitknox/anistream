// The fixtures flavour's update: a made-up release on the update's page, at a stage the shots
// name. Nothing is checked for, fetched or installed: the "download" stops part way, or fails.

package tv.anistream.tv

import android.content.Intent
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import tv.anistream.store.ApkFacts
import tv.anistream.store.ApkFetch
import tv.anistream.store.UpdateFiles
import tv.anistream.store.UpdateHost
import tv.anistream.store.UpdateModel
import tv.anistream.store.UpdateOffer

/** Where the staged update stands once its page opens. */
enum class UpdateStage {
    /** Offered, nothing pressed. */
    AVAILABLE,

    /** Update pressed, and "Install unknown apps" is off. */
    ALLOWING,

    /** Downloading, two fifths of the way. */
    DOWNLOADING,

    /** The download stopped: the connection went. */
    FAILED,

    /** Too little room for it. */
    STORAGE,
    ;

    companion object {
        fun named(name: String?): UpdateStage? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

class StagedUpdate(
    private val model: UpdateModel,
    private val scope: CoroutineScope,
    private val stage: UpdateStage,
    /** Where the model keeps its files: nothing is written there but an empty folder. */
    dir: File,
) : UpdateHost, UpdateLauncher {
    private val _launch = MutableStateFlow<Intent?>(null)
    override val launch: StateFlow<Intent?> = _launch.asStateFlow()

    override fun launched(intent: Intent, started: Boolean) {
        _launch.compareAndSet(intent, null)
    }

    override fun resumed() = model.resumed()

    private var download: Job? = null

    override val packageName: String get() = TV_PACKAGE

    override fun hasRoom(offer: UpdateOffer): Boolean = stage != UpdateStage.STORAGE

    override fun canInstall(): Boolean = stage != UpdateStage.ALLOWING

    // Android's page is not opened: the stage stays on the explanation.
    override fun openInstallSettings(offer: UpdateOffer) = Unit

    override fun returnedFromSettings() = Unit

    override fun startDownload(offer: UpdateOffer): Boolean {
        download = scope.launch { model.run(StagedFetch(stage)) }
        return true
    }

    override fun stopDownload() {
        download?.cancel()
    }

    override fun inspect(file: File): ApkFacts? = null

    override fun installed(): ApkFacts = ApkFacts(TV_PACKAGE, 1, emptySet())

    override fun install(file: File, offer: UpdateOffer) = throw IOException("the fixtures install nothing")

    override fun abandonInstall() = Unit

    init {
        model.attach(this, UpdateFiles(dir))
        model.offer(OFFER)
        if (stage != UpdateStage.AVAILABLE) model.press()
    }

    /** Two fifths of the APK, then it waits, or the connection goes. */
    private class StagedFetch(private val stage: UpdateStage) : ApkFetch {
        override suspend fun fetch(url: String, into: File, expected: Long, progress: (done: Long, total: Long) -> Unit) {
            progress(expected * 2 / 5, expected)
            if (stage == UpdateStage.FAILED) throw IOException("the connection was lost")
            awaitCancellation()
        }
    }

    companion object {
        /** The made-up release: a newer version, its size and the first lines of its notes. */
        val OFFER = UpdateOffer(
            version = "2.1.0",
            url = "https://updates.example/anistream-2.1.0-android-tv-arm64.apk",
            sha256 = "ab".repeat(32),
            bytes = 48_234_496,
            page = "https://updates.example/releases/v2.1.0",
            notes = "Browse with filters on the TV\n\nSearch takes genre, format, season, year and sort with nothing typed.",
        )
    }
}
