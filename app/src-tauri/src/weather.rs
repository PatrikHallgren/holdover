//! Open-Meteo lookup for the desktop shell.
//!
//! `holdover-core` does no network I/O. This module is the only place that
//! calls the weather service. Holdover is personal and non-commercial, which
//! is the free-tier condition. Station pressure for the solver is
//! `surface_pressure`. `pressure_msl` is labelled sea-level and is not used
//! unless the user picks it. Model wind is context until the user accepts the
//! speed. There is no background polling.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use holdover_core::{format_number, format_quantity};
use serde_json::{json, Value};

pub const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(4);
pub const ATTRIBUTION: &str = "Weather data by Open-Meteo (https://open-meteo.com/), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).";

const CURRENT: &str = "temperature_2m,relative_humidity_2m,surface_pressure,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m";

#[derive(Clone, Debug)]
pub struct FetchedWeather {
    pub temperature_c: f64,
    pub humidity: f64,
    pub surface_pressure_pa: f64,
    pub sea_level_pressure_pa: f64,
    pub elevation_m: f64,
    pub wind_speed_mps: Option<f64>,
    pub wind_from_deg: Option<f64>,
    pub wind_gust_mps: Option<f64>,
    pub valid_time: String,
    pub fetched_at: String,
    pub grid_latitude: f64,
    pub grid_longitude: f64,
}

#[derive(Clone, Debug)]
pub struct WeatherFailure {
    pub kind: &'static str,
    pub detail: String,
}

pub struct PresentUnits<'a> {
    pub temperature: &'a str,
    pub pressure: &'a str,
    pub length: &'a str,
    pub velocity: &'a str,
}

pub fn place_missing() -> Value {
    json!({ "ok": false, "kind": "place" })
}

impl FetchedWeather {
    pub fn presented(&self, units: PresentUnits<'_>) -> Value {
        json!({
            "ok": true,
            "source": "fetched",
            "provider": "open-meteo",
            "pressure_used": "station",
            "pressure_field": "surface_pressure",
            "temperature_c": self.temperature_c,
            "humidity": self.humidity,
            "surface_pressure_pa": self.surface_pressure_pa,
            "sea_level_pressure_pa": self.sea_level_pressure_pa,
            "elevation_m": self.elevation_m,
            "wind_speed_mps": self.wind_speed_mps,
            "wind_from_deg": self.wind_from_deg,
            "wind_gust_mps": self.wind_gust_mps,
            "valid_time": self.valid_time,
            "fetched_at": self.fetched_at,
            "grid_latitude": self.grid_latitude,
            "grid_longitude": self.grid_longitude,
            "temperature_text": quantity_text(self.temperature_c, units.temperature),
            "temperature_unit": units.temperature,
            "pressure_text": quantity_text(self.surface_pressure_pa, units.pressure),
            "pressure_unit": units.pressure,
            "sea_level_text": quantity_text(self.sea_level_pressure_pa, units.pressure),
            "elevation_text": quantity_text(self.elevation_m, units.length),
            "elevation_unit": units.length,
            "wind_text": self.wind_speed_mps.map(|value| quantity_text(value, units.velocity)),
            "wind_unit": units.velocity,
            "gust_text": self.wind_gust_mps.map(|value| quantity_text(value, units.velocity)),
            "attribution": ATTRIBUTION,
        })
    }
}

impl WeatherFailure {
    fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn body(&self) -> Value {
        json!({ "ok": false, "kind": self.kind, "detail": self.detail })
    }
}

/// Age of a fetch, in seconds, and whether it is past `stale_after_secs`.
pub fn weather_age(
    fetched_at: &str,
    now: &str,
    stale_after_secs: u64,
) -> Result<(u64, bool), String> {
    let fetched = parse_utc(fetched_at)?;
    let current = parse_utc(now)?;
    let age = current.saturating_sub(fetched) as u64;
    Ok((age, age > stale_after_secs))
}

pub fn fetch(
    latitude: f64,
    longitude: f64,
    timeout: Duration,
    base: &str,
) -> Result<FetchedWeather, WeatherFailure> {
    if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
        return Err(WeatherFailure::new("place", "latitude"));
    }
    if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
        return Err(WeatherFailure::new("place", "longitude"));
    }
    let url = format!(
        "{base}?latitude={lat}&longitude={lon}&current={CURRENT}&wind_speed_unit=ms&timezone=GMT&forecast_days=1",
        lat = format_coord(latitude),
        lon = format_coord(longitude),
    );
    let body = if base.starts_with("https://") {
        http_get(&url, timeout)?
    } else {
        http_get_plain(&url, timeout)?
    };
    parse_open_meteo(&body, &utc_now())
}

