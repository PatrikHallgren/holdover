//! Phone sensors and the share sheet.
//!
//! Ballistics and group statistics stay in `holdover-core`. This module only
//! reads what the phone measured and formats it with the same unit helpers.
//! Desktop builds report that the sensors are absent.

use holdover_core::format_quantity;
use serde_json::{json, Value};

use crate::store::UnitSet;

pub fn present(raw: &Value, units: &UnitSet) -> Value {
    let mut out = raw.clone();
    out["label"] = json!("Measured (phone)");
    out["pressure_kind"] = json!("station");
    let mut missing = string_list(raw.get("missing"));
    if let Some(hpa) = raw.get("pressure_hpa").and_then(Value::as_f64) {
        match format_quantity(hpa * 100.0, &units.pressure) {
            Ok(text) => {
                out["pressure"] = json!({
                    "text": text,
                    "unit": units.pressure,
                    "kind": "station",
                });
            }
            Err(_) => missing.push("barometer".into()),
        }
    }
    if let Some(metres) = raw.get("altitude_m").and_then(Value::as_f64) {
        match format_quantity(metres, &units.length) {
            Ok(text) => {
                out["altitude"] = json!({ "text": text, "unit": units.length });
            }
            Err(_) => missing.push("gps".into()),
        }
    }
    angle_field(
        &mut out,
        raw,
        "azimuth_deg",
        "azimuth",
        &mut missing,
        "compass",
    );
    angle_field(
        &mut out,
        raw,
        "inclination_deg",
        "inclination",
        &mut missing,
        "inclinometer",
    );
    angle_field(
        &mut out,
        raw,
        "cant_deg",
        "cant",
        &mut missing,
        "inclinometer",
    );
    out["missing"] = json!(missing);
    out["ok"] = json!(true);
    out
}

