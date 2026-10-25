// The sign-ins that do not go through the browser, as modal bottom sheets: Kitsu's user name and
// password, the code Simkl and Trakt ask for on another screen, and settling what a library pull
// found different here and on AniList.

package tv.anistream.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.tabular
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.store.CodeCountdown
import tv.anistream.store.AccountsModel

/**
 * A sheet as the brief has it: surfaceContainer, corners 28 on top, the drag handle in haze at 40 %,
 * over the scrim at black 32 %; its motion and drag are the component's own (T7).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SettingsSheet(onDismiss: () -> Unit, tall: Boolean = false, content: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = tall),
        containerColor = colors.surfaceContainer,
        scrimColor = Color.Black.copy(alpha = 0.32f),
        dragHandle = { BottomSheetDefaults.DragHandle(color = colors.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
                .padding(bottom = 24.dp)
                .navigationBarsPadding(),
        ) { content() }
    }
}

/** A sheet's title, in titleLarge. */
@Composable
internal fun SheetTitle(text: String) {
    Text(text, style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(bottom = 8.dp).semantics { heading() })
}

/** A sheet's text, in bodyMedium haze. */
@Composable
internal fun SheetText(text: String, modifier: Modifier = Modifier) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = modifier)
}

/** A sheet's buttons at its foot, at the end: text buttons, then the one filled amber action. */
@Composable
internal fun SheetButtons(content: @Composable () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(top = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        verticalAlignment = Alignment.CenterVertically,
    ) { content() }
}

/** A text action in silk, never amber: Cancel, Close, Copy link. */
@Composable
internal fun QuietButton(text: String, enabled: Boolean = true, onClick: () -> Unit) {
    TextButton(
        onClick = onClick,
        enabled = enabled,
        colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
    ) { Text(text) }
}

/** A filled text field on surfaceContainerHighest, its label above what is typed. */
@Composable
internal fun SheetField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    password: Boolean = false,
    keyboard: KeyboardOptions = KeyboardOptions.Default,
    actions: KeyboardActions = KeyboardActions.Default,
    supporting: String? = null,
    error: Boolean = false,
    trailing: (@Composable () -> Unit)? = null,
    enabled: Boolean = true,
) {
    val colors = MaterialTheme.colorScheme
    TextField(
        value = value,
        onValueChange = onChange,
        enabled = enabled,
        label = { Text(label) },
        singleLine = true,
        isError = error,
        supportingText = supporting?.let { { Text(it) } },
        trailingIcon = trailing,
        visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = keyboard,
        keyboardActions = actions,
        shape = RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp),
        colors = TextFieldDefaults.colors(
            focusedContainerColor = colors.surfaceContainerHighest,
            unfocusedContainerColor = colors.surfaceContainerHighest,
            focusedIndicatorColor = colors.onSurface,
            unfocusedIndicatorColor = colors.onSurfaceVariant,
            focusedLabelColor = colors.onSurface,
            unfocusedLabelColor = colors.onSurfaceVariant,
            cursorColor = colors.primary,
            disabledContainerColor = colors.surfaceContainerHighest.copy(alpha = 0.38f),
        ),
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * Kitsu's sign-in: its email or user name and password, which go to Kitsu once, for a token kept
 * on this device. It closes once Kitsu says yes; a refusal comes as a notice and the sheet stays.
 */
@Composable
internal fun KitsuSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val working by store.accounts.working.collectAsStateWithLifecycle()
    var username by rememberSaveable { mutableStateOf("") }
    // Never saved: the password lives only as long as the sheet.
    var password by remember { mutableStateOf("") }
    val busy = store.accounts.isWorking(working, "kitsu")
    val connected = trackers["kitsu"]?.connected == true
    LaunchedEffect(connected) { if (connected) onDismiss() }
    val signIn = {
        if (username.isNotBlank() && password.isNotEmpty() && !busy) {
            store.connectKitsu(username.trim(), password)
            password = ""
        }
    }
    SettingsSheet(onDismiss) {
        SheetTitle("Sign in to Kitsu")
        SheetText("Your password goes to Kitsu once, for a token kept on this device.")
        Spacer(Modifier.heightIn(min = 16.dp))
        SheetField(
            username,
            { username = it },
            "Email or user name",
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next, autoCorrectEnabled = false),
        )
        Spacer(Modifier.heightIn(min = 12.dp))
        SheetField(
            password,
            { password = it },
            "Password",
            password = true,
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done, autoCorrectEnabled = false),
            actions = KeyboardActions(onDone = { signIn() }),
        )
        SheetButtons {
            QuietButton("Cancel", onClick = onDismiss)
            Button(onClick = signIn, enabled = username.isNotBlank() && password.isNotEmpty() && !busy) {
                Text(if (busy) "Signing in" else "Sign in")
            }
        }
    }
}

