// The app's language: the system's per-app language on Android 13 and up, and the same choice
// kept by the app below that (the TV runs on Android 11 and 12 too).

package tv.anistream.store

import android.app.LocaleManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import java.util.Locale
import tv.anistream.i18n.Language
import tv.anistream.i18n.Strings

object AppLanguage {
    private const val PREFS = "app-language"
    private const val KEY = "tag"

    /** The languages this build ships, source first. A draft one is labelled as such. */
    val shipped: List<Language> get() = Strings.languages

    /** The chosen language, or `null` to follow the system's. Only a shipped language counts. */
    fun chosen(context: Context): Language? {
        val tag = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.getSystemService(LocaleManager::class.java)?.applicationLocales?.takeIf { !it.isEmpty }?.get(0)?.toLanguageTag()
        } else {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null)
        } ?: return null
        return match(tag)
    }

    /** The language the text is in now: the chosen one, or the system's best match among the shipped. */
    fun showing(context: Context): Language =
        chosen(context) ?: match(Locale.getDefault().toLanguageTag()) ?: shipped.first()

    /**
     * Choose [language], or `null` to follow the system. From Android 13 this is the system's
     * per-app setting, so the system's own screen shows the same choice and the activity is
     * recreated by the system; below that the caller recreates it.
     */
    fun choose(context: Context, language: Language?) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val locales = language?.let { LocaleList.forLanguageTags(it.tag) } ?: LocaleList.getEmptyLocaleList()
            context.getSystemService(LocaleManager::class.java)?.applicationLocales = locales
        } else {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().apply {
                if (language == null) remove(KEY) else putString(KEY, language.tag)
            }.apply()
        }
    }

    /**
     * [base] in the chosen language, for an activity's `attachBaseContext` below Android 13. From
     * 13 the system does this itself, and [base] comes back as it is.
     */
    fun wrap(base: Context): Context {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) return base
        val tag = base.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null) ?: return base
        val locale = Locale.forLanguageTag(tag)
        Locale.setDefault(locale)
        val config = Configuration(base.resources.configuration).apply { setLocales(LocaleList(locale)) }
        return base.createConfigurationContext(config)
    }

    private fun match(tag: String): Language? {
        val wanted = Locale.forLanguageTag(tag)
        return shipped.firstOrNull { Locale.forLanguageTag(it.tag) == wanted }
            ?: shipped.firstOrNull { Locale.forLanguageTag(it.tag).language == wanted.language }
    }
}
