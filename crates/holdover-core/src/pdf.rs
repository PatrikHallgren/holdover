//! Range-card PDF at exact paper size.
//!
//! The page `MediaBox` and the scale bar are in PDF points (1/72 inch).
//! Nothing in this module reads the screen scale.

use crate::field::{scale_bar_mm, CardPage};

/// Millimetres to points, rounded to 0.01 pt. 100 mm is 283.46 pt.
pub fn round_pt(mm: f64) -> f64 {
    (mm * 72.0 / 25.4 * 100.0).round() / 100.0
}

/// Text and table for one card. The page size comes from [`CardPage`].
#[derive(Clone, Debug)]
pub struct CardSheet {
    pub title: String,
    pub subtitle: String,
    pub meta: Vec<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub footer: String,
    pub disclaimer: String,
    pub scale_label: String,
}

/// One target sheet. Lengths on the figure are centimetres in target space.
/// The scale bar is always true paper size. The figure is 1:1 when it fits, otherwise 1:N.
#[derive(Clone, Debug)]
pub struct TargetPrint {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub title: String,
    pub notes: Vec<String>,
    pub rings_cm: Vec<f64>,
    pub paths: Vec<TargetPath>,
    pub cell_cm: Option<f64>,
    pub span_cm: (f64, f64),
}

#[derive(Clone, Debug)]
pub struct TargetPath {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
    pub dashed: bool,
}

#[derive(Clone, Debug)]
pub struct PrintedTarget {
    pub pdf: Vec<u8>,
    pub scale_denominator: u32,
    pub scale_bar_mm: f64,
}

/// Build a PDF page for a target. The black bar is 10 cm, or 5 cm on a narrow page.
pub fn render_target_pdf(sheet: &TargetPrint) -> PrintedTarget {
    let margin = 12.0_f64;
    let bar_mm = if sheet.page_width_mm >= 110.0 {
        100.0
    } else {
        50.0
    };
    let notes: Vec<&str> = sheet.notes.iter().take(8).map(String::as_str).collect();
    let header_mm = 28.0 + notes.len() as f64 * 5.0 + 16.0;
    let usable_w = (sheet.page_width_mm - 2.0 * margin).max(20.0);
    let usable_h = (sheet.page_height_mm - margin - header_mm).max(20.0);
    let span_w_mm = sheet.span_cm.0.max(1.0) * 10.0;
    let span_h_mm = sheet.span_cm.1.max(1.0) * 10.0;
    let denom = ((span_w_mm / usable_w).ceil())
        .max((span_h_mm / usable_h).ceil())
        .max(1.0) as u32;
    let width = round_pt(sheet.page_width_mm);
    let height = round_pt(sheet.page_height_mm);
    let bar = round_pt(bar_mm);
    let margin_pt = round_pt(margin);
    let mut content = Content::new();
    let mut y = height - margin_pt;
    content.text(margin_pt, y - 12.0, 12.0, &sheet.title);
    y -= 16.0;
    let scale_line = if denom == 1 {
        format!("Printed at 1:1. The bar below is {bar_mm:.0} mm on the paper.")
    } else {
        format!("Printed at 1:{denom}. The bar below is {bar_mm:.0} mm on the paper. The figure is smaller than life size.")
    };
    content.text(margin_pt, y - 9.0, 8.0, &scale_line);
    y -= 12.0;
    for note in &notes {
        content.text(margin_pt, y - 8.0, 8.0, note);
        y -= 10.0;
    }
    content.text(margin_pt, y - 9.0, 8.0, &format!("{bar_mm:.0} mm"));
    y -= 12.0;
    content.scale_bar(margin_pt, y - 8.0, bar, 8.0);
    y -= 18.0;
    let origin_x = width / 2.0;
    let origin_y = margin_pt + (y - margin_pt) / 2.0;
    let cm_pt = round_pt(10.0) / f64::from(denom);
    if let Some(cell) = sheet.cell_cm {
        if cell > 0.0 {
            let half_w = sheet.span_cm.0 / 2.0;
            let half_h = sheet.span_cm.1 / 2.0;
            let mut at = -half_w;
            while at <= half_w + 1e-6 {
                content.segment(
                    origin_x + at * cm_pt,
                    origin_y - half_h * cm_pt,
                    origin_x + at * cm_pt,
                    origin_y + half_h * cm_pt,
                    false,
                );
                at += cell;
            }
            let mut at = -half_h;
            while at <= half_h + 1e-6 {
                content.segment(
                    origin_x - half_w * cm_pt,
                    origin_y + at * cm_pt,
                    origin_x + half_w * cm_pt,
                    origin_y + at * cm_pt,
                    false,
                );
                at += cell;
            }
        }
    }
    for path in &sheet.paths {
        content.poly(
            origin_x,
            origin_y,
            cm_pt,
            &path.points,
            path.closed,
            path.dashed,
        );
    }
    for diameter in &sheet.rings_cm {
        if *diameter <= 0.0 {
            continue;
        }
        let radius = round_pt(diameter * 10.0 / f64::from(denom)) / 2.0;
        content.circle(origin_x, origin_y, radius);
    }
    let bytes = content.take();
    PrintedTarget {
        pdf: assemble(width, height, &[bytes]),
        scale_denominator: denom,
        scale_bar_mm: bar_mm,
    }
}

