package tv.anistream.tv

import android.os.Build
import tv.anistream.engine.model.Release
import tv.anistream.engine.model.ReleaseAsset

/**
 * The release's TV APK for the ABI this app runs as. A TV box with a 64-bit processor often runs a
 * 32-bit system, and then its first supported ABI is armeabi-v7a, the one its app was installed
 * for; the 32-bit file is the one it can update to. Otherwise the 64-bit file.
 */
fun tvUpdateAsset(release: Release, abis: Array<String> = Build.SUPPORTED_ABIS): ReleaseAsset? =
    if (abis.firstOrNull() == "armeabi-v7a") release.androidTvArmv7 else release.androidTv
