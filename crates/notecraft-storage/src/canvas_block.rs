//! The `notecraft-canvas` block of a `.notecraft` page (docs/markdown.md sections 3.4 and 9).
//!
//! The block is the last fenced code block with the info string `notecraft-canvas`. This module
//! finds it, reads its ink strokes, and writes strokes back with a minimal diff: only the value
//! of the `"ink"` key changes, so every other key (containers, shapes, images, files, z, and
//! keys this version does not know) and all spacing stay byte for byte. A block is created
//! only when there are strokes to store.
//!
//! Reading is lenient and bounded. Unknown fields are skipped, a stroke or point that is not
//! usable is dropped, bad numbers fall back (pressure to 1, width to 1, color to the default
//! pen color), coordinates are clamped to +-1,000,000, a stroke over 20,000 points is simplified,
//! and a page keeps at most 100,000 strokes. Anything worse gives a [`CanvasProblem`], never a
//! panic. The block is never rewritten when it is not understood, so nothing is lost.

use std::collections::HashSet;
use std::fmt;
use std::ops::Range;

use notecraft_core::ink::{MAX_STROKE_POINTS, MAX_STROKE_WIDTH, MIN_STROKE_WIDTH};
use notecraft_core::{simplify, InkColor, InkPoint, InkTool, Stroke};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

/// The info string of the canvas block.
pub const CANVAS_INFO: &str = "notecraft-canvas";
/// The `format` number this version reads and writes.
pub const FORMAT_VERSION: i64 = 1;
/// Largest canvas block read (48 MiB).
pub const MAX_CANVAS_BYTES: usize = 48 * 1024 * 1024;
/// Most strokes a page keeps (100,000).
pub const MAX_STROKES: usize = 100_000;
/// Coordinates read from a file are clamped to this distance from the origin.
pub const MAX_COORD: f32 = 1_000_000.0;
/// Longest stroke id kept.
const MAX_ID_CHARS: usize = 64;

/// Where the canvas block sits in a page's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasBlock {
    /// From the opening fence to the end of the closing fence (before its line ending). An
    /// unclosed block runs to the end of the text.
    pub block: Range<usize>,
    /// The JSON between the fences.
    pub json: Range<usize>,
    /// The closing fence was present.
    pub closed: bool,
}

/// Why a canvas block was not (fully) used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasProblem {
    /// The block is not valid JSON (or not an object), for example a truncated file.
    Unparseable,
    /// The block is over 48 MiB.
    TooLarge,
    /// The block's `format` is newer than this version understands.
    NewerFormat(i64),
    /// The page holds more strokes than the limit; the first 100,000 were read. The number is
    /// how many strokes the file holds.
    TooManyStrokes(usize),
}

impl CanvasProblem {
    /// A plain-text line for the user.
    pub fn message(self) -> String {
        match self {
            Self::Unparseable => "The ink and layout data in this page could not be read. The page is shown as text and left as it is on disk.".to_owned(),
            Self::TooLarge => "The ink and layout data in this page is too large to open. The page is shown as text and left as it is on disk.".to_owned(),
            Self::NewerFormat(n) => format!("This page was saved by a newer version of NoteCraft (format {n}). It is shown read-only."),
            Self::TooManyStrokes(n) => format!("This page has {n} ink strokes; only the first {MAX_STROKES} are drawn. The rest are kept in the file, and the page is read-only."),
        }
    }

    /// The page must not be saved over.
    pub fn blocks_saving(self) -> bool {
        true
    }
}

/// What reading a page's canvas block gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasRead {
    pub block: Option<CanvasBlock>,
    /// The strokes read, in paint order, each forced into range.
    pub strokes: Vec<Stroke>,
    pub problem: Option<CanvasProblem>,
    /// Strokes dropped because they had no usable points.
    pub skipped: usize,
    /// Strokes simplified because they had too many points.
    pub simplified: usize,
}

/// Why [`write_ink`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteInkError {
    /// The existing block is not understood; it is left alone.
    Blocked(CanvasProblem),
}

impl fmt::Display for WriteInkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked(problem) => f.write_str(&problem.message()),
        }
    }
}

