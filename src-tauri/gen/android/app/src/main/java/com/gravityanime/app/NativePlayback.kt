package com.gravityanime.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.MergingMediaSource

data class PlayRequest(
    val title: String,
    val videoUrl: String,
    val referer: String,
    val audioUrl: String?,
    val subtitleUrl: String?,
    val startMs: Long = 0L,
)

@OptIn(UnstableApi::class)
object NativePlayback {
    private const val UA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/115.0"
    private const val PREFS = "gravity_player"
    const val CINEMA_CLASS = "com.gravityanime.app.CinemaActivity"

    fun cinemaAvailable(): Boolean =
        try { Class.forName(CINEMA_CLASS); true } catch (_: Throwable) { false }

    fun prefersCinema(ctx: Context): Boolean =
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean("cinema", false)

    fun setPrefersCinema(ctx: Context, value: Boolean) {
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean("cinema", value).apply()
    }

    fun intentFor(ctx: Context, cls: Class<*>, r: PlayRequest): Intent =
        Intent(ctx, cls).apply {
            putExtra("title", r.title)
            putExtra("videoUrl", r.videoUrl)
            putExtra("referer", r.referer)
            putExtra("audioUrl", r.audioUrl)
            putExtra("subtitleUrl", r.subtitleUrl)
            putExtra("startMs", r.startMs)
        }

    fun requestFrom(i: Intent) = PlayRequest(
        title = i.getStringExtra("title") ?: "",
        videoUrl = i.getStringExtra("videoUrl") ?: "",
        referer = i.getStringExtra("referer") ?: "",
        audioUrl = i.getStringExtra("audioUrl"),
        subtitleUrl = i.getStringExtra("subtitleUrl"),
        startMs = i.getLongExtra("startMs", 0L),
    )

    fun resultIntent(completed: Boolean, failed: Boolean, message: String?, posMs: Long) =
        Intent().apply {
            putExtra("completed", completed)
            putExtra("failed", failed)
            putExtra("message", message)
            putExtra("positionMs", posMs)
        }

    private fun subtitleMime(url: String): String = when {
        url.contains(".srt") -> MimeTypes.APPLICATION_SUBRIP
        url.contains(".ass") || url.contains(".ssa") -> MimeTypes.TEXT_SSA
        else -> MimeTypes.TEXT_VTT
    }

    private fun mediaItem(url: String, subtitleUrl: String? = null): MediaItem {
        val b = MediaItem.Builder().setUri(url)
        if (url.contains(".m3u8")) b.setMimeType(MimeTypes.APPLICATION_M3U8)
        if (subtitleUrl != null) {
            val sub = MediaItem.SubtitleConfiguration.Builder(Uri.parse(subtitleUrl))
                .setMimeType(subtitleMime(subtitleUrl))
                .setLanguage("en")
                .setSelectionFlags(C.SELECTION_FLAG_DEFAULT)
                .build()
            b.setSubtitleConfigurations(listOf(sub))
        }
        return b.build()
    }

    fun buildSource(r: PlayRequest): MediaSource {
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(UA)
            .setAllowCrossProtocolRedirects(true)
            .setDefaultRequestProperties(mapOf("Referer" to r.referer))
        val factory = DefaultMediaSourceFactory(http)

        // Subtitles ride on the video item; the factory loads and parses them correctly.
        val video = factory.createMediaSource(mediaItem(r.videoUrl, r.subtitleUrl))
        val audioUrl = r.audioUrl ?: return video
        val audio = factory.createMediaSource(mediaItem(audioUrl))
        return MergingMediaSource(video, audio)
    }
}