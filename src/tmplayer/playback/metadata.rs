use crate::tmplayer::app::state::{LyricLine, LyricWord};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

const MAX_LOCAL_COVER_BYTES: u64 = 8 * 1024 * 1024;

pub fn read_cover_from_folder(dir: &Path) -> Option<(Vec<u8>, u64)> {
    // Common filenames used by many players.
    // Keep this list small and predictable.
    let candidates = [
        "cover", "folder", "front", "album", "artwork", "Cover", "Folder", "Front",
    ];
    let exts = ["jpg", "jpeg", "png"];

    for base in candidates {
        for ext in exts {
            let p = dir.join(format!("{base}.{ext}"));
            if let Some(cover) = read_cover_file(&p) {
                return Some(cover);
            }
        }
    }
    None
}

fn read_cover_file(path: &Path) -> Option<(Vec<u8>, u64)> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let len = metadata.len();
    if len == 0 || len > MAX_LOCAL_COVER_BYTES {
        return None;
    }

    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }

    let hash = hash_bytes(&bytes);
    Some((bytes, hash))
}

pub fn parse_lrc(content: &str) -> Option<Vec<LyricLine>> {
    let mut out: Vec<LyricLine> = Vec::new();

    for raw in content.lines() {
        let mut s = raw.trim();
        if s.is_empty() {
            continue;
        }

        // Collect leading [..] tags; keep all time tags, ignore metadata tags like [ti:]
        let mut times: Vec<u64> = Vec::new();
        while let Some(rest) = s.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                break;
            };
            let tag = &rest[..end];
            if let Some(ms) = parse_lrc_time_tag(tag) {
                times.push(ms);
            }
            s = &rest[end + 1..];
        }

        if times.is_empty() {
            continue;
        }

        let text = s.trim().to_string();
        for t in times {
            out.push(LyricLine {
                start_ms: t,
                text: text.clone(),
                translation: None,
                words: Vec::new(),
            });
        }
    }

    if out.is_empty() {
        return None;
    }
    out.sort_by_key(|l| l.start_ms);
    Some(out)
}

pub fn parse_plain_lyrics(content: &str) -> Option<Vec<LyricLine>> {
    let mut non_empty = content.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = non_empty.next()?.to_string();
    let second = non_empty.next().map(|s| s.to_string());

    let mut out = Vec::new();
    out.push(LyricLine {
        start_ms: 0,
        text: first,
        translation: None,
        words: Vec::new(),
    });
    if let Some(s2) = second {
        out.push(LyricLine {
            start_ms: u64::MAX,
            text: s2,
            translation: None,
            words: Vec::new(),
        });
    }
    Some(out)
}

/// Parse NetEase YRC data. Each word is encoded as `[start,duration]text`.
/// The parser is intentionally lenient because responses contain occasional
/// metadata lines and may use either millisecond or centisecond timestamps.
pub fn parse_yrc(content: &str) -> Option<Vec<LyricLine>> {
    let mut out = Vec::new();
    for raw in content.lines() {
        let line_start = raw
            .strip_prefix('[')
            .and_then(|rest| rest.find(']').map(|end| &rest[..end]))
            .and_then(|tag| {
                tag.split_once(',')
                    .and_then(|(start, _)| start.parse::<u64>().ok())
            })
            .unwrap_or(0);
        let mut words = Vec::new();
        let mut cursor = 0usize;
        while let Some(open_rel) = raw[cursor..].find('(') {
            let open = cursor + open_rel;
            let Some(close_rel) = raw[open + 1..].find(')') else {
                break;
            };
            let close = open + 1 + close_rel;
            let mut fields = raw[open + 1..close].split(',');
            let Some(offset_s) = fields.next() else {
                cursor = close + 1;
                continue;
            };
            let Some(duration_s) = fields.next() else {
                cursor = close + 1;
                continue;
            };
            let Ok(offset_ms) = offset_s.trim().parse::<u64>() else {
                cursor = close + 1;
                continue;
            };
            let Ok(duration_ms) = duration_s.trim().parse::<u64>() else {
                cursor = close + 1;
                continue;
            };
            let text_start = close + 1;
            let text_end = raw[text_start..]
                .find('(')
                .map(|offset| text_start + offset)
                .unwrap_or(raw.len());
            let text = raw[text_start..text_end].to_string();
            if !text.is_empty() {
                words.push(LyricWord {
                    start_ms: line_start.saturating_add(offset_ms),
                    end_ms: line_start
                        .saturating_add(offset_ms)
                        .saturating_add(duration_ms),
                    text,
                });
            }
            cursor = text_end;
        }

        // Some servers return the simplified `[start,duration]word` form.
        if words.is_empty() {
            let mut fallback_cursor = 0usize;
            while let Some(open_rel) = raw[fallback_cursor..].find('[') {
                let open = fallback_cursor + open_rel;
                let Some(close_rel) = raw[open + 1..].find(']') else {
                    break;
                };
                let close = open + 1 + close_rel;
                let Some((start_s, duration_s)) = raw[open + 1..close].split_once(',') else {
                    fallback_cursor = close + 1;
                    continue;
                };
                let (Ok(start_ms), Ok(duration_ms)) = (
                    start_s.trim().parse::<u64>(),
                    duration_s.trim().parse::<u64>(),
                ) else {
                    fallback_cursor = close + 1;
                    continue;
                };
                let text_start = close + 1;
                let text_end = raw[text_start..]
                    .find('[')
                    .map(|offset| text_start + offset)
                    .unwrap_or(raw.len());
                let text = raw[text_start..text_end].to_string();
                if !text.is_empty() {
                    words.push(LyricWord {
                        start_ms,
                        end_ms: start_ms.saturating_add(duration_ms),
                        text,
                    });
                }
                fallback_cursor = text_end;
            }
        }

        if let Some(first) = words.first() {
            let text = words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<String>();
            out.push(LyricLine {
                start_ms: first.start_ms,
                text,
                translation: None,
                words,
            });
        }
    }
    if out.is_empty() {
        None
    } else {
        out.sort_by_key(|line| line.start_ms);
        Some(out)
    }
}