/// Build a PDF whose page box is the card size and whose scale bar is `scale_bar_mm` long.
pub fn render_card_pdf(page: &CardPage, sheet: &CardSheet) -> Vec<u8> {
    let width = round_pt(page.width_mm);
    let height = round_pt(page.height_mm);
    let bar = round_pt(scale_bar_mm(page));
    let margin = if page.width_mm < 110.0 {
        round_pt(4.0)
    } else {
        round_pt(10.0)
    };
    let streams = layout(width, height, margin, bar, &sheet.scale_label, sheet);
    assemble(width, height, &streams)
}

fn layout(
    width: f64,
    height: f64,
    margin: f64,
    bar: f64,
    bar_label: &str,
    sheet: &CardSheet,
) -> Vec<Vec<u8>> {
    let cols = sheet.headers.len().max(1);
    let usable = (width - 2.0 * margin).max(bar);
    let font = if cols > 10 {
        6.0
    } else if cols > 7 {
        7.0
    } else {
        8.0
    };
    let col_w = usable / cols as f64;
    let row_h = font + 4.0;
    let mut pages = Vec::new();
    let mut content = Content::new();
    let mut y = height - margin;

    let mut new_page = |content: &mut Content, y: &mut f64| {
        if !content.is_empty() {
            pages.push(content.take());
        }
        *y = height - margin;
    };

    content.text(margin, y - 12.0, 12.0, &sheet.title);
    y -= 16.0;
    if !sheet.subtitle.is_empty() {
        content.text(margin, y - 10.0, 9.0, &sheet.subtitle);
        y -= 12.0;
    }
    for line in &sheet.meta {
        if y < margin + 80.0 {
            new_page(&mut content, &mut y);
        }
        content.text(margin, y - 9.0, 8.0, line);
        y -= 11.0;
    }
    y -= 4.0;
    let bar_h = 8.0;
    if y < margin + bar_h + 24.0 {
        new_page(&mut content, &mut y);
    }
    content.text(margin, y - 9.0, 8.0, bar_label);
    y -= 12.0;
    content.scale_bar(margin, y - bar_h, bar, bar_h);
    y -= bar_h + 10.0;

    let draw_header = |content: &mut Content, y: &mut f64| {
        for (index, header) in sheet.headers.iter().enumerate() {
            let x = margin + index as f64 * col_w;
            content.text(x, *y - font, font, &clip(header, col_w, font));
        }
        *y -= row_h;
        content.hline(margin, *y + 2.0, usable);
    };
    draw_header(&mut content, &mut y);

    for row in &sheet.rows {
        if y < margin + row_h + 28.0 {
            new_page(&mut content, &mut y);
            draw_header(&mut content, &mut y);
        }
        for (index, cell) in row.iter().enumerate().take(cols) {
            let x = margin + index as f64 * col_w;
            content.text(x, y - font, font, &clip(cell, col_w, font));
        }
        y -= row_h;
    }
    y -= 8.0;
    if y < margin + 24.0 {
        new_page(&mut content, &mut y);
    }
    content.text(margin, y - 8.0, 8.0, &sheet.footer);
    y -= 12.0;
    for line in wrap(&sheet.disclaimer, usable, 7.0) {
        if y < margin + 10.0 {
            new_page(&mut content, &mut y);
        }
        content.text(margin, y - 8.0, 7.0, &line);
        y -= 9.0;
    }
    pages.push(content.take());
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
}

