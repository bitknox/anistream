// What pairing shows on the TV: the dialog when a phone asks (TVPairRequest), the full screen the
// TV opens itself (TVPair), the QR code both carry, and the notice in the top right.

package tv.anistream.tv.remote

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

/** A QR code of [text], silk modules on ground with a quiet zone of four, [size] square. */
@Composable
fun QrCode(text: String, modifier: Modifier = Modifier, size: Dp = 180.dp, label: String = "QR code") {
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
private fun Code(code: String) {
    Text(
        grouped(code),
        style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
        color = ObiSilk.Silk,
    )
}

/** TVPairRequest: a phone asked to pair, wherever the browse UI is. Cancel abandons its code. */
@Composable
fun PairRequestDialog(request: PairRequest, onCancel: () -> Unit) {
    Dialog(onDismissRequest = onCancel) {
        val cancel = remember { FocusRequester() }
        LaunchedEffect(Unit) { runCatching { cancel.requestFocus() } }
        Column(
            Modifier
                .widthIn(max = 560.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text("Pair with ${request.phone}?", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Code(request.code)
            Text("Enter this code on the phone.", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            Spacer(Modifier.height(8.dp))
            TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(cancel)) { ButtonLabel("Cancel") }
        }
    }
}

/** Settings › Remotes › a paired phone: remove it, after asking. */
@Composable
fun RemovePhoneDialog(phone: String, onRemove: () -> Unit, onCancel: () -> Unit) {
    Dialog(onDismissRequest = onCancel) {
        val cancel = remember { FocusRequester() }
        LaunchedEffect(Unit) { runCatching { cancel.requestFocus() } }
        Column(
            Modifier
                .widthIn(max = 560.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text("Remove $phone?", style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Text(
                "It stops working as a remote for this TV. It can pair again with a new code.",
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
            )
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(cancel)) { ButtonLabel("Cancel") }
                TonalButton(onClick = onRemove) { ButtonLabel("Remove") }
            }
        }
    }
}

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
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Row(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Text("Pair a phone", style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk)
            Text(
                "Open anistream on your phone and choose Play on TV, then scan this code.",
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.widthIn(max = 640.dp),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
                if (link != null) {
                    QrCode(link, label = "QR code for pairing")
                } else {
                    Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
                }
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Or enter this code", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
                    if (code != null) Code(code) else Text("Starting", style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                    if (address != null) {
                        Text("This TV: $name, $address", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                    }
                }
            }
            Text("The code changes every 5 minutes.", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            TonalButton(onClick = onClose, modifier = Modifier.focusRequester(focus)) { ButtonLabel("Done") }
        }
    }
}

/** The remote's notice, top right: "Mika's phone is paired". */
@Composable
fun RemoteNotice(text: String, modifier: Modifier = Modifier) {
    Box(
        modifier
            .padding(top = Canvas.SafeY, end = Canvas.SafeX)
            .background(ObiSilk.RaisedHigh, RoundedCornerShape(12.dp))
            .padding(horizontal = 20.dp, vertical = 14.dp),
    ) {
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
    }
}
