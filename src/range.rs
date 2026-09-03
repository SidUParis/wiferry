use axum::http::{HeaderMap, StatusCode, header};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn len(self) -> u64 {
        self.end - self.start + 1
    }
}

pub fn parse_range(headers: &HeaderMap, size: u64) -> Result<Option<ByteRange>, StatusCode> {
    let Some(raw) = headers.get(header::RANGE) else {
        return Ok(None);
    };
    let value = raw
        .to_str()
        .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
    let spec = value
        .strip_prefix("bytes=")
        .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
    if spec.contains(',') || size == 0 {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    let (first, last) = spec
        .split_once('-')
        .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
    let (start, end) = if first.is_empty() {
        let suffix = last
            .parse::<u64>()
            .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
        if suffix == 0 {
            return Err(StatusCode::RANGE_NOT_SATISFIABLE);
        }
        (size.saturating_sub(suffix), size - 1)
    } else {
        let start = first
            .parse::<u64>()
            .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
        let end = if last.is_empty() {
            size - 1
        } else {
            last.parse::<u64>()
                .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?
                .min(size - 1)
        };
        (start, end)
    };
    if start >= size || start > end {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    Ok(Some(ByteRange { start, end }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn parses_normal_open_and_suffix_ranges() {
        assert_eq!(
            parse_range(&headers("bytes=2-5"), 10).unwrap(),
            Some(ByteRange { start: 2, end: 5 })
        );
        assert_eq!(
            parse_range(&headers("bytes=7-"), 10).unwrap(),
            Some(ByteRange { start: 7, end: 9 })
        );
        assert_eq!(
            parse_range(&headers("bytes=-3"), 10).unwrap(),
            Some(ByteRange { start: 7, end: 9 })
        );
    }

    #[test]
    fn rejects_multi_and_invalid_ranges() {
        assert!(parse_range(&headers("bytes=0-1,4-5"), 10).is_err());
        assert!(parse_range(&headers("bytes=99-"), 10).is_err());
        assert!(parse_range(&headers("items=1-2"), 10).is_err());
    }
}
