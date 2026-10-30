package tv.anistream.store

import android.content.Context
import tv.anistream.engine.model.SourceMaterial
import tv.anistream.i18n.Strings

/** What a title was adapted from, in words; `null` for Other, which is left out. */
fun SourceMaterial.words(context: Context): String? = when (this) {
    SourceMaterial.ORIGINAL -> Strings.materialOriginal(context)
    SourceMaterial.MANGA -> Strings.materialManga(context)
    SourceMaterial.LIGHT_NOVEL -> Strings.materialLightNovel(context)
    SourceMaterial.WEB_NOVEL -> Strings.materialWebNovel(context)
    SourceMaterial.VISUAL_NOVEL -> Strings.materialVisualNovel(context)
    SourceMaterial.VIDEO_GAME -> Strings.materialVideoGame(context)
    SourceMaterial.NOVEL -> Strings.materialNovel(context)
    SourceMaterial.DOUJINSHI -> Strings.materialDoujinshi(context)
    SourceMaterial.ANIME -> Strings.materialAnime(context)
    SourceMaterial.LIVE_ACTION -> Strings.materialLiveAction(context)
    SourceMaterial.GAME -> Strings.materialGame(context)
    SourceMaterial.COMIC -> Strings.materialComic(context)
    SourceMaterial.MULTIMEDIA_PROJECT -> Strings.materialMultimedia(context)
    SourceMaterial.PICTURE_BOOK -> Strings.materialPictureBook(context)
    SourceMaterial.OTHER -> null
}
