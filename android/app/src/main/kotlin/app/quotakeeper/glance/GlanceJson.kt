package app.quotakeeper.glance

import org.json.JSONObject
import java.net.URI
import kotlin.math.roundToInt

data class RemainWindow(
    val label: String,
    val remainingPct: Int,
    val level: String,
    val resetsInMs: Long?,
)

data class RemainCard(
    val title: String,
    val plan: String,
    val windows: List<RemainWindow>,
)

sealed class Glance {
    data object Waiting : Glance()
    data class Ready(val stale: Boolean, val cards: List<RemainCard>) : Glance()
}

fun originOf(raw: String): String? {
    val text = raw.trim()
    if (text.isEmpty()) return null
    val withScheme = if ("://" in text) text else "http://$text"
    val uri = try {
        URI(withScheme)
    } catch (_: Exception) {
        return null
    }
    val host = uri.host ?: return null
    if (uri.scheme != "http" && uri.scheme != "https") return null
    val port = if (uri.port == -1) "" else ":${uri.port}"
    return "${uri.scheme}://$host$port"
}

fun parseGlance(body: String, nowMs: Long = System.currentTimeMillis()): Glance {
    val json = JSONObject(body)
    when (json.optString("state")) {
        "waiting" -> return Glance.Waiting
        "ready" -> return Glance.Ready(
            stale = json.optBoolean("stale", false),
            cards = cardsOf(json.optJSONArray("cards")),
        )
    }
    if (json.has("rings")) return legacyRings(json, nowMs)
    error("state")
}

private fun legacyRings(json: JSONObject, nowMs: Long): Glance {
    val rings = json.optJSONArray("rings")
    if ((rings == null || rings.length() == 0) && json.optLong("generatedAtMs") == 0L) {
        return Glance.Waiting
    }
    val grouped = linkedMapOf<String, MutableList<RemainWindow>>()
    val plans = linkedMapOf<String, String>()
    val titles = linkedMapOf<String, String>()
    if (rings != null) {
        for (index in 0 until rings.length()) {
            val ring = rings.optJSONObject(index) ?: continue
            if (!ring.has("usedPct")) continue
            val title = ring.optString("title")
            val plan = ring.optString("planType")
            val key = "$title\u0000$plan"
            val used = ring.optDouble("usedPct").coerceIn(0.0, 100.0)
            val remaining = (100.0 - used).roundToInt().coerceIn(0, 100)
            val reset = if (ring.has("resetsInMs") && !ring.isNull("resetsInMs")) {
                ring.optLong("resetsInMs").takeIf { it > 0 }
            } else {
                null
            }
            val level = when (ring.optInt("level")) {
                2 -> "out"
                1 -> "warn"
                else -> "ok"
            }
            titles[key] = title
            plans[key] = plan
            grouped.getOrPut(key) { mutableListOf() }.add(
                RemainWindow(ring.optString("label"), remaining, level, reset),
            )
        }
    }
    val generated = json.optLong("generatedAtMs")
    val stale = generated > 0 && nowMs - generated > 120_000
    return Glance.Ready(
        stale = stale,
        cards = grouped.map { (key, windows) ->
            RemainCard(
                title = titles.getValue(key),
                plan = plans.getValue(key),
                windows = windows.sortedBy { it.remainingPct },
            )
        },
    )
}

private fun cardsOf(array: org.json.JSONArray?): List<RemainCard> {
    if (array == null) return emptyList()
    return buildList {
        for (index in 0 until array.length()) {
            val card = array.optJSONObject(index) ?: continue
            val windows = windowsOf(card.optJSONArray("windows"))
            if (windows.isEmpty()) continue
            add(
                RemainCard(
                    title = card.optString("title"),
                    plan = card.optString("plan"),
                    windows = windows,
                ),
            )
        }
    }
}

private fun windowsOf(array: org.json.JSONArray?): List<RemainWindow> {
    if (array == null) return emptyList()
    return buildList {
        for (index in 0 until array.length()) {
            val window = array.optJSONObject(index) ?: continue
            if (!window.has("remainingPct")) continue
            val reset = if (window.has("resetsInMs") && !window.isNull("resetsInMs")) {
                window.optLong("resetsInMs").takeIf { it > 0 }
            } else {
                null
            }
            add(
                RemainWindow(
                    label = window.optString("label"),
                    remainingPct = window.optInt("remainingPct"),
                    level = window.optString("level", "ok"),
                    resetsInMs = reset,
                ),
            )
        }
    }
}

fun fmtReset(ms: Long): String {
    if (ms < 60_000) return "<1m"
    val minutes = Math.round(ms / 60_000.0).toInt()
    if (minutes < 60) return "${minutes}m"
    val hours = minutes / 60
    if (hours < 24) {
        val rest = minutes % 60
        return if (rest == 0) "${hours}h" else "${hours}h${rest}m"
    }
    val days = hours / 24
    val rest = hours % 24
    return if (rest == 0) "${days}d" else "${days}d${rest}h"
}