/**
 * A code to type on another screen, for Simkl or Trakt: the code large, where to type it, and a
 * way there. It stays until the engine says the sign-in ended, or the viewer closes it.
 */
@Composable
internal fun DeviceCodeSheet(prompt: DeviceCodePrompt, onDismiss: () -> Unit) {
    val uri = LocalUriHandler.current
    val name = AccountsModel.name(prompt.tracker)
    val host = runCatching { android.net.Uri.parse(prompt.url).host }.getOrNull() ?: prompt.url
    val colors = MaterialTheme.colorScheme
    SettingsSheet(onDismiss) {
        SheetTitle("Sign in to $name")
        SheetText("Go to $host and enter this code. anistream signs in once you have.")
        Text(
            prompt.code,
            style = TextStyle(fontFamily = FontFamily.Monospace, fontWeight = FontWeight.Medium, fontSize = 34.sp, lineHeight = 44.sp, letterSpacing = 4.sp),
            color = colors.onSurface,
            modifier = Modifier
                .padding(top = 16.dp)
                .fillMaxWidth()
                .background(colors.surface, RoundedCornerShape(12.dp))
                .padding(vertical = 14.dp)
                .semantics { contentDescription = "Code ${prompt.code.toList().joinToString(" ")}" },
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        SheetText("Waiting for $name", Modifier.padding(top = 12.dp))
        CodeExpiry(prompt, Modifier.padding(top = 4.dp))
        SheetButtons {
            QuietButton("Close", onClick = onDismiss)
            Button(onClick = { runCatching { uri.openUri(prompt.url) } }) { Text("Open $host") }
        }
    }
}

/**
 * "Expires in 14:59" under a code, ticking each second. At nought the sign-in asks for a new code,
 * as the TV's "Get a new code" does, which replaces this one. Nothing when the service gave no time.
 */
@Composable
internal fun CodeExpiry(prompt: DeviceCodePrompt, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val expiresAt = prompt.expiresAt ?: return
    var left by remember(prompt.code, expiresAt) { mutableStateOf(CodeCountdown.secondsLeft(expiresAt, System.currentTimeMillis())) }
    LaunchedEffect(prompt.code, expiresAt) {
        var live = false
        CodeCountdown.ticks(expiresAt).collect {
            if (it > 0) live = true
            left = it
        }
        // A code that was out when it arrived is not asked for again, which would only loop.
        if (live) store.connect(prompt.tracker)
    }
    Text(
        CodeCountdown.line(left),
        style = MaterialTheme.typography.bodyMedium.tabular(),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

/** What a library pull found different here and on AniList: each title, and which side to keep. */
@Composable
internal fun ConflictsSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val conflicts by store.accounts.conflicts.collectAsStateWithLifecycle()
    LaunchedEffect(conflicts.isEmpty()) { if (conflicts.isEmpty()) onDismiss() }
    SettingsSheet(onDismiss, tall = true) {
        SheetTitle("Different on AniList")
        SheetText("These changed on AniList and here since the last sync. Keep one side of each.")
        conflicts.forEachIndexed { index, row ->
            if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            Column(Modifier.padding(vertical = 12.dp)) {
                Text(row.title, style = tv.anistream.design.ShowTitle.row)
                SheetText("${row.field.replaceFirstChar { it.uppercase() }}: ${row.local} here, ${row.remote} on AniList")
                Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { store.resolveConflict(row.anilistId, keepLocal = true) }) { Text("Keep this device's") }
                    OutlinedButton(onClick = { store.resolveConflict(row.anilistId, keepLocal = false) }) { Text("Keep AniList's") }
                }
            }
        }
        SheetButtons {
            Button(onClick = onDismiss) { Text("Done") }
        }
    }
}

/** Which of the sheets that come up over any screen is open: Kitsu's sign-in for now. */
@Stable
class AppSheets {
    var kitsu by mutableStateOf(false)
}

val LocalAppSheets = staticCompositionLocalOf { AppSheets() }

/**
 * The sheets a sign-in can bring up over any screen: the code Simkl or Trakt waits on, and
 * Kitsu's, which its row and a notice's Sign in button open.
 */
@Composable
internal fun AccountSheets() {
    val store = LocalAppStore.current
    val sheets = LocalAppSheets.current
    val prompt by store.accounts.deviceCode.collectAsStateWithLifecycle()
    // The code the viewer closed, so it does not come straight back.
    var closed by remember { mutableStateOf<String?>(null) }
    prompt?.takeIf { it.code != closed }?.let { shown ->
        DeviceCodeSheet(shown) { closed = shown.code }
    }
    if (sheets.kitsu) KitsuSheet { sheets.kitsu = false }
}
