use axum::http::HeaderMap;

use crate::storage::types::Consistency;

pub const CONSISTENCY_HEADER: &str = "x-rux-consistency";

pub fn extract_consistency(headers: &HeaderMap, default: &Consistency) -> Consistency {
    headers
        .get(CONSISTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(*default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_consistency_with_valid_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rux-consistency", "quorum".parse().unwrap());
        let default = Consistency::Eventual;
        let result = extract_consistency(&headers, &default);
        assert!(matches!(result, Consistency::Quorum));
    }

    #[test]
    fn extract_consistency_with_invalid_header_returns_default() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rux-consistency", "bogus".parse().unwrap());
        let default = Consistency::Strong;
        let result = extract_consistency(&headers, &default);
        assert!(matches!(result, Consistency::Strong));
    }

    #[test]
    fn extract_consistency_with_no_header_returns_default() {
        let headers = HeaderMap::new();
        let default = Consistency::All;
        let result = extract_consistency(&headers, &default);
        assert!(matches!(result, Consistency::All));
    }
}