fn angle_field(
    out: &mut Value,
    raw: &Value,
    source: &str,
    name: &str,
    missing: &mut Vec<String>,
    missing_name: &str,
) {
    let Some(degrees) = raw.get(source).and_then(Value::as_f64) else {
        return;
    };
    match format_quantity(degrees.to_radians(), "deg") {
        Ok(text) => {
            out[name] = json!({ "text": format!("{text} deg"), "unit": "deg" });
        }
        Err(_) => {
            if !missing.iter().any(|item| item == missing_name) {
                missing.push(missing_name.into());
            }
        }
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn read_sensors() -> Result<Value, String> {
    #[cfg(target_os = "android")]
    {
        android::call("readSensors", "")
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(json!({ "ok": false, "kind": "unavailable" }))
    }
}

pub fn prepare(kind: &str) -> Result<Value, String> {
    #[cfg(target_os = "android")]
    {
        android::call("prepare", kind)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = kind;
        Ok(json!({ "ok": true, "kind": "desktop" }))
    }
}

pub fn share_file(path: &str, mime: &str) -> Result<Value, String> {
    open_target(path, mime)
}

pub fn open_target(target: &str, mime: &str) -> Result<Value, String> {
    #[cfg(target_os = "android")]
    {
        android::open_target(target, mime)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (target, mime);
        Err("share-desktop".into())
    }
}

/// `Some("dark")` or `Some("light")` when the phone reports a night mode.
pub fn multicast(on: bool) {
    #[cfg(target_os = "android")]
    {
        let _ = android::call("setMulticast", if on { "1" } else { "0" });
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = on;
    }
}

pub fn log_line(line: &str) {
    #[cfg(target_os = "android")]
    {
        let _ = android::call("logLine", line);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = line;
    }
}

pub fn mdns_browse(wait: std::time::Duration) -> Option<Vec<holdover_lan::Found>> {
    #[cfg(target_os = "android")]
    {
        let millis = wait.as_millis().min(5000).to_string();
        let value = android::call("discoverMdns", &millis).unwrap_or_else(|_| json!([]));
        let rows = value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| {
                let host = row.get("host")?.as_str()?.trim().to_string();
                let port = row.get("port")?.as_u64()? as u16;
                let spki = row.get("fp")?.as_str()?.trim().to_string();
                if host.is_empty() || port == 0 || spki.len() != 64 {
                    return None;
                }
                Some(holdover_lan::Found {
                    name: row
                        .get("name")
                        .and_then(|item| item.as_str())
                        .unwrap_or("")
                        .to_string(),
                    host,
                    port,
                    device_id: row
                        .get("id")
                        .and_then(|item| item.as_str())
                        .unwrap_or("")
                        .to_string(),
                    spki_hex: spki,
                })
            })
            .collect();
        Some(rows)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = wait;
        None
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn mdns_advertise(port: u16, device_id: &str, name: &str, spki_hex: &str) {
    #[cfg(target_os = "android")]
    {
        let payload = json!({
            "port": port,
            "id": device_id,
            "name": name,
            "fp": spki_hex,
        })
        .to_string();
        let _ = android::call("advertiseMdns", &payload);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (port, device_id, name, spki_hex);
    }
}

pub fn mdns_stop() {
    #[cfg(target_os = "android")]
    {
        let _ = android::call("stopMdns", "");
    }
    #[cfg(not(target_os = "android"))]
    {}
}

pub fn system_mode() -> Option<&'static str> {
    #[cfg(target_os = "android")]
    {
        match android::night_mode().as_deref() {
            Ok("light") => Some("light"),
            Ok("dark") => Some("dark"),
            _ => Some("dark"),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{JObject, JString, JValue};
    use jni::JavaVM;
    use serde_json::{json, Value};

    fn with_env<T>(
        f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> Result<T, String>,
    ) -> Result<T, String> {
        let context = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.map_err(|err| err.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };
        let result = if activity.as_raw().is_null() {
            Err("the phone context is not ready".into())
        } else {
            f(&mut env, &activity)
        };
        // Detaching this thread with a pending exception aborts the process.
        // FindClass on a Rust thread throws ClassNotFoundException for app
        // classes; clear anything left before the attach guard drops.
        let _ = env.exception_clear();
        result
    }

    /// App classes are invisible to `FindClass` on a thread attached from
    /// Rust. The theme loop is one of those threads, and it asks for the night
    /// mode as soon as the window starts. Load `PhoneBridge` through the
    /// context's own class loader instead.
    fn bridge_class<'a>(
        env: &mut jni::JNIEnv<'a>,
        context: &JObject<'_>,
    ) -> Result<jni::objects::JClass<'a>, String> {
        let loader = env
            .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .map_err(|err| err.to_string())?;
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Err("the phone context has no class loader".into());
        }
        let loader = loader.l().map_err(|err| err.to_string())?;
        let name = env
            .new_string("se.hallgren.holdover.PhoneBridge")
            .map_err(|err| err.to_string())?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )
            .map_err(|err| err.to_string())?;
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Err("PhoneBridge is not on the app class loader".into());
        }
        Ok(class.l().map_err(|err| err.to_string())?.into())
    }

    fn check(env: &mut jni::JNIEnv) -> Result<(), String> {
        if env.exception_check().map_err(|err| err.to_string())? {
            env.exception_clear().map_err(|err| err.to_string())?;
            return Err("the phone call failed".into());
        }
        Ok(())
    }

    fn jstring(env: &mut jni::JNIEnv, value: JObject) -> Result<String, String> {
        let text: String = env
            .get_string(&JString::from(value))
            .map_err(|err| err.to_string())?
            .into();
        Ok(text)
    }

    pub fn call(method: &str, payload: &str) -> Result<Value, String> {
        let text = with_env(|env, activity| {
            let class = bridge_class(env, activity)?;
            let body = env.new_string(payload).map_err(|err| err.to_string())?;
            let value = env
                .call_static_method(
                    class,
                    method,
                    "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
                    &[JValue::Object(activity), JValue::Object(&body)],
                )
                .map_err(|err| err.to_string())?;
            check(env)?;
            let object = value.l().map_err(|err| err.to_string())?;
            jstring(env, object)
        })?;
        serde_json::from_str(&text).map_err(|err| err.to_string())
    }

    pub fn open_target(target: &str, mime: &str) -> Result<Value, String> {
        let text = with_env(|env, activity| {
            let class = bridge_class(env, activity)?;
            let jtarget = env.new_string(target).map_err(|err| err.to_string())?;
            let jmime = env.new_string(mime).map_err(|err| err.to_string())?;
            let value = env
                .call_static_method(
                    class,
                    "openTarget",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(activity),
                        JValue::Object(&jtarget),
                        JValue::Object(&jmime),
                    ],
                )
                .map_err(|err| err.to_string())?;
            check(env)?;
            let object = value.l().map_err(|err| err.to_string())?;
            jstring(env, object)
        })?;
        let parsed: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "ok": false }));
        if parsed.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(parsed)
        } else {
            Err(parsed
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the phone could not open that file")
                .to_string())
        }
    }

    pub fn night_mode() -> Result<String, String> {
        with_env(|env, activity| {
            let class = bridge_class(env, activity)?;
            let value = env
                .call_static_method(
                    class,
                    "nightMode",
                    "(Landroid/content/Context;)Ljava/lang/String;",
                    &[JValue::Object(activity)],
                )
                .map_err(|err| err.to_string())?;
            check(env)?;
            let object = value.l().map_err(|err| err.to_string())?;
            jstring(env, object)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::present;
    use crate::store::UnitSet;
    use serde_json::json;

    #[test]
    fn a_barometer_reading_is_station_pressure_in_the_active_unit() {
        let raw = json!({
            "ok": true,
            "pressure_hpa": 1013.25,
            "altitude_m": 42.0,
            "azimuth_deg": 270.0,
            "inclination_deg": 1.5,
            "cant_deg": -0.25,
            "missing": ["temperature"]
        });
        let shown = present(&raw, &UnitSet::metric());
        assert_eq!(shown["label"], "Measured (phone)");
        assert_eq!(shown["pressure"]["kind"], "station");
        assert_eq!(shown["pressure"]["unit"], "hPa");
        let pressure: f64 = shown["pressure"]["text"].as_str().unwrap().parse().unwrap();
        assert!((pressure - 1013.25).abs() < 0.2, "{pressure}");
        assert_eq!(shown["altitude"]["unit"], "m");
        assert!(shown["azimuth"]["text"].as_str().unwrap().contains("270"));
        assert!(shown["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "temperature"));
    }

    #[test]
    fn the_bridge_is_loaded_through_the_app_class_loader() {
        let source = include_str!("phone.rs");
        let code = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            !code.contains("find_class"),
            "FindClass on a Rust thread cannot see se.hallgren.holdover.PhoneBridge"
        );
        assert!(code.contains("getClassLoader"));
        assert!(code.contains("loadClass"));
        assert!(code.contains("se.hallgren.holdover.PhoneBridge"));
    }

    #[test]
    fn a_reading_without_a_barometer_does_not_invent_pressure() {
        let shown = present(&json!({ "ok": true, "azimuth_deg": 10.0 }), &UnitSet::us());
        assert!(shown.get("pressure").is_none());
        assert!(shown["azimuth"]["text"].as_str().unwrap().ends_with("deg"));
    }
}
