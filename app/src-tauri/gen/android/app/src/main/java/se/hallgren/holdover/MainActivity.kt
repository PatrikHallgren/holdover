package se.hallgren.holdover

import android.content.res.Configuration
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    PhoneBridge.attach(this)
    paintInsetBackground()
    val content = findViewById<View>(android.R.id.content)
    if (content != null) {
      ViewCompat.setOnApplyWindowInsetsListener(content) { _, insets ->
        // targetSdk 35+ draws edge-to-edge. Pad the WebView inside the status
        // bar, the camera cutout, and the gesture bar, in either orientation.
        val bars = insets.getInsets(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
        content.setPadding(bars.left, bars.top, bars.right, bars.bottom)
        WindowInsetsCompat.CONSUMED
      }
      ViewCompat.requestApplyInsets(content)
    }
  }

  override fun onResume() {
    super.onResume()
    findViewById<View>(android.R.id.content)?.let { ViewCompat.requestApplyInsets(it) }
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    paintInsetBackground()
  }

  override fun onDestroy() {
    PhoneBridge.detach(this)
    super.onDestroy()
  }

  private fun paintInsetBackground() {
    val night = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
    val color = if (night == Configuration.UI_MODE_NIGHT_YES) 0xFF1A1916.toInt() else 0xFFF7F4EE.toInt()
    window.decorView.setBackgroundColor(color)
    findViewById<View>(android.R.id.content)?.setBackgroundColor(color)
  }
}
