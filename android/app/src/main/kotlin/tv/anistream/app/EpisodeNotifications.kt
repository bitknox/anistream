// New-episode notifications: one for each episode the check finds, grouped under a summary when
// there are several, with Play and Mark watched routed through the app.

package tv.anistream.app

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Typeface
import android.net.Uri
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.StyleSpan
import androidx.compose.ui.graphics.toArgb
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import tv.anistream.design.ObiSilk
import tv.anistream.engine.core.AnilistId
import tv.anistream.store.NewEpisode
import tv.anistream.store.Wording
import tv.anistream.ui.AppLink

/**
 * What to post for a check's episodes, beside the ones still in the shade: a notification for
 * each episode, keyed by the show and the episode so one said twice replaces itself, and a
 * summary over the group once it holds more than one.
 *
 * Of a batch, only the first episode not already shown makes a sound, so five at once ring once.
 */
data class EpisodePlan(val posts: List<Post>, val summary: Summary?) {
    /** One episode's notification. */
    data class Post(val episode: NewEpisode, val tag: String, val alert: Boolean)

    /** An episode as the summary lists it: the show, then "Episode 8". */
    data class Line(val title: String, val episode: UInt)

    /** The group's summary: how many, and a line for each, newest first. */
    data class Summary(val count: Int, val lines: List<Line>) {
        /** "3 new episodes". */
        val title: String get() = if (count == 1) "1 new episode" else "$count new episodes"

        /** The shows, as the collapsed summary names them: "Kazamachi Shokudo, Ame no Kajiya and Natsukaze Relay". */
        val text: String get() = Wording.list(lines.map { it.title }.distinct())
    }

    companion object {
        /**
         * The plan for [episodes] when [shown] are already posted: each by its tag, with the
         * line the summary gives it.
         */
        fun of(episodes: List<NewEpisode>, shown: Map<String, Line>): EpisodePlan {
            val unique = episodes.distinctBy(::tagOf)
            var rung = false
            val posts = unique.map { episode ->
                val tag = tagOf(episode)
                val alert = !rung && tag !in shown
                if (alert) rung = true
                Post(episode, tag, alert)
            }
            val lines = shown + unique.associate { tagOf(it) to lineOf(it) }
            val newestFirst = unique.reversed().map { tagOf(it) } + shown.keys.reversed()
            val summary = if (lines.size > 1) Summary(lines.size, newestFirst.distinct().mapNotNull(lines::get)) else null
            return EpisodePlan(posts, summary)
        }

        /** The notification's tag: the show and the episode. */
        fun tagOf(episode: NewEpisode): String = tagOf(episode.id, episode.episode)

        fun tagOf(id: AnilistId, episode: UInt): String = "episode/$id/$episode"

        /**
         * What the notification says under the show's title, never the episode's name: "Episode 9
         * is out.", "Episode 9 is out, dubbed." for a show watched dubbed, and for a viewer behind,
         * "Episode 10 is out. Next for you: episode 8."
         */
        fun textOf(episode: NewEpisode): String = buildString {
            append("Episode ${episode.episode} is out")
            if (episode.dubbed) append(", dubbed")
            append(".")
            episode.nextForYou?.let { append(" Next for you: episode $it.") }
        }

        /** The episode Play starts: the viewer's next where they are behind, else the new one. */
        fun playOf(episode: NewEpisode): UInt = episode.nextForYou ?: episode.episode

        /** The summary's line for it. */
        fun lineOf(episode: NewEpisode): Line = Line(episode.title, episode.episode)

        /** What shows with previews hidden on the lock screen: the app's name, and this. */
        const val HIDDEN_TEXT = "New episode"
    }
}

/**
 * Posts the new-episode notifications on their own channel, "New episodes", at the default
 * importance, as NativeNotify draws them: the mark on an amber circle, the show's title, "Episode 9
 * is out." and when it aired, the cover on the right, and expanded, the art in full with Play and
 * Mark watched. A tap opens the show's page. With previews hidden, the lock screen says only "New
 * episode".
 */
class EpisodeNotifier(private val context: Context) {
    private val manager = context.getSystemService(NotificationManager::class.java)

    init {
        val channel = NotificationChannel(CHANNEL, context.getString(R.string.episodes_channel), NotificationManager.IMPORTANCE_DEFAULT)
        channel.description = context.getString(R.string.episodes_channel_description)
        manager.createNotificationChannel(channel)
    }

    /**
     * Whether a notification posted now would show: the permission given, notifications on for
     * the app and the channel not switched off. Without it the check is not asked, so what it
     * would say waits for a check that can say it.
     */
    val canPost: Boolean
        get() = ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED &&
            NotificationManagerCompat.from(context).areNotificationsEnabled() &&
            manager.getNotificationChannel(CHANNEL)?.importance != NotificationManager.IMPORTANCE_NONE

    /** Post [episodes], beside the ones already shown. */
    fun post(episodes: List<NewEpisode>) {
        if (episodes.isEmpty() || !canPost) return
        val plan = EpisodePlan.of(episodes, shown())
        for (post in plan.posts) manager.notify(post.tag, EPISODE_ID, notification(post))
        plan.summary?.let { manager.notify(SUMMARY_ID, summary(it)) }
    }

    /**
     * Take [tag]'s notification away, as a tap on one of its actions does, and the summary once
     * the group is empty. Not before: the system takes a group's episodes away with its summary,
     * and shows a group of one as that one alone.
     */
    fun dismiss(tag: String) {
        manager.cancel(tag, EPISODE_ID)
        val left = shown()
        if (left.isEmpty()) {
            manager.cancel(SUMMARY_ID)
        } else {
            manager.notify(SUMMARY_ID, summary(EpisodePlan.Summary(left.size, left.values.reversed())))
        }
    }

