package com.gravityanime.app

import android.app.Activity
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class PlayArgs {
    var title: String = ""
    var videoUrl: String = ""
    var referer: String = ""
    var audioUrl: String? = null
    var subtitleUrl: String? = null
    var startMs: Long = 0L
}

@TauriPlugin
class PlayerPlugin(private val host: Activity) : Plugin(host) {
    @Command
    fun play(invoke: Invoke) {
        val args = invoke.parseArgs(PlayArgs::class.java)
        val req = PlayRequest(
            args.title, args.videoUrl, args.referer, args.audioUrl, args.subtitleUrl, args.startMs
        )
        val useCinema = NativePlayback.cinemaAvailable() && NativePlayback.prefersCinema(host)
        val cls: Class<*> =
            if (useCinema) Class.forName(NativePlayback.CINEMA_CLASS) else PlayerActivity::class.java
        val intent = NativePlayback.intentFor(host, cls, req)
        host.runOnUiThread { startActivityForResult(invoke, intent, "onPlaybackResult") }
    }

    @ActivityCallback
    fun onPlaybackResult(invoke: Invoke, result: ActivityResult) {
        val d = result.data
        val ret = JSObject()
        ret.put("completed", d?.getBooleanExtra("completed", false) ?: false)
        ret.put("failed", d?.getBooleanExtra("failed", false) ?: false)
        ret.put("message", d?.getStringExtra("message"))
        ret.put("positionMs", d?.getLongExtra("positionMs", 0L) ?: 0L)
        ret.put("durationMs", d?.getLongExtra("durationMs", 0L) ?: 0L)
        invoke.resolve(ret)
    }
}