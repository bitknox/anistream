// What the engine asks of the app it runs in.

package tv.anistream.store

import uniffi.anistream_ffi.FfiHost

/**
 * The app's [FfiHost]: links opened, notifications posted and secrets kept, each through what the
 * app hands it, so the store needs nothing of an activity and a test hands it nothing real.
 *
 * The engine calls it on its own threads, never the main one, and waits for each answer.
 *
 * @param secrets where tokens are kept.
 * @param open opens a link in the browser or the app for it; false when it could not.
 * @param notify posts a notification, a copy of a notice for a user who has switched away.
 */
class AppHost(
    val secrets: SecretBackend,
    private val open: (String) -> Boolean = { false },
    private val notify: (String, String) -> Unit = { _, _ -> },
) : FfiHost {
    override fun openUrl(url: String): Boolean = open(url)

    override fun notify(title: String, body: String) = notify.invoke(title, body)

    override fun secretGet(id: String): String? = secrets.secret(id)

    override fun secretSet(id: String, value: String): Boolean = secrets.setSecret(value, id)

    override fun secretDelete(id: String): Boolean = secrets.deleteSecret(id)
}
