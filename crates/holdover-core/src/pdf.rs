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
}
