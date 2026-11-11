package se.hallgren.holdover

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.net.Uri
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.Inet4Address
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

object PhoneBridge {
  private const val REQUEST_SENSORS = 4401
  private const val REQUEST_CAMERA = 4402
  private const val TAG = "Holdover"
  private const val SERVICE_TYPE = "_holdover._tcp."
  private var attached: MainActivity? = null
  private var multicast: WifiManager.MulticastLock? = null
  private var nsd: NsdManager? = null
  private var registration: NsdManager.RegistrationListener? = null
  // Callbacks stay off the main looper. A blocked command thread must not
  // swallow discovery. An always-on VPN can still hide multicast.
  private val nsdExecutor = Executors.newSingleThreadExecutor { runnable ->
    Thread(runnable, "holdover-nsd")
  }

  fun attach(activity: MainActivity) {
    attached = activity
  }

  fun detach(activity: MainActivity) {
    if (attached == activity) attached = null
  }

  private fun activity(context: Context): Activity? {
    return (context as? Activity) ?: attached
  }

  @JvmStatic
  fun setMulticast(context: Context, on: String): String {
    return try {
      val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
      val lock = multicast ?: wifi.createMulticastLock("holdover").also {
        it.setReferenceCounted(false)
        multicast = it
      }
      if (on == "1") {
        if (!lock.isHeld) lock.acquire()
      } else if (lock.isHeld) {
        lock.release()
      }
      JSONObject().put("ok", true).toString()
    } catch (err: Exception) {
      fail("multicast", err.message ?: "multicast lock failed")
    }
  }

  @JvmStatic
  fun logLine(context: Context, line: String): String {
    Log.i(TAG, line.take(300))
    return JSONObject().put("ok", true).toString()
  }

  @JvmStatic
  fun advertiseMdns(context: Context, payload: String): String {
    return try {
      holdMulticast(context)
      stopMdns(context, "")
      val body = JSONObject(payload)
      val info = NsdServiceInfo()
      val id = body.optString("id")
      info.serviceName = "holdover-" + id.take(8)
      info.serviceType = SERVICE_TYPE
      info.port = body.optInt("port")
      info.setAttribute("id", id)
      info.setAttribute("fp", body.optString("fp"))
      info.setAttribute("name", body.optString("name").take(40))
      info.setAttribute("schema", "1")
      val manager = context.applicationContext.getSystemService(Context.NSD_SERVICE) as NsdManager
      val listener = object : NsdManager.RegistrationListener {
        override fun onServiceRegistered(service: NsdServiceInfo) {
          Log.i(TAG, "lan action=advertise result=ok port=" + service.port)
        }
        override fun onRegistrationFailed(service: NsdServiceInfo, code: Int) {
          Log.i(TAG, "lan action=advertise result=mdns-unavailable port=" + service.port)
        }
        override fun onServiceUnregistered(service: NsdServiceInfo) {}
        override fun onUnregistrationFailed(service: NsdServiceInfo, code: Int) {}
      }
      manager.registerService(info, NsdManager.PROTOCOL_DNS_SD, listener)
      nsd = manager
      registration = listener
      JSONObject().put("ok", true).toString()
    } catch (err: Exception) {
      Log.i(TAG, "lan action=advertise result=mdns-unavailable port=0")
      fail("mdns", "discovery did not start")
    }
  }

  @JvmStatic
  fun stopMdns(context: Context, ignored: String): String {
    val listener = registration
    val manager = nsd ?: context.applicationContext.getSystemService(Context.NSD_SERVICE) as NsdManager
    if (listener != null) {
      try {
        manager.unregisterService(listener)
      } catch (_: Exception) {
      }
    }
    registration = null
    nsd = null
    return JSONObject().put("ok", true).toString()
  }

