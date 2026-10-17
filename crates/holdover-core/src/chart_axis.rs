//! Tick placement for the drop and wind charts.
//!
//! Steps are 1, 2, 2.5, or 5 times a power of ten, so a 0–1000 m axis is
//! labelled 0, 200, 400… rather than 0, 166.7, 333.3.

#[derive(Clone, Debug, PartialEq)]
pub struct ChartAxis {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub ticks: Vec<f64>,
    pub labels: Vec<String>,
}

/// Nice step that splits `span` into at most `max_ticks` intervals.
pub fn chart_tick_step(span: f64, max_ticks: u32) -> f64 {
    let span = span.abs();
    let max_ticks = max_ticks.clamp(2, 12);
    if !span.is_finite() || span == 0.0 {
        return 1.0;
    }
    let raw = span / f64::from(max_ticks);
    let exp = raw.log10().floor();
    let base = 10.0_f64.powf(exp);
    if !base.is_finite() || base == 0.0 {
        return 1.0;
    }
    let frac = raw / base;
    let nice = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .find(|step| *step + 1e-9 >= frac)
        .unwrap_or(10.0);
    let step = nice * base;
    if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    }
}

/// Axis covering the data. `include_zero` pulls 0 into the scale so a
/// line-of-sight or no-wind reference can be drawn on a tick.
pub fn chart_axis(data_min: f64, data_max: f64, max_ticks: u32, include_zero: bool) -> ChartAxis {
    let max_ticks = max_ticks.clamp(2, 12);
    let mut lo = data_min.min(data_max);
    let mut hi = data_max.max(data_min);
    if !lo.is_finite() || !hi.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    if include_zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    if hi - lo < 1e-9 {
        lo -= 1.0;
        hi += 1.0;
        if include_zero {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        if hi - lo < 1e-9 {
            hi = lo + 1.0;
        }
    }
    let step = chart_tick_step(hi - lo, max_ticks);
    let start = snap_tick((lo / step).floor() * step, step);
    let end = snap_tick((hi / step).ceil() * step, step);
    let mut ticks = Vec::new();
    let mut labels = Vec::new();
    let mut guard = 0;
    let mut value = start;
    while value <= end + step * 1e-6 && guard < 48 {
        let snapped = snap_tick(value, step);
        if ticks
            .last()
            .is_none_or(|prev: &f64| (snapped - prev).abs() > step * 1e-6)
        {
            labels.push(format_tick(snapped, step));
            ticks.push(snapped);
        }
        value += step;
        guard += 1;
    }
    if ticks.is_empty() {
        ticks.push(0.0);
        labels.push("0".into());
    }
    let min = ticks[0];
    let max = *ticks.last().unwrap_or(&min);
    ChartAxis {
        min,
        max,
        step,
        ticks,
        labels,
    }
}

fn snap_tick(value: f64, step: f64) -> f64 {
    let decimals = tick_decimals(step);
    let scale = 10.0_f64.powi(decimals);
    (value * scale).round() / scale
}

fn tick_decimals(step: f64) -> i32 {
    if step <= 0.0 || !step.is_finite() {
        return 0;
    }
    let mut scaled = step;
    let mut decimals = 0;
    while decimals < 8 && (scaled - scaled.round()).abs() > 1e-6 {
        scaled *= 10.0;
        decimals += 1;
    }
    decimals
}

pub fn format_tick(value: f64, step: f64) -> String {
    let snapped = snap_tick(value, step);
    if !snapped.is_finite() || snapped.abs() < step.abs().max(1.0) * 1e-9 {
        return "0".into();
    }
    let decimals = tick_decimals(step) as usize;
    let mut text = format!("{snapped:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".into()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thousand_metres_steps_by_two_hundred() {
        assert_eq!(chart_tick_step(1000.0, 6), 200.0);
        let axis = chart_axis(0.0, 1000.0, 6, true);
        assert_eq!(axis.step, 200.0);
        assert_eq!(axis.labels, ["0", "200", "400", "600", "800", "1000"]);
    }

    #[test]
    fn centimetres_and_milliradians_use_a_coarse_step() {
        assert_eq!(chart_tick_step(30.0, 5), 10.0);
        assert_eq!(chart_tick_step(0.8, 5), 0.2);
        let axis = chart_axis(-42.0, 2.0, 5, true);
        assert_eq!(axis.step, 10.0);
        assert!(axis.labels.iter().any(|label| label == "0"));
        assert!(axis.min <= -42.0);
        assert!(axis.max >= 2.0);
    }

    #[test]
    fn steps_stay_on_the_one_two_two_point_five_five_ladder() {
        for span in [0.03, 0.8, 1.0, 7.5, 30.0, 100.0, 250.0, 1000.0, 2345.0] {
            for max in [4, 5, 6, 8] {
                let step = chart_tick_step(span, max);
                assert!(step.is_finite() && step > 0.0, "{span} {step}");
                let intervals = (span / step).ceil();
                assert!(
                    intervals <= f64::from(max) + 1e-6,
                    "span {span} step {step} intervals {intervals} max {max}"
                );
                let magnitude = 10.0_f64.powf(step.log10().floor());
                let frac = step / magnitude;
                assert!(
                    [1.0, 2.0, 2.5, 5.0, 10.0]
                        .iter()
                        .any(|nice| (frac - nice).abs() < 1e-6),
                    "{step}"
                );
            }
        }
    }

    #[test]
    fn zero_is_a_tick_when_the_data_crosses_or_is_flat() {
        let crossed = chart_axis(-3.0, 10.0, 6, true);
        assert!(crossed.ticks.contains(&0.0));
        assert!(crossed.min <= -3.0);
        assert!(crossed.max >= 10.0);
        assert_eq!(crossed.labels.len(), crossed.ticks.len());

        let flat = chart_axis(0.0, 0.0, 5, true);
        assert!(flat.step.is_finite() && flat.step > 0.0);
        assert!(flat.ticks.len() >= 2);
        assert!(flat.labels.iter().any(|label| label == "0"));
    }

    #[test]
    fn a_negative_span_still_covers_the_data() {
        let axis = chart_axis(-80.0, -10.0, 6, false);
        assert!(axis.step > 0.0);
        assert!(axis.min <= -80.0);
        assert!(axis.max >= -10.0);
        assert!(axis
            .labels
            .iter()
            .all(|label| label.starts_with('-') || label == "0"));
    }
}
