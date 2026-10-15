package app.quotakeeper.glance

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class GlanceJsonTest {
    @Test
    fun originKeepsHostAndPort() {
        assertEquals("http://192.168.1.8:8420", originOf("http://192.168.1.8:8420/"))
        assertEquals("http://192.168.1.8:8421", originOf("192.168.1.8:8421/api/summary"))
        assertNull(originOf("not a host"))
    }

    @Test
    fun waitingHasNoCards() {
        assertEquals(Glance.Waiting, parseGlance("""{"state":"waiting"}"""))
    }

    @Test
    fun readyKeepsOrderAndDropsPassedReset() {
        val glance = parseGlance(
            """
            {"state":"ready","stale":false,"cards":[
              {"title":"Codex","plan":"plus","windows":[
                {"label":"MONTH","remainingPct":0,"level":"out"},
                {"label":"5H","remainingPct":20,"level":"warn","resetsInMs":1000}
              ]},
              {"title":"MiniMax Coding","plan":"","windows":[
                {"label":"WEEK","remainingPct":80,"level":"ok","resetsInMs":86400000}
              ]}
            ]}
            """.trimIndent(),
        ) as Glance.Ready
        assertEquals(listOf("Codex", "MiniMax Coding"), glance.cards.map { it.title })
        assertEquals(0, glance.cards[0].windows[0].remainingPct)
        assertNull(glance.cards[0].windows[0].resetsInMs)
        assertEquals(1000L, glance.cards[0].windows[1].resetsInMs)
        assertEquals("<1m", fmtReset(1000))
        assertEquals("1h", fmtReset(3_600_000))
        assertEquals("1d", fmtReset(86_400_000))
    }

    @Test
    fun oldSummaryBecomesRemaining() {
        val glance = parseGlance(
            """
            {"ok":true,"generatedAtMs":1000,"rings":[
              {"title":"Codex","planType":"plus","label":"5H","usedPct":42,"resetsInMs":3600000,"level":0},
              {"title":"Codex","planType":"plus","label":"WEEK","usedPct":91,"resetsInMs":-5,"level":1}
            ]}
            """.trimIndent(),
            nowMs = 1000,
        ) as Glance.Ready
        assertEquals(false, glance.stale)
        assertEquals("Codex", glance.cards.single().title)
        assertEquals(9, glance.cards[0].windows[0].remainingPct)
        assertEquals("warn", glance.cards[0].windows[0].level)
        assertNull(glance.cards[0].windows[0].resetsInMs)
        assertEquals(58, glance.cards[0].windows[1].remainingPct)
    }
}