fn clip(text: &str, width: f64, font: f64) -> String {
    let max_chars = ((width / (font * 0.5)).floor() as usize).max(1);
    let mut out = String::new();
    for ch in text.chars() {
        if out.chars().count() + 1 > max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

fn wrap(text: &str, width: f64, font: f64) -> Vec<String> {
    let max_chars = ((width / (font * 0.5)).floor() as usize).max(8);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let extra = if line.is_empty() { 0 } else { 1 };
        if line.chars().count() + extra + word.chars().count() > max_chars && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct Content {
    bytes: Vec<u8>,
}

impl Content {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }

    fn text(&mut self, x: f64, y: f64, size: f64, text: &str) {
        let escaped = pdf_escape(text);
        let line = format!("BT /F1 {size:.2} Tf {x:.2} {y:.2} Td ({escaped}) Tj ET\n");
        self.bytes.extend(line.as_bytes());
    }

    fn hline(&mut self, x: f64, y: f64, width: f64) {
        let line = format!("0.4 w {x:.2} {y:.2} m {w:.2} {y:.2} l S\n", w = x + width);
        self.bytes.extend(line.as_bytes());
    }

    fn segment(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, dashed: bool) {
        let dash = if dashed { "[2 2] 0 d\n" } else { "" };
        let reset = if dashed { "[] 0 d\n" } else { "" };
        let line = format!("{dash}0.6 w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n{reset}");
        self.bytes.extend(line.as_bytes());
    }

    fn poly(
        &mut self,
        origin_x: f64,
        origin_y: f64,
        cm_pt: f64,
        points: &[(f64, f64)],
        closed: bool,
        dashed: bool,
    ) {
        if points.is_empty() {
            return;
        }
        let dash = if dashed { "[3 2] 0 d\n" } else { "" };
        let reset = if dashed { "[] 0 d\n" } else { "" };
        let close = if closed { "h " } else { "" };
        let mut path = format!("{dash}0.8 w ");
        for (index, (x, y)) in points.iter().enumerate() {
            let px = origin_x + x * cm_pt;
            let py = origin_y + y * cm_pt;
            if index == 0 {
                path.push_str(&format!("{px:.2} {py:.2} m "));
            } else {
                path.push_str(&format!("{px:.2} {py:.2} l "));
            }
        }
        path.push_str(close);
        path.push_str("S\n");
        path.push_str(reset);
        self.bytes.extend(path.as_bytes());
    }

    fn circle(&mut self, cx: f64, cy: f64, radius: f64) {
        if radius <= 0.0 {
            return;
        }
        let k = 0.5522847498 * radius;
        let diameter = radius * 2.0;
        let (x0, y0) = (cx, cy + radius);
        let curves = [
            (cx + k, cy + radius, cx + radius, cy + k, cx + radius, cy),
            (cx + radius, cy - k, cx + k, cy - radius, cx, cy - radius),
            (cx - k, cy - radius, cx - radius, cy - k, cx - radius, cy),
            (cx - radius, cy + k, cx - k, cy + radius, cx, cy + radius),
        ];
        let mut path =
            format!("% HOLDOVER_CIRCLE_DIAMETER_PT {diameter:.2}\n0.8 w {x0:.2} {y0:.2} m ");
        for (x1, y1, x2, y2, x3, y3) in curves {
            path.push_str(&format!(
                "{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x3:.2} {y3:.2} c "
            ));
        }
        path.push_str("S\n");
        self.bytes.extend(path.as_bytes());
    }

    /// Filled bar. The path width is the physical length; a fill does not grow it.
    fn scale_bar(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let line = format!(
            "% HOLDOVER_SCALE_BAR {width:.2}\n0 0 0 rg {x:.2} {y:.2} {width:.2} {height:.2} re f\n"
        );
        self.bytes.extend(line.as_bytes());
    }
}

fn pdf_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            c if (c as u32) < 32 => out.push(' '),
            c if (c as u32) < 127 => out.push(c),
            c if (c as u32) <= 255 => {
                out.push('\\');
                out.push_str(&format!("{:03o}", c as u32));
            }
            _ => out.push('?'),
        }
    }
    out
}

