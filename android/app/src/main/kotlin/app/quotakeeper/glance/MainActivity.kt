package app.quotakeeper.glance

import android.content.Context
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URI

class MainActivity : ComponentActivity() {
    private val scanned = mutableStateOf<String?>(null)
    private val scan = registerForActivityResult(ScanContract()) { result ->
        result.contents?.let { scanned.value = it }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme(background = Color.Black, surface = Color.Black)) {
                GlanceApp(
                    startOrigin = savedOrigin(),
                    scanned = scanned.value,
                    onScan = {
                        scan.launch(
                            ScanOptions().apply {
                                setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                                setPrompt("对准分享页上的二维码")
                                setBeepEnabled(false)
                                setOrientationLocked(false)
                            },
                        )
                    },
                    onSave = { origin ->
                        getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                            .edit()
                            .putString(KEY_ORIGIN, origin)
                            .apply()
                    },
                    onClear = {
                        scanned.value = null
                        getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().remove(KEY_ORIGIN).apply()
                    },
                )
            }
        }
    }

    private fun savedOrigin(): String? =
        getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_ORIGIN, null)
}

private const val PREFS = "glance"
private const val KEY_ORIGIN = "origin"

@Composable
private fun GlanceApp(
    startOrigin: String?,
    scanned: String?,
    onScan: () -> Unit,
    onSave: (String) -> Unit,
    onClear: () -> Unit,
) {
    var origin by remember { mutableStateOf(startOrigin) }
    var draft by remember { mutableStateOf(startOrigin.orEmpty()) }
    LaunchedEffect(scanned) {
        val next = scanned?.let(::originOf) ?: return@LaunchedEffect
        draft = next
        origin = next
        onSave(next)
    }
    Column(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .padding(20.dp),
    ) {
        if (origin == null) {
            ConnectForm(
                draft = draft,
                onDraft = { draft = it },
                onScan = onScan,
                onConnect = {
                    val next = originOf(draft) ?: return@ConnectForm
                    origin = next
                    onSave(next)
                },
            )
        } else {
            Board(
                origin = origin!!,
                onReplace = {
                    draft = origin.orEmpty()
                    origin = null
                    onClear()
                },
            )
        }
    }
}

@Composable
private fun ConnectForm(
    draft: String,
    onDraft: (String) -> Unit,
    onScan: () -> Unit,
    onConnect: () -> Unit,
) {
    Text("剩余", color = Color(0xFFF5F5F7), fontSize = 34.sp, fontWeight = FontWeight.SemiBold)
    Spacer(Modifier.height(12.dp))
    Text(
        "扫电脑分享页上的二维码，或把复制的地址粘贴到这里。",
        color = Color(0xFF98989F),
        fontSize = 15.sp,
    )
    Spacer(Modifier.height(20.dp))
    OutlinedTextField(
        value = draft,
        onValueChange = onDraft,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text("电脑地址") },
        placeholder = { Text("192.168.1.8:8420") },
    )
    Spacer(Modifier.height(16.dp))
    Row {
        Button(onClick = onScan) { Text("扫二维码") }
        Spacer(Modifier.width(12.dp))
        Button(onClick = onConnect, enabled = originOf(draft) != null) { Text("连接") }
    }
}

@Composable
private fun Board(origin: String, onReplace: () -> Unit) {
    var glance by remember { mutableStateOf<Glance?>(null) }
    var pill by remember { mutableStateOf("连接中") }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(origin) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            while (isActive) {
                val result = withContext(Dispatchers.IO) { runCatching { fetchGlance(origin) } }
                result.onSuccess { next ->
                    glance = next
                    pill = when (next) {
                        Glance.Waiting -> "连接中"
                        is Glance.Ready -> if (next.stale) "数据滞后" else "实时"
                    }
                }.onFailure {
                    pill = "等待数据"
                }
                delay(30_000)
            }
        }
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(pill, color = Color(0xFFF5F5F7), fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        Button(onClick = onReplace) { Text("更换") }
    }
    Spacer(Modifier.height(8.dp))
    val current = glance
    when {
        current == null && pill == "等待数据" -> Empty("连不上这台电脑")
        current == null || current is Glance.Waiting -> Empty("等待配额")
        current is Glance.Ready && current.cards.isEmpty() -> Empty("没有可显示的配额")
        current is Glance.Ready -> CardList(current.cards)
    }
}

@Composable
private fun Empty(text: String) {
    Text(text, color = Color(0xFF98989F), fontSize = 16.sp, modifier = Modifier.padding(top = 28.dp))
}

@Composable
private fun CardList(cards: List<RemainCard>) {
    Column(Modifier.verticalScroll(rememberScrollState())) {
        cards.forEach { card ->
            Text(card.title, color = Color(0xFFF5F5F7), fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            if (card.plan.isNotEmpty()) {
                Text(card.plan, color = Color(0xFF98989F), fontSize = 13.sp)
            }
            card.windows.forEachIndexed { index, window ->
                if (index == 0) Hero(window) else WindowRow(window)
            }
            Spacer(Modifier.height(22.dp))
        }
    }
}

@Composable
private fun Hero(window: RemainWindow) {
    val color = levelColor(window.level)
    Text("${window.remainingPct}%", color = color, fontSize = 64.sp, fontWeight = FontWeight.SemiBold)
    Box(
        Modifier
            .padding(vertical = 8.dp)
            .fillMaxWidth()
            .height(2.dp)
            .background(Color(0xFF2C2C2E)),
    ) {
        Box(
            Modifier
                .fillMaxWidth(window.remainingPct.coerceIn(0, 100) / 100f)
                .height(2.dp)
                .background(color),
        )
    }
    Meta(window)
}

@Composable
private fun WindowRow(window: RemainWindow) {
    Row(Modifier.padding(top = 10.dp)) {
        Text(
            "${window.remainingPct}%",
            color = levelColor(window.level),
            fontSize = 28.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.width(92.dp),
        )
        Meta(window)
    }
}

@Composable
private fun Meta(window: RemainWindow) {
    Row {
        Text(window.label, color = Color(0xFF98989F), fontSize = 14.sp)
        val reset = window.resetsInMs
        if (reset != null) {
            Text("  ${fmtReset(reset)} 后重置", color = Color(0xFF98989F), fontSize = 14.sp)
        }
    }
}

private fun levelColor(level: String): Color = when (level) {
    "warn" -> Color(0xFFFFD60A)
    "out" -> Color(0xFFFF453A)
    else -> Color(0xFF30D158)
}

private fun fetchGlance(origin: String): Glance {
    val connection = URI("$origin/api/summary").toURL().openConnection() as HttpURLConnection
    connection.connectTimeout = 8_000
    connection.readTimeout = 8_000
    connection.instanceFollowRedirects = false
    try {
        if (connection.responseCode != 200) error("http ${connection.responseCode}")
        val body = connection.inputStream.bufferedReader().use { it.readText() }
        return parseGlance(body)
    } finally {
        connection.disconnect()
    }
}