impl std::error::Error for WriteInkError {}

/// The fence a line opens or closes: its character and length.
fn fence_of(line: &str) -> Option<(u8, usize)> {
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = line.get(indent..)?;
    let ch = *rest.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let run = rest.bytes().take_while(|b| *b == ch).count();
    if run < 3 || (ch == b'`' && rest.get(run..)?.contains('`')) {
        return None;
    }
    Some((ch, run))
}

fn closes(open: (u8, usize), line: &str) -> bool {
    match fence_of(line) {
        Some((ch, len)) => ch == open.0 && len >= open.1 && line.trim().bytes().all(|b| b == ch),
        None => false,
    }
}

/// Find the canvas block: the last top-level fenced block whose info string is
/// `notecraft-canvas`. Fences nested in another code block do not count.
pub fn find_canvas_block(text: &str) -> Option<CanvasBlock> {
    struct Open {
        fence: (u8, usize),
        start: usize,
        json_start: usize,
        canvas: bool,
    }
    let mut found = None;
    let mut open: Option<Open> = None;
    let mut offset = 0usize;
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        match &open {
            Some(o) => {
                if closes(o.fence, line) {
                    if o.canvas {
                        found = Some(CanvasBlock {
                            block: o.start..start + line.len(),
                            json: o.json_start..start,
                            closed: true,
                        });
                    }
                    open = None;
                }
            }
            None => {
                if let Some(fence) = fence_of(line) {
                    let info = line
                        .trim_start_matches(' ')
                        .trim_start_matches(fence.0 as char)
                        .trim();
                    open = Some(Open {
                        fence,
                        start,
                        json_start: offset,
                        canvas: info.split_whitespace().next() == Some(CANVAS_INFO),
                    });
                }
            }
        }
    }
    if let Some(o) = open {
        if o.canvas {
            found = Some(CanvasBlock {
                block: o.start..text.len(),
                json: o.json_start.min(text.len())..text.len(),
                closed: false,
            });
        }
    }
    found
}

// ---------------------------------------------------------------------------------------
// Lenient streaming deserialization (no intermediate JSON tree, so memory stays near the
// size of the strokes themselves).

/// Implement the `visit_*` methods that make a visitor accept any scalar as `$value`.
macro_rules! accept_scalars {
    ($value:expr) => {
        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
            Ok($value)
        }
        fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
            Ok($value)
        }
    };
}

