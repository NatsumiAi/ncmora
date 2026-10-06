use crate::tmplayer::app::state::LyricLine;

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

/// Parse the newline-delimited JSON lyric format returned by NetEase's
/// `lyric/new` endpoint. Each entry contains an absolute timestamp and text
/// fragments in `c[].tx`.
pub fn parse_netease_json_lyrics(content: &str) -> Option<Vec<LyricLine>> {
    fn parse_entry(value: &serde_json::Value) -> Option<LyricLine> {
        let start_ms = value.get("t").and_then(|value| {
            value.as_u64().or_else(|| {
                value
                    .as_i64()
                    .filter(|timestamp| *timestamp >= 0)
                    .map(|timestamp| timestamp as u64)
            })
        })?;
        let text = value
            .get("c")?
            .as_array()?
            .iter()
            .filter_map(|fragment| fragment.get("tx").and_then(serde_json::Value::as_str))
            .collect::<String>();
        if text.trim().is_empty() {
            return None;
        }
        Some(LyricLine {
            start_ms,
            text,
            translation: None,
            words: Vec::new(),
        })
    }

    let mut out: Vec<LyricLine> = match serde_json::from_str::<serde_json::Value>(content) {
        Ok(serde_json::Value::Array(entries)) => entries.iter().filter_map(parse_entry).collect(),
        Ok(value @ serde_json::Value::Object(_)) => parse_entry(&value).into_iter().collect(),
        _ => content
            .lines()
            .filter_map(|line| serde_json::from_str(line.trim()).ok())
            .filter_map(|value| parse_entry(&value))
            .collect(),
    };
    if out.is_empty() {
        return None;
    }
    out.sort_by_key(|line| line.start_ms);
    Some(out)
}

pub fn parse_lyrics(content: &str) -> Option<Vec<LyricLine>> {
    parse_lrc(content)
        .or_else(|| parse_netease_json_lyrics(content))
        .or_else(|| parse_plain_lyrics(content))
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

#[cfg(test)]
mod tests {
    use super::{parse_lyrics, parse_netease_json_lyrics};

    #[test]
    fn parses_netease_json_line_lyrics() {
        let content = concat!(
            r#"{"t":0,"c":[{"tx":"作词："},{"tx":"柿崎ユウタ"}]}"#,
            "\n",
            r#"{"t":1000,"c":[{"tx":"作曲："},{"tx":"柿崎ユウタ"}]}"#,
        );
        let lines = parse_netease_json_lyrics(content).expect("JSON lyrics");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 0);
        assert_eq!(lines[0].text, "作词：柿崎ユウタ");
        assert_eq!(lines[1].start_ms, 1000);
        assert_eq!(lines[1].text, "作曲：柿崎ユウタ");
    }

    #[test]
    fn lyric_parser_preserves_plain_text_fallback() {
        let lines = parse_lyrics("A plain lyric line").expect("plain lyrics");
        assert_eq!(lines[0].text, "A plain lyric line");
    }
}
