// What pairing shows on the TV: the dialog when a phone asks (TVPairRequest), the full screen the
// TV opens itself (TVPair), the QR code both carry, and the notice in the top right.

package tv.anistream.tv.remote

import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas as DrawCanvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.common.BitMatrix
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import tv.anistream.design.Nagare
import tv.anistream.design.ObiSilk
import tv.anistream.tv.ui.ButtonLabel
import tv.anistream.tv.ui.Canvas
import tv.anistream.tv.ui.TonalButton
import tv.anistream.tv.ui.entryFocus
import tv.anistream.tv.ui.focusSoon
import tv.anistream.tv.ui.rememberScreenReader
import tv.anistream.tv.ui.spelled
import tv.anistream.tv.ui.spokenBlock

/** A QR code of [text], silk modules on ground with a quiet zone of four, [size] square. */
@Composable
fun QrCode(text: String, modifier: Modifier = Modifier, size: Dp = 180.dp, label: String = stringResource(R.string.tv_qr_label_default)) {
    val matrix = remember(text) { qr(text) }
    DrawCanvas(
        modifier
            .size(size)
            .semantics { contentDescription = label },
    ) {
        drawRect(ObiSilk.Ground)
        val cell = this.size.width / matrix.width
        for (y in 0 until matrix.height) {
            for (x in 0 until matrix.width) {
                if (matrix[x, y]) drawRect(ObiSilk.Silk, Offset(x * cell, y * cell), Size(cell + 0.5f, cell + 0.5f))
            }
        }
    }
}

/** [text] as a QR code's modules, with the four-module quiet zone the brief asks for. */
fun qr(text: String): BitMatrix = QRCodeWriter().encode(
    text,
    BarcodeFormat.QR_CODE,
    0,
    0,
    mapOf(EncodeHintType.MARGIN to 4, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M),
)

/** A six-digit code in two groups of three, as people read it out: "042 917". */
fun grouped(code: String): String = if (code.length == 6) "${code.take(3)} ${code.drop(3)}" else code

@Composable
private fun Code(code: String, modifier: Modifier = Modifier) {
    Text(
        grouped(code),
        style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
        color = ObiSilk.Silk,
        modifier = modifier,
    )
}

/** TVPairRequest: a phone asked to pair, wherever the browse UI is. Cancel abandons its code. */
@Composable
fun PairRequestDialog(request: PairRequest, onCancel: () -> Unit) {
    Dialog(onDismissRequest = onCancel) {
        val cancel = remember { FocusRequester() }
        val codeFocus = remember { FocusRequester() }
        val reader by rememberScreenReader()
        // A screen reader starts on the question and the code, read together, then the button.
        LaunchedEffect(reader, request.code) { if (reader) codeFocus.focusSoon() }
        val title = stringResource(R.string.tv_pair_request, request.phone)
        val detail = stringResource(R.string.tv_pair_request_detail)
        Column(
            Modifier
                .entryFocus(cancel)
                .widthIn(max = 560.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Column(Modifier.spokenBlock("$title ${spelled(request.code)}. $detail", codeFocus), verticalArrangement = Arrangement.spacedBy(16.dp)) { // i18n-skip
                Text(title, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
                Code(request.code)
                Text(detail, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
            Spacer(Modifier.height(8.dp))
            TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(stringResource(R.string.action_cancel)) }
        }
    }
}

/** A question with one action and Cancel, Cancel focused: removing a phone, signing out. */
@Composable
fun ConfirmDialog(title: String, body: String, action: String, onConfirm: () -> Unit, onCancel: () -> Unit) {
    Dialog(onDismissRequest = onCancel) {
        val cancel = remember { FocusRequester() }
        Column(
            Modifier
                .entryFocus(cancel)
                .widthIn(max = 560.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(title, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Text(body, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(stringResource(R.string.action_cancel)) }
                TonalButton(onClick = onConfirm) { ButtonLabel(action) }
            }
        }
    }
}

/** Settings › Remotes › a paired phone: remove it, after asking. */
@Composable
fun RemovePhoneDialog(phone: String, onRemove: () -> Unit, onCancel: () -> Unit) = ConfirmDialog(
    title = stringResource(R.string.tv_remove_phone_title, phone),
    body = stringResource(R.string.tv_remove_phone_detail),
    action = stringResource(R.string.action_remove),
    onConfirm = onRemove,
    onCancel = onCancel,
)

/**
 * TVPair: Settings › Remotes › Pair a phone. A QR code of this TV's address, key and code, the
 * code beside it for typing, and the steps, until a phone pairs or Back.
 */
@Composable
fun PairScreen(host: TvRemoteHost, onClose: () -> Unit) {
    val code = host.ownCode
    PairPage(name = host.name, code = code, link = code?.let(host::pairLink), address = host.typedAddress(), onClose = onClose)
}

/** TVPair as it draws: [link] for the QR code, [code] beside it, and [address] for typing. */
@Composable
fun PairPage(name: String, code: String?, link: String?, address: String?, onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    val focus = remember { FocusRequester() }
    Row(
        Modifier
            .entryFocus(focus)
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(stringResource(R.string.tv_pair_phone), style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk)
            Text(
                stringResource(R.string.tv_pair_phone_detail),
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.widthIn(max = 640.dp),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
                if (link != null) {
                    QrCode(link, label = stringResource(R.string.tv_qr_label_pairing))
                } else {
                    Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
                }
                val enter = stringResource(R.string.tv_or_enter_code)
                val thisTv = address?.let { stringResource(R.string.tv_this_tv, name, it) }
                val codeFocus = remember { FocusRequester() }
                val reader by rememberScreenReader()
                // A screen reader starts on the code, and says a new one when it changes.
                LaunchedEffect(reader, code) { if (reader && code != null) codeFocus.focusSoon() }
                Column(
                    Modifier.spokenBlock(listOfNotNull(enter, code?.let(::spelled), thisTv).joinToString(". "), codeFocus), // i18n-skip
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(enter, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
                    if (code != null) Code(code) else Text(stringResource(R.string.downloads_notice_starting), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                    if (thisTv != null) {
                        Text(thisTv, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                    }
                }
            }
            Text(stringResource(R.string.tv_code_changes), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            TonalButton(onClick = onClose, modifier = Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.action_done)) }
        }
    }
}