/// Skip the rest of a sequence.
fn drain_seq<'de, A: SeqAccess<'de>>(mut seq: A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

/// Skip the rest of a map.
fn drain_map<'de, A: MapAccess<'de>>(mut map: A) -> Result<(), A::Error> {
    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
    Ok(())
}

/// Any JSON value reduced to what the reader can use.
enum Scalar {
    Str(String),
    Num(f64),
    Other,
}

impl Scalar {
    fn num(&self) -> Option<f64> {
        match self {
            Self::Num(n) if n.is_finite() => Some(*n),
            _ => None,
        }
    }
}

struct ScalarVisitor;

impl<'de> Visitor<'de> for ScalarVisitor {
    type Value = Scalar;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any value")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Scalar, E> {
        Ok(Scalar::Other)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Scalar, E> {
        Ok(Scalar::Other)
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Scalar, E> {
        Ok(Scalar::Other)
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Scalar, E> {
        Ok(Scalar::Str(v.chars().take(256).collect()))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Scalar, E> {
        Ok(Scalar::Num(v as f64))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Scalar, E> {
        Ok(Scalar::Num(v as f64))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Scalar, E> {
        Ok(Scalar::Num(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Scalar, A::Error> {
        drain_seq(seq).map(|()| Scalar::Other)
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Scalar, A::Error> {
        drain_map(map).map(|()| Scalar::Other)
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ScalarVisitor)
    }
}

/// One point: `[x, y, pressure]`; `None` when x or y is not a number.
struct RawPoint(Option<InkPoint>);

struct PointVisitor;

impl<'de> Visitor<'de> for PointVisitor {
    type Value = RawPoint;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a point")
    }
    accept_scalars!(RawPoint(None));
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<RawPoint, A::Error> {
        drain_map(map).map(|()| RawPoint(None))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawPoint, A::Error> {
        let mut v: [Option<f64>; 3] = [None; 3];
        let mut i = 0usize;
        while let Some(s) = seq.next_element::<Scalar>()? {
            if let Some(slot) = v.get_mut(i) {
                *slot = s.num();
            }
            i += 1;
        }
        let coord = |n: f64| (n as f32).clamp(-MAX_COORD, MAX_COORD);
        let [x, y, p] = v;
        Ok(RawPoint(match (x, y) {
            (Some(x), Some(y)) => Some(InkPoint::new(
                coord(x),
                coord(y),
                p.map_or(1.0, |p| (p as f32).clamp(0.0, 1.0)),
            )),
            _ => None,
        }))
    }
}

impl<'de> Deserialize<'de> for RawPoint {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(PointVisitor)
    }
}

/// A list of points; anything that is not a list is empty.
#[derive(Default)]
struct Points(Vec<InkPoint>);

struct PointsVisitor;

impl<'de> Visitor<'de> for PointsVisitor {
    type Value = Points;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of points")
    }
    accept_scalars!(Points::default());
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Points, A::Error> {
        drain_map(map).map(|()| Points::default())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Points, A::Error> {
        let mut points = Vec::new();
        while let Some(RawPoint(p)) = seq.next_element::<RawPoint>()? {
            points.extend(p);
        }
        Ok(Points(points))
    }
}

impl<'de> Deserialize<'de> for Points {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(PointsVisitor)
    }
}

#[derive(Default)]
struct RawStroke {
    id: Option<Scalar>,
    tool: Option<Scalar>,
    color: Option<Scalar>,
    width: Option<Scalar>,
    points: Points,
}

struct StrokeVisitor;

impl<'de> Visitor<'de> for StrokeVisitor {
    type Value = RawStroke;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a stroke")
    }
    accept_scalars!(RawStroke::default());
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<RawStroke, A::Error> {
        drain_seq(seq).map(|()| RawStroke::default())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawStroke, A::Error> {
        let mut s = RawStroke::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "id" => s.id = Some(map.next_value()?),
                "tool" => s.tool = Some(map.next_value()?),
                "color" => s.color = Some(map.next_value()?),
                "width" => s.width = Some(map.next_value()?),
                "points" => s.points = map.next_value()?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(s)
    }
}

impl<'de> Deserialize<'de> for RawStroke {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(StrokeVisitor)
    }
}

/// The strokes list: the first [`MAX_STROKES`] elements are kept, the rest only counted.
#[derive(Default)]
struct StrokeList {
    strokes: Vec<RawStroke>,
    total: usize,
}

struct StrokeListVisitor;

impl<'de> Visitor<'de> for StrokeListVisitor {
    type Value = StrokeList;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of strokes")
    }
    accept_scalars!(StrokeList::default());
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<StrokeList, A::Error> {
        drain_map(map).map(|()| StrokeList::default())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<StrokeList, A::Error> {
        let mut list = StrokeList::default();
        loop {
            if list.strokes.len() >= MAX_STROKES {
                if seq.next_element::<IgnoredAny>()?.is_none() {
                    break;
                }
            } else {
                match seq.next_element::<RawStroke>()? {
                    Some(s) => list.strokes.push(s),
                    None => break,
                }
            }
            list.total += 1;
        }
        Ok(list)
    }
}

impl<'de> Deserialize<'de> for StrokeList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(StrokeListVisitor)
    }
}

#[derive(Default)]
struct RawCanvas {
    format: Option<Scalar>,
    ink: Option<StrokeList>,
}

struct CanvasVisitor;

impl<'de> Visitor<'de> for CanvasVisitor {
    type Value = RawCanvas;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a canvas object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawCanvas, A::Error> {
        let mut c = RawCanvas::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "format" => c.format = Some(map.next_value()?),
                "ink" => c.ink = Some(map.next_value()?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(c)
    }
}

impl<'de> Deserialize<'de> for RawCanvas {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(CanvasVisitor)
    }
}

/// Turn a raw stroke into a usable one, or `None` when it has no points.
fn finish_stroke(
    raw: RawStroke,
    index: usize,
    seen: &mut HashSet<String>,
    simplified: &mut usize,
) -> Option<Stroke> {
    if raw.points.0.is_empty() {
        return None;
    }
    let mut id = match &raw.id {
        Some(Scalar::Str(s)) if !s.trim().is_empty() => s.chars().take(MAX_ID_CHARS).collect(),
        Some(Scalar::Num(n)) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 => {
            format!("{}", *n as i64)
        }
        _ => format!("s{}", index + 1),
    };
    let mut n = index + 1;
    while !seen.insert(id.clone()) {
        n += 1;
        id = format!("s{n}");
    }
    let tool = match &raw.tool {
        Some(Scalar::Str(s)) if s.eq_ignore_ascii_case("highlighter") => InkTool::Highlighter,
        _ => InkTool::Pen,
    };
    let color = match &raw.color {
        Some(Scalar::Str(s)) => InkColor::parse(s).unwrap_or_default(),
        _ => InkColor::default(),
    };
    let width = raw.width.as_ref().and_then(Scalar::num).map_or(1.0, |w| {
        (w as f32).clamp(MIN_STROKE_WIDTH, MAX_STROKE_WIDTH)
    });
    let mut points = raw.points.0;
    if points.len() > MAX_STROKE_POINTS {
        points = simplify(&points, MAX_STROKE_POINTS);
        *simplified += 1;
    }
    Some(
        Stroke {
            id,
            tool,
            color,
            width,
            points,
        }
        .bounded(),
    )
}

/// Deepest JSON nesting read. serde_json may be built without its own depth limit (another
/// crate can turn it off through feature unification), so the reader checks first.
const MAX_JSON_DEPTH: usize = 32;

/// True if `json` nests arrays or objects deeper than [`MAX_JSON_DEPTH`] (strings skipped).
fn too_deep(json: &str) -> bool {
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for b in json.bytes() {
        if in_str {
            match (esc, b) {
                (true, _) => esc = false,
                (false, b'\\') => esc = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// Read the canvas block of `text`. Never fails; see [`CanvasProblem`].
pub fn read_canvas(text: &str) -> CanvasRead {
    let mut read = CanvasRead {
        block: find_canvas_block(text),
        ..CanvasRead::default()
    };
    let Some(block) = &read.block else {
        return read;
    };
    let json = text.get(block.json.clone()).unwrap_or("");
    if json.len() > MAX_CANVAS_BYTES {
        read.problem = Some(CanvasProblem::TooLarge);
        return read;
    }
    if too_deep(json) {
        read.problem = Some(CanvasProblem::Unparseable);
        return read;
    }
    let Ok(raw) = serde_json::from_str::<RawCanvas>(json) else {
        read.problem = Some(CanvasProblem::Unparseable);
        return read;
    };
    if let Some(Scalar::Num(n)) = raw.format {
        if n.is_finite() && n > FORMAT_VERSION as f64 {
            read.problem = Some(CanvasProblem::NewerFormat(n as i64));
            return read;
        }
    }
    let Some(list) = raw.ink else {
        return read;
    };
    let mut seen = HashSet::new();
    for (i, stroke) in list.strokes.into_iter().enumerate() {
        match finish_stroke(stroke, i, &mut seen, &mut read.simplified) {
            Some(s) => read.strokes.push(s),
            None => read.skipped += 1,
        }
    }
    if list.total > MAX_STROKES {
        read.problem = Some(CanvasProblem::TooManyStrokes(list.total));
    }
    read
}

// ---------------------------------------------------------------------------------------
// Writing.

fn round2(v: f32) -> f32 {
    let r = (v * 100.0).round() / 100.0;
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

/// A stroke as the file stores it: bounded, with every number rounded to 2 decimal places.
pub fn normalized(stroke: &Stroke) -> Stroke {
    let mut s = stroke.clone().bounded();
    s.width = round2(s.width).max(MIN_STROKE_WIDTH);
    s.points = s.points.iter().map(|p| p.rounded()).collect();
    s
}

fn num(v: f32) -> String {
    let r = round2(v);
    if r == 0.0 {
        "0".to_owned()
    } else {
        format!("{r}")
    }
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// The `[ ... ]` text of the ink list, one stroke per line, with `indent` spaces before each
/// stroke and `indent - 2` before the closing bracket.
fn ink_array(strokes: &[Stroke], eol: &str, indent: usize) -> String {
    if strokes.is_empty() {
        return "[]".to_owned();
    }
    let pad = " ".repeat(indent);
    let mut out = String::from("[");
    out.push_str(eol);
    for (i, stroke) in strokes.iter().enumerate() {
        let s = normalized(stroke);
        out.push_str(&pad);
        out.push_str("{ \"id\": ");
        out.push_str(&json_string(&s.id));
        out.push_str(", \"tool\": ");
        out.push_str(match s.tool {
            InkTool::Pen => "\"pen\"",
            InkTool::Highlighter => "\"highlighter\"",
        });
        out.push_str(", \"color\": \"");
        out.push_str(&s.color.to_hex());
        out.push_str("\", \"width\": ");
        out.push_str(&num(s.width));
        out.push_str(", \"points\": [");
        for (j, p) in s.points.iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            out.push('[');
            out.push_str(&num(p.x));
            out.push_str(", ");
            out.push_str(&num(p.y));
            out.push_str(", ");
            out.push_str(&num(p.pressure));
            out.push(']');
        }
        out.push_str("] }");
        if i + 1 < strokes.len() {
            out.push(',');
        }
        out.push_str(eol);
    }
    out.push_str(&" ".repeat(indent.saturating_sub(2)));
    out.push(']');
    out
}

/// The dominant line ending of `text`: CRLF only when more lines end in CRLF than bare LF.
fn eol_of(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf {
        "\r\n"
    } else {
        "\n"
    }
}

/// The top-level members of a JSON object that is already known to be valid.
struct ObjectScan {
    open: usize,
    close: usize,
    /// Raw key text (without quotes) and the byte range of the value.
    members: Vec<(Range<usize>, Range<usize>)>,
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// The index just past the string that starts at `i` (a `"`).
fn skip_string(b: &[u8], mut i: usize) -> Option<usize> {
    i += 1;
    loop {
        match *b.get(i)? {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
}

/// The index just past the value that starts at `i`.
fn skip_value(b: &[u8], i: usize) -> Option<usize> {
    match *b.get(i)? {
        b'"' => skip_string(b, i),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut j = i;
            loop {
                match *b.get(j)? {
                    b'"' => {
                        j = skip_string(b, j)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        _ => {
            let mut j = i;
            while let Some(c) = b.get(j) {
                if matches!(c, b',' | b'}' | b']') || c.is_ascii_whitespace() {
                    break;
                }
                j += 1;
            }
            Some(j)
        }
    }
}

fn scan_object(json: &str) -> Option<ObjectScan> {
    let b = json.as_bytes();
    let open = skip_ws(b, 0);
    if *b.get(open)? != b'{' {
        return None;
    }
    let mut members = Vec::new();
    let mut i = skip_ws(b, open + 1);
    loop {
        match *b.get(i)? {
            b'}' => {
                return Some(ObjectScan {
                    open,
                    close: i,
                    members,
                })
            }
            b'"' => {
                let end = skip_string(b, i)?;
                let key = i + 1..end - 1;
                i = skip_ws(b, end);
                if *b.get(i)? != b':' {
                    return None;
                }
                i = skip_ws(b, i + 1);
                let value_end = skip_value(b, i)?;
                members.push((key, i..value_end));
                i = skip_ws(b, value_end);
                if *b.get(i)? == b',' {
                    i = skip_ws(b, i + 1);
                }
            }
            _ => return None,
        }
    }
}

/// `strokes` normalized, empty strokes dropped.
fn usable(strokes: &[Stroke]) -> Vec<Stroke> {
    strokes
        .iter()
        .map(normalized)
        .filter(|s| !s.points.is_empty())
        .collect()
}

/// The page `text` with its canvas block holding `strokes`, changing only the `"ink"` value.
///
/// - If the block already holds exactly these strokes (after rounding), the text comes back
///   unchanged, byte for byte.
/// - If there is no block and no strokes, the text comes back unchanged.
/// - If there is no block, one is appended at the end.
/// - If the block is not understood (see [`CanvasProblem`]) the write is refused.
pub fn write_ink(text: &str, strokes: &[Stroke]) -> Result<String, WriteInkError> {
    let current = read_canvas(text);
    if let Some(problem) = current.problem {
        return Err(WriteInkError::Blocked(problem));
    }
    let new = usable(strokes);
    let old: Vec<Stroke> = current.strokes.iter().map(normalized).collect();
    if new == old && (current.block.is_some() || new.is_empty()) {
        return Ok(text.to_owned());
    }
    let eol = eol_of(text);
    let Some(block) = current.block else {
        let out = append_block(text, &new, eol);
        // The page may end inside an unclosed code fence, which would swallow the new block.
        // Check that the block really is found; if not, refuse rather than lose the ink.
        return match find_canvas_block(&out) {
            Some(found) if found.closed && found.block.start >= text.len() => Ok(out),
            _ => Err(WriteInkError::Blocked(CanvasProblem::Unparseable)),
        };
    };
    let json = text.get(block.json.clone()).unwrap_or("");
    let base = block.json.start;
    let Some(scan) = scan_object(json) else {
        return Err(WriteInkError::Blocked(CanvasProblem::Unparseable));
    };
    let array = ink_array(&new, eol, 4);
    let ink = scan
        .members
        .iter()
        .rev()
        .find(|(key, _)| json.get(key.clone()) == Some("ink"));
    let (range, replacement) = if let Some((_, value)) = ink {
        (base + value.start..base + value.end, array)
    } else if let Some((_, last)) = scan.members.last() {
        let at = base + last.end;
        (at..at, format!(",{eol}  \"ink\": {array}"))
    } else {
        (
            base + scan.open..base + scan.close + 1,
            format!("{{{eol}  \"ink\": {array}{eol}}}"),
        )
    };
    let mut out = String::with_capacity(text.len() + replacement.len());
    out.push_str(text.get(..range.start).unwrap_or(""));
    out.push_str(&replacement);
    out.push_str(text.get(range.end..).unwrap_or(""));
    Ok(out)
}

fn append_block(text: &str, strokes: &[Stroke], eol: &str) -> String {
    let mut out = text.to_owned();
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push_str(eol);
        }
        out.push_str(eol);
    }
    out.push_str("```");
    out.push_str(CANVAS_INFO);
    out.push_str(eol);
    out.push('{');
    out.push_str(eol);
    out.push_str(&format!("  \"format\": {FORMAT_VERSION},{eol}"));
    out.push_str(&format!("  \"units\": \"px96\",{eol}"));
    out.push_str("  \"ink\": ");
    out.push_str(&ink_array(strokes, eol, 4));
    out.push_str(eol);
    out.push('}');
    out.push_str(eol);
    out.push_str("```");
    out.push_str(eol);
    out
}

/// `text` without its canvas block (and the blank line before it), for converting a page back
/// to plain markdown. Text with no block comes back unchanged.
pub fn strip_canvas_block(text: &str) -> String {
    let Some(block) = find_canvas_block(text) else {
        return text.to_owned();
    };
    let head = text.get(..block.block.start).unwrap_or("");
    let mut tail = text.get(block.block.end..).unwrap_or("");
    tail = tail
        .strip_prefix("\r\n")
        .or_else(|| tail.strip_prefix('\n'))
        .unwrap_or(tail);
    let mut out = format!("{head}{tail}");
    if tail.is_empty() {
        // A blank line that only separated the block from the text goes with it.
        for blank in ["\r\n\r\n", "\n\n"] {
            if out.ends_with(blank) {
                out.truncate(out.len() - blank.len() / 2);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "canvas_block_tests.rs"]
mod tests;