pub fn parse_open_meteo(body: &str, fetched_at: &str) -> Result<FetchedWeather, WeatherFailure> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| WeatherFailure::new("parse", "json"))?;
    let current = value
        .get("current")
        .ok_or_else(|| WeatherFailure::new("parse", "current"))?;
    let temperature_c = number_at(current, "temperature_2m")?;
    let humidity_pct = number_at(current, "relative_humidity_2m")?;
    let surface_hpa = number_at(current, "surface_pressure")?;
    let sea_hpa = number_at(current, "pressure_msl")?;
    let elevation_m = number_at(&value, "elevation")?;
    let grid_latitude = number_at(&value, "latitude")?;
    let grid_longitude = number_at(&value, "longitude")?;
    if !(0.0..=100.0).contains(&humidity_pct) {
        return Err(WeatherFailure::new("parse", "humidity"));
    }
    let valid_time = current
        .get("time")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if valid_time.is_empty() {
        return Err(WeatherFailure::new("parse", "time"));
    }
    Ok(FetchedWeather {
        temperature_c,
        humidity: humidity_pct / 100.0,
        surface_pressure_pa: surface_hpa * 100.0,
        sea_level_pressure_pa: sea_hpa * 100.0,
        elevation_m,
        wind_speed_mps: optional_number(current, "wind_speed_10m"),
        wind_from_deg: optional_number(current, "wind_direction_10m"),
        wind_gust_mps: optional_number(current, "wind_gusts_10m"),
        valid_time,
        fetched_at: fetched_at.to_string(),
        grid_latitude,
        grid_longitude,
    })
}

fn http_get(url: &str, timeout: Duration) -> Result<String, WeatherFailure> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout(timeout)
        .build();
    match agent
        .get(url)
        .set("User-Agent", "Holdover/0.1 (personal, non-commercial)")
        .set("Accept", "application/json")
        .call()
    {
        Ok(response) => response
            .into_string()
            .map_err(|_| WeatherFailure::new("parse", "body")),
        Err(ureq::Error::Status(_, _)) => Err(WeatherFailure::new("http", "status")),
        Err(ureq::Error::Transport(err)) => Err(classify_transport(&err.to_string())),
    }
}

/// HTTP used by the offline and timeout tests. The public fetch uses `https`.
fn http_get_plain(url: &str, timeout: Duration) -> Result<String, WeatherFailure> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| WeatherFailure::new("offline", "url"))?;
    let (host_port, path) = rest
        .split_once('/')
        .ok_or_else(|| WeatherFailure::new("offline", "url"))?;
    let mut stream = TcpStream::connect_timeout(
        &host_port
            .parse()
            .map_err(|_| WeatherFailure::new("offline", "address"))?,
        timeout,
    )
    .map_err(|err| classify_transport(&err.to_string()))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|err| classify_transport(&err.to_string()))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|err| classify_transport(&err.to_string()))?;
    let request = format!(
        "GET /{path} HTTP/1.1\r\nHost: {host_port}\r\nUser-Agent: Holdover/0.1 (personal, non-commercial)\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| classify_transport(&err.to_string()))?;
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).map_err(|err| {
        if err.kind() == std::io::ErrorKind::TimedOut
            || err.kind() == std::io::ErrorKind::WouldBlock
        {
            WeatherFailure::new("timeout", err.to_string())
        } else {
            classify_transport(&err.to_string())
        }
    })?;
    let text = String::from_utf8_lossy(&buffer);
    let Some((_, body)) = text.split_once("\r\n\r\n") else {
        return Err(WeatherFailure::new("parse", "http"));
    };
    let status = text
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("");
    if status != "200" {
        return Err(WeatherFailure::new("http", status));
    }
    Ok(body.to_string())
}

fn classify_transport(message: &str) -> WeatherFailure {
    let lower = message.to_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        WeatherFailure::new("timeout", message)
    } else {
        WeatherFailure::new("offline", message)
    }
}

fn number_at(value: &Value, key: &str) -> Result<f64, WeatherFailure> {
    value
        .get(key)
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
        .ok_or_else(|| WeatherFailure::new("parse", key))
}

fn optional_number(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
}

fn quantity_text(si: f64, unit: &str) -> String {
    match format_quantity(si, unit) {
        Ok(number) => format!("{number} {unit}"),
        Err(_) => format!("{} {unit}", format_number(si)),
    }
}

fn format_coord(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text.is_empty() || text == "-" {
        "0".into()
    } else {
        text
    }
}

fn utc_now() -> String {
    crate::store::now_iso()
}