/// Attach translated lines and word timing to the regular LRC lines.
pub fn enrich_lyrics(
    mut lines: Vec<LyricLine>,
    translated: Option<&str>,
    yrc: Option<&str>,
) -> Vec<LyricLine> {
    if let Some(translated) = translated
        .and_then(parse_lrc)
        .or_else(|| translated.and_then(parse_plain_lyrics))
    {
        for (index, line) in lines.iter_mut().enumerate() {
            let match_line = translated
                .iter()
                .find(|candidate| candidate.start_ms == line.start_ms)
                .or_else(|| translated.get(index));
            if let Some(match_line) = match_line {
                if !match_line.text.trim().is_empty() && match_line.text != line.text {
                    line.translation = Some(match_line.text.clone());
                }
            }
        }
    }

    if let Some(word_lines) = yrc.and_then(parse_yrc) {
        for (index, line) in lines.iter_mut().enumerate() {
            let word_line = word_lines
                .iter()
                .min_by_key(|candidate| candidate.start_ms.abs_diff(line.start_ms))
                .or_else(|| word_lines.get(index));
            if let Some(word_line) = word_line {
                if word_line.start_ms.abs_diff(line.start_ms) <= 5_000 {
                    line.words = word_line.words.clone();
                }
            }
        }
    }
    lines
}

fn parse_lrc_time_tag(tag: &str) -> Option<u64> {
    // Supports mm:ss, mm:ss.xx, mm:ss.xxx
    // Rejects metadata tags like "ti:xxx" by requiring numeric mm and ss.
    let (mm_s, rest) = tag.split_once(':')?;
    let mm: u64 = mm_s.trim().parse().ok()?;

    let rest = rest.trim();
    let (ss_s, frac_s) = if let Some((a, b)) = rest.split_once('.') {
        (a, Some(b))
    } else {
        (rest, None)
    };
    let ss: u64 = ss_s.trim().parse().ok()?;
    if ss >= 60 {
        // be lenient but avoid obvious non-timestamps
        return None;
    }

    let mut ms: u64 = 0;
    if let Some(frac) = frac_s {
        let frac = frac.trim();
        let digits: String = frac
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .take(3)
            .collect();
        if digits.is_empty() {
            ms = 0;
        } else if digits.len() == 1 {
            ms = digits.parse::<u64>().ok()? * 100;
        } else if digits.len() == 2 {
            ms = digits.parse::<u64>().ok()? * 10;
        } else {
            ms = digits.parse::<u64>().ok()?;
        }
    }

    Some(mm * 60_000 + ss * 1_000 + ms)
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::{enrich_lyrics, parse_lrc, parse_yrc};

    #[test]
    fn parses_yrc_word_ranges() {
        let lines = parse_yrc("[100,300]Hel[400,200]lo").expect("YRC line");
        assert_eq!(lines[0].start_ms, 100);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].words[0].end_ms, 400);
        assert_eq!(lines[0].words[1].start_ms, 400);

        let lines = parse_yrc("[1000,1000](0,400,0)Hel(400,600,0)lo").expect("YRC line");
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].words[0].start_ms, 1000);
        assert_eq!(lines[0].words[1].end_ms, 2000);
    }

    #[test]
    fn attaches_translation_and_word_timing() {
        let base = parse_lrc("[00:01.00]Hello").expect("LRC line");
        let enriched = enrich_lyrics(
            base,
            Some("[00:01.00]你好"),
            Some("[1000,500]Hel[1500,500]lo"),
        );
        assert_eq!(enriched[0].translation.as_deref(), Some("你好"));
        assert_eq!(enriched[0].words.len(), 2);
    }
}
