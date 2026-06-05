use crate::error::AppError;
use axum::http::HeaderMap;

pub const SCHOOL_SUBDOMAIN_HEADER: &str = "x-school-subdomain";

/// Extract subdomain from X-School-Subdomain header or Origin/Referer.
///
/// Browser tenant requests normally rely on Origin/Referer. X-School-Subdomain
/// is an explicit override for local, custom-host, script, or non-browser clients.
pub fn extract_subdomain_from_request(headers: &HeaderMap) -> Result<String, AppError> {
    if let Some(subdomain_header) = headers.get(SCHOOL_SUBDOMAIN_HEADER) {
        let subdomain = subdomain_header
            .to_str()
            .ok()
            .and_then(normalize_subdomain)
            .ok_or_else(|| bad_request("Invalid subdomain"))?;

        if let Some(origin_subdomain) = origin_subdomain(headers) {
            if origin_subdomain != subdomain {
                return Err(bad_request("Subdomain header does not match origin"));
            }
        }

        return Ok(subdomain);
    }

    let url = origin_or_referer(headers)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| bad_request("No subdomain specified"))?;

    extract_subdomain_from_url(url).ok_or_else(|| bad_request("Invalid domain"))
}

fn origin_or_referer(headers: &HeaderMap) -> Option<&axum::http::HeaderValue> {
    headers.get("origin").or_else(|| headers.get("referer"))
}

fn origin_subdomain(headers: &HeaderMap) -> Option<String> {
    origin_or_referer(headers)
        .and_then(|h| h.to_str().ok())
        .and_then(extract_subdomain_from_url)
}

fn extract_subdomain_from_url(url: &str) -> Option<String> {
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()?
        .rsplit('@')
        .next()?
        .split(':')
        .next()?;

    extract_subdomain_from_host(host)
}

fn extract_subdomain_from_host(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let parts: Vec<&str> = host.split('.').collect();

    if parts.len() < 3 || parts[parts.len() - 2] != "schoolorbit" || parts.last()? != &"app" {
        return None;
    }

    normalize_subdomain(parts[0])
}

fn normalize_subdomain(subdomain: &str) -> Option<String> {
    let subdomain = subdomain.trim().to_ascii_lowercase();

    if subdomain.is_empty()
        || subdomain == "www"
        || !subdomain
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return None;
    }

    Some(subdomain)
}

fn bad_request(error: &str) -> AppError {
    AppError::BadRequest(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers_with(name: &'static str, value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn extracts_header_subdomain_first() {
        let mut headers = headers_with(SCHOOL_SUBDOMAIN_HEADER, "Sandbox");
        headers.insert(
            "origin",
            HeaderValue::from_static("https://sandbox.schoolorbit.app"),
        );

        let subdomain = extract_subdomain_from_request(&headers).unwrap();

        assert_eq!(subdomain, "sandbox");
    }

    #[test]
    fn rejects_invalid_header_subdomain() {
        let headers = headers_with(SCHOOL_SUBDOMAIN_HEADER, "bad_domain");

        assert!(extract_subdomain_from_request(&headers).is_err());
    }

    #[test]
    fn rejects_header_that_does_not_match_origin() {
        let mut headers = headers_with(SCHOOL_SUBDOMAIN_HEADER, "sandbox");
        headers.insert(
            "origin",
            HeaderValue::from_static("https://demo.schoolorbit.app"),
        );

        assert!(extract_subdomain_from_request(&headers).is_err());
    }

    #[test]
    fn accepts_header_with_localhost_origin_for_local_dev() {
        let mut headers = headers_with(SCHOOL_SUBDOMAIN_HEADER, "sandbox");
        headers.insert("origin", HeaderValue::from_static("http://localhost:5173"));

        let subdomain = extract_subdomain_from_request(&headers).unwrap();

        assert_eq!(subdomain, "sandbox");
    }

    #[test]
    fn extracts_origin_subdomain() {
        let headers = headers_with("origin", "https://sandbox.schoolorbit.app");

        let subdomain = extract_subdomain_from_request(&headers).unwrap();

        assert_eq!(subdomain, "sandbox");
    }

    #[test]
    fn extracts_referer_subdomain_with_path_and_port() {
        let headers = headers_with(
            "referer",
            "https://demo.schoolorbit.app:443/staff/dashboard",
        );

        let subdomain = extract_subdomain_from_request(&headers).unwrap();

        assert_eq!(subdomain, "demo");
    }

    #[test]
    fn rejects_root_or_localhost_domains() {
        assert!(
            extract_subdomain_from_request(&headers_with("origin", "https://schoolorbit.app"))
                .is_err()
        );
        assert!(
            extract_subdomain_from_request(&headers_with("origin", "http://localhost:5173"))
                .is_err()
        );
    }
}