fn assemble(width: f64, height: f64, streams: &[Vec<u8>]) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let page_count = streams.len();
    let font_id = 3;
    let first_page = 4;
    let mut kids = String::new();
    for index in 0..page_count {
        let page_id = first_page + index * 2;
        if index > 0 {
            kids.push(' ');
        }
        kids.push_str(&format!("{page_id} 0 R"));
    }
    objects.push(dict("<</Type /Catalog /Pages 2 0 R>>"));
    objects.push(dict(&format!(
        "<</Type /Pages /Kids [{kids}] /Count {page_count}>>"
    )));
    objects.push(dict("<</Type /Font /Subtype /Type1 /BaseFont /Helvetica>>"));
    for stream in streams {
        let content_obj_id = objects.len() + 2;
        objects.push(dict(&format!(
            "<</Type /Page /Parent 2 0 R /MediaBox [0 0 {width:.2} {height:.2}] /Contents {content_obj_id} 0 R /Resources <</Font <</F1 {font_id} 0 R>>>>>>>"
        )));
        objects.push(stream_obj(stream));
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        let header = format!("{} 0 obj\n", index + 1);
        out.extend(header.as_bytes());
        out.extend(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let size = objects.len() + 1;
    out.extend(format!("xref\n0 {size}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

fn dict(body: &str) -> Vec<u8> {
    body.as_bytes().to_vec()
}

fn stream_obj(content: &[u8]) -> Vec<u8> {
    let mut body = content.to_vec();
    if !body.ends_with(b"\n") {
        body.push(b'\n');
    }
    let mut out = format!("<< /Length {} >>\nstream\n", body.len()).into_bytes();
    out.extend(body);
    out.extend_from_slice(b"endstream");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_page;

    fn sample() -> CardSheet {
        CardSheet {
            title: "Example .308".into(),
            subtitle: "Zero 100 m".into(),
            meta: vec!["Truing: Not trued".into()],
            headers: vec!["Range".into(), "Elevation".into(), "Wind".into()],
            rows: vec![vec!["100".into(), "0.00 mrad".into(), "0.10 mrad".into()]],
            footer: "Predicted values; confirm with live fire.".into(),
            disclaimer: "All values are predictions.".into(),
            scale_label: "10 cm".into(),
        }
    }

    fn media_box(pdf: &[u8]) -> (f64, f64) {
        let text = String::from_utf8_lossy(pdf);
        let marker = "/MediaBox [0 0 ";
        let start = text.find(marker).expect("MediaBox") + marker.len();
        let end = text[start..].find(']').unwrap();
        let nums: Vec<f64> = text[start..start + end]
            .split_whitespace()
            .map(|part| part.parse().unwrap())
            .collect();
        (nums[0], nums[1])
    }

    fn scale_bar_pt(pdf: &[u8]) -> f64 {
        let text = String::from_utf8_lossy(pdf);
        let marker = "% HOLDOVER_SCALE_BAR ";
        let start = text.find(marker).expect("scale bar") + marker.len();
        let end = text[start..].find('\n').unwrap();
        let commented: f64 = text[start..start + end].trim().parse().unwrap();
        let rest = &text[start + end..];
        let re = rest.find(" re").expect("bar rectangle");
        let nums: Vec<f64> = rest[..re]
            .split_whitespace()
            .filter_map(|token| token.parse().ok())
            .collect();
        let drawn = nums[nums.len() - 2];
        assert!(
            (drawn - commented).abs() < 1e-6,
            "drawn {drawn} comment {commented}"
        );
        commented
    }

    #[test]
    fn ten_centimetres_is_283_46_points() {
        assert!((round_pt(100.0) - 283.46).abs() < 1e-9);
        assert!((round_pt(50.0) - 141.73).abs() < 1e-9);
    }

    #[test]
    fn letter_and_a4_pages_use_the_physical_box_and_a_10_cm_bar() {
        let letter = render_card_pdf(card_page("letter").unwrap(), &sample());
        assert!(letter.starts_with(b"%PDF-1.4"));
        let (width, height) = media_box(&letter);
        assert!((width - 612.0).abs() < 0.01, "{width}");
        assert!((height - 792.0).abs() < 0.01, "{height}");
        assert!((scale_bar_pt(&letter) - 283.46).abs() < 0.01);

        let mut a4_sheet = sample();
        a4_sheet.scale_label = "10 cm".into();
        let a4 = render_card_pdf(card_page("a4").unwrap(), &a4_sheet);
        let (width, height) = media_box(&a4);
        assert!((width - round_pt(210.0)).abs() < 0.01, "{width}");
        assert!((height - round_pt(297.0)).abs() < 0.01, "{height}");
        assert!((scale_bar_pt(&a4) - 283.46).abs() < 0.01);

        let mut compact_sheet = sample();
        compact_sheet.scale_label = "5 cm".into();
        let compact = render_card_pdf(card_page("compact").unwrap(), &compact_sheet);
        let (width, height) = media_box(&compact);
        assert!((width - round_pt(90.0)).abs() < 0.01, "{width}");
        assert!((height - round_pt(50.0)).abs() < 0.01, "{height}");
        assert!((scale_bar_pt(&compact) - 141.73).abs() < 0.01);
    }

    #[test]
    fn a_ten_centimetre_ring_prints_at_283_46_points_when_it_fits() {
        let sheet = TargetPrint {
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            title: "10 cm ring".into(),
            notes: vec!["Generic. Not an official target.".into()],
            rings_cm: vec![10.0],
            paths: Vec::new(),
            cell_cm: None,
            span_cm: (10.0, 10.0),
        };
        let printed = render_target_pdf(&sheet);
        assert_eq!(printed.scale_denominator, 1);
        assert!((printed.scale_bar_mm - 100.0).abs() < 1e-9);
        assert!((scale_bar_pt(&printed.pdf) - 283.46).abs() < 0.01);
        let text = String::from_utf8_lossy(&printed.pdf);
        let marker = "% HOLDOVER_CIRCLE_DIAMETER_PT ";
        let start = text.find(marker).expect("circle") + marker.len();
        let end = text[start..].find('\n').unwrap();
        let diameter: f64 = text[start..start + end].trim().parse().unwrap();
        assert!(
            (diameter - 283.46).abs() < 0.02,
            "10 cm ring drew {diameter} pt"
        );
    }
}