    /** The episodes' notifications on screen, each by its tag, with its summary line, oldest first. */
    private fun shown(): Map<String, EpisodePlan.Line> = manager.activeNotifications
        .filter { it.id == EPISODE_ID && it.tag != null && it.notification.group == GROUP }
        .sortedBy { it.notification.`when` }
        .associate { shown ->
            val extras = shown.notification.extras
            shown.tag to EpisodePlan.Line(extras.getString(EXTRA_SHOW).orEmpty(), extras.getInt(EXTRA_EPISODE).toUInt())
        }

    /** The notification's frame, the same for an episode and the summary: the mark on amber, the channel, the group. */
    private fun builder(): NotificationCompat.Builder = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        .setColor(AMBER)
        .setGroup(GROUP)
        .setAutoCancel(true)

    private fun notification(post: EpisodePlan.Post): Notification {
        val episode = post.episode
        val tag = post.tag
        val builder = builder()
            .setContentTitle(episode.title)
            .setContentText(EpisodePlan.textOf(episode))
            .setWhen(episode.airedAt * 1000)
            .setShowWhen(true)
            .setSilent(!post.alert)
            .setOnlyAlertOnce(true)
            .setPublicVersion(builder().setContentTitle(context.getString(R.string.app_name)).setContentText(EpisodePlan.HIDDEN_TEXT).build())
            .addExtras(
                android.os.Bundle().apply {
                    putString(EXTRA_SHOW, episode.title)
                    putInt(EXTRA_EPISODE, episode.episode.toInt())
                },
            )
            .setContentIntent(openIntent(context, AppLink.Title(episode.id), tag))
            .addAction(0, context.getString(R.string.episode_play), openIntent(context, AppLink.Play(episode.id, EpisodePlan.playOf(episode).toString()), tag))
        // Behind, marking the new one watched would skip the ones between: no Mark watched.
        if (episode.nextForYou == null) {
            builder.addAction(0, context.getString(R.string.episode_mark_watched), markWatchedIntent(context, episode.id, episode.episode))
        }
        // The cover on the right, and expanded, the banner in full where there is one.
        episode.art?.let(builder::setLargeIcon)
        (episode.banner ?: episode.art)?.let { picture ->
            builder.setStyle(NotificationCompat.BigPictureStyle().bigPicture(picture).bigLargeIcon(null as Bitmap?))
        }
        return builder.build()
    }

    private fun summary(summary: EpisodePlan.Summary): Notification {
        val style = NotificationCompat.InboxStyle()
        for (line in summary.lines) {
            style.addLine(
                SpannableStringBuilder()
                    .append(line.title, StyleSpan(Typeface.BOLD), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                    .append("  Episode ${line.episode}"),
            )
        }
        return builder()
            .setContentTitle(summary.title)
            .setContentText(summary.text)
            .setStyle(style)
            .setPublicVersion(builder().setContentTitle(context.getString(R.string.app_name)).setContentText(summary.title).setGroupSummary(true).build())
            .setGroupSummary(true)
            // The episodes ring, as the plan says; the summary never does.
            .setGroupAlertBehavior(NotificationCompat.GROUP_ALERT_CHILDREN)
            .setContentIntent(
                PendingIntent.getActivity(
                    context,
                    0,
                    Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )
            .build()
    }

    companion object {
        /** The channel, "New episodes". */
        const val CHANNEL = "new_episodes"

        /** The group every episode's notification joins. */
        const val GROUP = "new-episodes"

        /** Each episode's notification goes by its tag under this id; the summary by its own. */
        const val EPISODE_ID = 1
        const val SUMMARY_ID = 2

        /** The broadcast Mark watched sends to [EpisodeActionReceiver]. */
        const val ACTION_MARK_WATCHED = "tv.anistream.action.MARK_WATCHED"

        /** The notification an app link came from, by its tag, to take away as it opens. */
        const val EXTRA_NOTIFICATION = "tv.anistream.extra.NOTIFICATION"

        /** The show and the episode a notification is for, for the summary's lines. */
        private const val EXTRA_SHOW = "tv.anistream.extra.SHOW"
        private const val EXTRA_EPISODE = "tv.anistream.extra.EPISODE"

        /** The small icon's circle: the palette's amber. */
        private val AMBER = ObiSilk.Amber.toArgb()

        /**
         * The activity on [link], from [tag]'s notification. Each link is its own intent by its
         * data, so every episode's actions stay apart.
         */
        fun openIntent(context: Context, link: AppLink, tag: String): PendingIntent = PendingIntent.getActivity(
            context,
            0,
            context.linkIntent(link).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK).putExtra(EXTRA_NOTIFICATION, tag),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        /** Mark [episode] of [id] watched, in the app's receiver, which no other app can reach. */
        fun markWatchedIntent(context: Context, id: AnilistId, episode: UInt): PendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            Intent(ACTION_MARK_WATCHED, watchedUri(id, episode), context, EpisodeActionReceiver::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        /** What a Mark watched intent carries: `anistream://watched/<id>/<episode>`. */
        fun watchedUri(id: AnilistId, episode: UInt): Uri = Uri.parse("anistream://watched/$id/$episode")

        /** The show and episode [uri] names, when it is a Mark watched one. */
        fun watchedOf(uri: String?): Pair<AnilistId, UInt>? {
            val parts = uri?.removePrefix("anistream://watched/")?.takeIf { it != uri }?.split('/') ?: return null
            val id = parts.getOrNull(0)?.toUIntOrNull() ?: return null
            val episode = parts.getOrNull(1)?.toUIntOrNull() ?: return null
            return if (parts.size == 2) id to episode else null
        }
    }
}
