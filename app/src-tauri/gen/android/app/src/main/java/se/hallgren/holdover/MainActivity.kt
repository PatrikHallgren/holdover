package se.hallgren.holdover

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    PhoneBridge.attach(this)
  }

  override fun onDestroy() {
    PhoneBridge.detach(this)
    super.onDestroy()
  }
}