  @JvmStatic
  fun discoverMdns(context: Context, waitMs: String): String {
    val found = JSONArray()
    val manager = context.applicationContext.getSystemService(Context.NSD_SERVICE) as NsdManager
    val latch = CountDownLatch(1)
    val listener = object : NsdManager.DiscoveryListener {
      override fun onDiscoveryStarted(serviceType: String) {}
      override fun onStartDiscoveryFailed(serviceType: String, code: Int) {
        Log.i(TAG, "lan action=discover result=mdns-unavailable port=0")
        latch.countDown()
      }
      override fun onStopDiscoveryFailed(serviceType: String, code: Int) {}
      override fun onDiscoveryStopped(serviceType: String) {
        latch.countDown()
      }
      override fun onServiceLost(service: NsdServiceInfo) {}
      override fun onServiceFound(service: NsdServiceInfo) {
        if (!service.serviceType.contains("_holdover._tcp")) return
        resolve(manager, service, found)
      }
    }
    return try {
      holdMulticast(context)
      if (Build.VERSION.SDK_INT >= 34) {
        manager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, nsdExecutor, listener)
      } else {
        manager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
      }
      val wait = waitMs.toLongOrNull() ?: 2000L
      latch.await(wait.coerceIn(500L, 5000L), TimeUnit.MILLISECONDS)
      try {
        manager.stopServiceDiscovery(listener)
      } catch (_: Exception) {
      }
      Thread.sleep(800)
      synchronized(found) { found.toString() }
    } catch (err: Exception) {
      Log.i(TAG, "lan action=discover result=mdns-unavailable port=0")
      found.toString()
    }
  }

  private fun holdMulticast(context: Context) {
    try {
      setMulticast(context, "1")
    } catch (_: Exception) {
    }
  }

  private fun resolve(manager: NsdManager, service: NsdServiceInfo, found: JSONArray) {
    val listener = object : NsdManager.ResolveListener {
      override fun onResolveFailed(info: NsdServiceInfo, code: Int) {}
      override fun onServiceResolved(info: NsdServiceInfo) {
        val host = serviceHost(info) ?: return
        val row = JSONObject()
        row.put("host", host)
        row.put("port", info.port)
        row.put("name", textAttr(info, "name"))
        row.put("id", textAttr(info, "id"))
        row.put("fp", textAttr(info, "fp"))
        synchronized(found) { found.put(row) }
      }
    }
    try {
      if (Build.VERSION.SDK_INT >= 34) {
        manager.resolveService(service, nsdExecutor, listener)
      } else {
        manager.resolveService(service, listener)
      }
    } catch (_: Exception) {
    }
  }

  private fun serviceHost(info: NsdServiceInfo): String? {
    if (Build.VERSION.SDK_INT >= 34) {
      val addresses = info.hostAddresses
      val v4 = addresses.firstOrNull { it is Inet4Address }
      val chosen = v4 ?: addresses.firstOrNull()
      if (chosen != null) return chosen.hostAddress
    }
    return info.host?.hostAddress
  }

  private fun textAttr(info: NsdServiceInfo, key: String): String {
    val bytes = info.attributes?.get(key) ?: return ""
    return String(bytes, Charsets.UTF_8)
  }

  @JvmStatic
  fun nightMode(context: Context): String {
    val night = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
    return if (night == Configuration.UI_MODE_NIGHT_YES) "dark" else "light"
  }

  @JvmStatic
  fun prepare(context: Context, kind: String): String {
    val activity = activity(context) ?: return fail("no-activity", "Holdover is not in the foreground.")
    val permissions = if (kind == "camera") {
      arrayOf(Manifest.permission.CAMERA)
    } else {
      arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION)
    }
    val missing = permissions.filter {
      ContextCompat.checkSelfPermission(activity, it) != PackageManager.PERMISSION_GRANTED
    }
    if (missing.isEmpty()) return JSONObject().put("ok", true).toString()
    val code = if (kind == "camera") REQUEST_CAMERA else REQUEST_SENSORS
    activity.runOnUiThread {
      activity.requestPermissions(missing.toTypedArray(), code)
    }
    return JSONObject()
      .put("ok", false)
      .put("kind", "permission")
      .put("missing", JSONArray(missing))
      .toString()
  }

  @JvmStatic
  fun readSensors(context: Context, ignored: String): String {
    val activity = activity(context) ?: return fail("no-activity", "Holdover is not in the foreground.")
    val prepared = JSONObject(prepare(activity, "sensors"))
    if (!prepared.optBoolean("ok")) return prepared.toString()
    val body = JSONObject().put("ok", true)
    val missing = JSONArray()
    sampleMotion(activity, body, missing)
    sampleLocation(activity, body, missing)
    body.put("missing", missing)
    return body.toString()
  }

  @JvmStatic
  fun openTarget(context: Context, target: String, mime: String): String {
    val activity = activity(context) ?: return fail("no-activity", "Holdover is not in the foreground.")
    return try {
      if (target.startsWith("http://") || target.startsWith("https://")) {
        val view = Intent(Intent.ACTION_VIEW, Uri.parse(target)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        activity.startActivity(view)
        JSONObject().put("ok", true).toString()
      } else {
        shareFile(activity, target, mime)
      }
    } catch (err: Exception) {
      fail("open", err.message ?: "could not open")
    }
  }

  private fun shareFile(activity: Activity, path: String, mime: String): String {
    val source = File(path)
    if (!source.isFile) return fail("missing", "the file is not on this phone")
    val dest = File(activity.cacheDir, source.name)
    source.copyTo(dest, overwrite = true)
    val uri = FileProvider.getUriForFile(activity, activity.packageName + ".fileprovider", dest)
    val send = Intent(Intent.ACTION_SEND).apply {
      type = if (mime.isBlank()) "application/octet-stream" else mime
      putExtra(Intent.EXTRA_STREAM, uri)
      putExtra(Intent.EXTRA_TITLE, dest.name)
      clipData = ClipData.newRawUri(dest.name, uri)
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    val chooser = Intent.createChooser(send, "Share Holdover").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    activity.startActivity(chooser)
    return JSONObject().put("ok", true).toString()
  }

  private fun sampleMotion(activity: Activity, body: JSONObject, missing: JSONArray) {
    val manager = activity.getSystemService(SensorManager::class.java)
    val pressure = manager.getDefaultSensor(Sensor.TYPE_PRESSURE)
    val rotation = manager.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR)
    if (pressure == null) missing.put("barometer")
    if (rotation == null) missing.put("compass")
    if (pressure == null && rotation == null) return
    val latch = CountDownLatch(1)
    val listener = object : SensorEventListener {
      var needPressure = pressure != null
      var needRotation = rotation != null
      override fun onSensorChanged(event: SensorEvent) {
        if (event.sensor.type == Sensor.TYPE_PRESSURE && needPressure) {
          body.put("pressure_hpa", event.values[0].toDouble())
          needPressure = false
        }
        if (event.sensor.type == Sensor.TYPE_ROTATION_VECTOR && needRotation) {
          val matrix = FloatArray(9)
          val orientation = FloatArray(3)
          SensorManager.getRotationMatrixFromVector(matrix, event.values)
          SensorManager.getOrientation(matrix, orientation)
          var azimuth = Math.toDegrees(orientation[0].toDouble())
          if (azimuth < 0) azimuth += 360.0
          body.put("azimuth_deg", azimuth)
          body.put("inclination_deg", Math.toDegrees(orientation[1].toDouble()))
          body.put("cant_deg", Math.toDegrees(orientation[2].toDouble()))
          needRotation = false
        }
        if (!needPressure && !needRotation) latch.countDown()
      }
      override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {}
    }
    activity.runOnUiThread {
      pressure?.let { manager.registerListener(listener, it, SensorManager.SENSOR_DELAY_NORMAL) }
      rotation?.let { manager.registerListener(listener, it, SensorManager.SENSOR_DELAY_NORMAL) }
    }
    latch.await(1500, TimeUnit.MILLISECONDS)
    activity.runOnUiThread { manager.unregisterListener(listener) }
    if (pressure != null && !body.has("pressure_hpa")) missing.put("barometer")
    if (rotation != null && !body.has("azimuth_deg")) missing.put("compass")
  }

  private fun sampleLocation(activity: Activity, body: JSONObject, missing: JSONArray) {
    val manager = activity.getSystemService(LocationManager::class.java)
    val providers = listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER)
    var best: Location? = null
    for (provider in providers) {
      val enabled = try { manager.isProviderEnabled(provider) } catch (err: Exception) { false }
      if (!enabled) continue
      val fix = try { manager.getLastKnownLocation(provider) } catch (err: SecurityException) { null }
      if (fix != null && (best == null || fix.time > best!!.time)) best = fix
    }
    if (best == null) {
      val latch = CountDownLatch(1)
      val listener = LocationListener { location ->
        best = location
        latch.countDown()
      }
      val provider = providers.firstOrNull { name ->
        try { manager.isProviderEnabled(name) } catch (err: Exception) { false }
      }
      if (provider != null) {
        try {
          activity.runOnUiThread {
            try {
              manager.requestLocationUpdates(provider, 0L, 0f, listener)
            } catch (err: Exception) {
              latch.countDown()
            }
          }
          latch.await(6000, TimeUnit.MILLISECONDS)
        } catch (err: Exception) {
          missing.put("gps")
        } finally {
          try { manager.removeUpdates(listener) } catch (err: Exception) { }
        }
      }
    }
    val fix = best
    if (fix == null) {
      missing.put("gps")
      return
    }
    body.put("latitude_deg", fix.latitude)
    body.put("longitude_deg", fix.longitude)
    if (fix.hasAltitude()) body.put("altitude_m", fix.altitude)
  }

  private fun fail(kind: String, message: String): String {
    return JSONObject().put("ok", false).put("kind", kind).put("message", message).toString()
  }
}