fn parse_utc(text: &str) -> Result<i64, String> {
    let bytes = text.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return Err("time".into());
    }
    let year: i64 = text[0..4].parse().map_err(|_| "time")?;
    let month: i64 = text[5..7].parse().map_err(|_| "time")?;
    let day: i64 = text[8..10].parse().map_err(|_| "time")?;
    let hour: i64 = text[11..13].parse().map_err(|_| "time")?;
    let minute: i64 = text[14..16].parse().map_err(|_| "time")?;
    let second: i64 = text[17..19].parse().map_err(|_| "time")?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || !(0..=23).contains(&hour) {
        return Err("time".into());
    }
    let days = days_from_civil(year, month, day);
    Ok(days * 86_400 + hour * 3600 + minute * 60 + second)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    (era * 146_097 + doe as i64) - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    const SAMPLE: &str = r#"{
      "latitude": 59.3294,
      "longitude": 18.0686,
      "elevation": 28.0,
      "current": {
        "time": "2026-10-21T01:00",
        "interval": 900,
        "temperature_2m": 7.4,
        "relative_humidity_2m": 86,
        "surface_pressure": 1004.2,
        "pressure_msl": 1007.8,
        "wind_speed_10m": 4.5,
        "wind_direction_10m": 210,
        "wind_gusts_10m": 9.1
      }
    }"#;

    #[test]
    fn surface_pressure_is_station_and_elevation_is_kept() {
        let weather = parse_open_meteo(SAMPLE, "2026-10-21T01:00:12Z").unwrap();
        assert!((weather.surface_pressure_pa - 100_420.0).abs() < 1e-6);
        assert!((weather.sea_level_pressure_pa - 100_780.0).abs() < 1e-6);
        assert!((weather.elevation_m - 28.0).abs() < 1e-9);
        assert!((weather.temperature_c - 7.4).abs() < 1e-9);
        assert!((weather.humidity - 0.86).abs() < 1e-9);
        assert_eq!(weather.wind_speed_mps, Some(4.5));
        assert_eq!(weather.valid_time, "2026-10-21T01:00");
        let shown = weather.presented(PresentUnits {
            temperature: "C",
            pressure: "hPa",
            length: "m",
            velocity: "mps",
        });
        assert_eq!(shown["pressure_field"], "surface_pressure");
        assert_eq!(shown["pressure_used"], "station");
        assert_eq!(shown["ok"], true);
        assert!(shown["pressure_text"].as_str().unwrap().contains("1004.2"));
        assert!(shown["sea_level_text"].as_str().unwrap().contains("1007.8"));
        assert!(shown["elevation_text"].as_str().unwrap().ends_with(" m"));
    }

    #[test]
    fn us_units_relabel_the_same_station_pressure() {
        let weather = parse_open_meteo(SAMPLE, "2026-10-21T01:00:12Z").unwrap();
        let shown = weather.presented(PresentUnits {
            temperature: "F",
            pressure: "inHg",
            length: "ft",
            velocity: "mph",
        });
        assert!(shown["temperature_text"].as_str().unwrap().ends_with(" F"));
        assert!(shown["pressure_text"].as_str().unwrap().ends_with(" inHg"));
        assert!(shown["elevation_text"].as_str().unwrap().ends_with(" ft"));
        assert!(shown["wind_text"].as_str().unwrap().ends_with(" mph"));
        assert_eq!(shown["surface_pressure_pa"], 100420.0);
    }

    #[test]
    fn stale_badge_uses_age() {
        let (age, stale) =
            weather_age("2026-10-21T00:00:00Z", "2026-10-21T02:00:00Z", 3600).unwrap();
        assert_eq!(age, 7200);
        assert!(stale);
        let (fresh, not_stale) =
            weather_age("2026-10-21T00:00:00Z", "2026-10-21T00:30:00Z", 3600).unwrap();
        assert_eq!(fresh, 1800);
        assert!(!not_stale);
    }

    #[test]
    fn attribution_is_cc_by_and_on_the_english_screen() {
        assert!(ATTRIBUTION.contains("https://open-meteo.com/"));
        assert!(ATTRIBUTION.contains("CC BY 4.0"));
        assert!(ATTRIBUTION.contains("https://creativecommons.org/licenses/by/4.0/"));
        let ftl = include_str!("../../../locales/en.ftl");
        let line = ftl
            .lines()
            .find(|line| line.starts_with("weather-attribution = "))
            .unwrap();
        assert!(line.contains("https://open-meteo.com/"));
        assert!(line.contains("CC BY 4.0"));
        assert!(line.contains("https://creativecommons.org/licenses/by/4.0/"));
        assert!(ftl.contains("weather-noncommercial = "));
    }

    #[test]
    fn offline_is_a_visible_failure_kind() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let err = fetch(
            59.3,
            18.0,
            Duration::from_millis(400),
            &format!("http://127.0.0.1:{port}/forecast"),
        )
        .unwrap_err();
        assert_eq!(err.kind, "offline");
        assert_eq!(err.body()["ok"], false);
    }

    #[test]
    fn timeout_is_a_visible_failure_kind() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf);
            thread::sleep(Duration::from_secs(2));
        });
        let err = fetch(
            59.3,
            18.0,
            Duration::from_millis(200),
            &format!("http://127.0.0.1:{port}/forecast"),
        )
        .unwrap_err();
        assert_eq!(err.kind, "timeout", "{}", err.detail);
        let _ = server.join();
    }

    #[test]
    fn a_local_forecast_parses_and_does_not_drop_wind() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = SAMPLE.to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 2048];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let weather = fetch(
            59.3,
            18.0,
            Duration::from_secs(2),
            &format!("http://127.0.0.1:{port}/forecast"),
        )
        .unwrap();
        assert!((weather.surface_pressure_pa - 100_420.0).abs() < 1e-6);
        assert_eq!(weather.wind_from_deg, Some(210.0));
        let _ = server.join();
    }
}
