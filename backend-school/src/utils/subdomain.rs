use crate::error::AppError;
use axum::http::HeaderMap;
use std::collections::HashSet;
use url::Url;

pub const SCHOOL_SUBDOMAIN_HEADER: &str = "x-school-subdomain";

#[derive(Clone, Debug)]
pub struct TenantOriginPolicy {
    base_domain: String,
    allowed_dev_origins: HashSet<String>,
}

impl TenantOriginPolicy {
    pub fn new<'a, I>(base_domain: &str, allowed_dev_origins: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        Self {
            base_domain: base_domain.to_ascii_lowercase(),
            allowed_dev_origins: allowed_dev_origins
                .into_iter()
                .filter_map(normalized_origin)
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests<'a, I>(base_domain: &str, allowed_dev_origins: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        Self::new(base_domain, allowed_dev_origins)
    }

    pub fn validate(&self, raw_origin: &str, tenant: &str) -> Result<(), AppError> {
        let parsed = parse_authoritative_url(raw_origin, true)?;
        let resolved = self.production_tenant(&parsed)?;
        let tenant = normalize_subdomain(tenant).ok_or_else(origin_rejected)?;
        if resolved.as_deref() == Some(tenant.as_str()) {
            Ok(())
        } else {
            Err(origin_rejected())
        }
    }

    pub fn resolve_tenant(
        &self,
        headers: &HeaderMap,
        dev_realtime_tenant_hint: Option<&str>,
    ) -> Result<String, AppError> {
        let origin = single_header(headers, "origin")?;
        let referer = single_header(headers, "referer")?;
        let header_hint = single_header(headers, SCHOOL_SUBDOMAIN_HEADER)?
            .map(validated_hint)
            .transpose()?;
        let query_hint = dev_realtime_tenant_hint.map(validated_hint).transpose()?;

        let hint = match (header_hint, query_hint) {
            (Some(header), Some(query)) if header != query => return Err(origin_rejected()),
            (Some(header), _) => Some(header),
            (_, Some(query)) => Some(query),
            (None, None) => None,
        };

        let (raw_url, strict_origin) = match (origin, referer) {
            (Some(origin), _) => (origin, true),
            (None, Some(referer)) => (referer, false),
            (None, None) => return Err(origin_rejected()),
        };
        let parsed = parse_authoritative_url(raw_url, strict_origin)?;
        let normalized = parsed.origin().ascii_serialization();

        if let Some(production) = self.production_tenant(&parsed)? {
            if hint.as_deref().is_some_and(|hint| hint != production) {
                return Err(origin_rejected());
            }
            return Ok(production);
        }

        if self.allowed_dev_origins.contains(&normalized) {
            return hint.ok_or_else(origin_rejected);
        }

        Err(origin_rejected())
    }

    fn production_tenant(&self, parsed: &Url) -> Result<Option<String>, AppError> {
        if parsed.scheme() != "https" || parsed.port().is_some() {
            return Ok(None);
        }
        let host = parsed.host_str().ok_or_else(origin_rejected)?;
        let suffix = format!(".{}", self.base_domain);
        let Some(prefix) = host.strip_suffix(&suffix) else {
            return Ok(None);
        };
        if prefix.contains('.') {
            return Ok(None);
        }
        Ok(normalize_subdomain(prefix))
    }
}

pub fn parse_realtime_tenant_hint(raw_query: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(raw_query) = raw_query else {
        return Ok(None);
    };
    let mut hint = None;
    for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
        if key != "school_subdomain" {
            continue;
        }
        if hint.is_some() {
            return Err(origin_rejected());
        }
        hint = Some(validated_hint(value.as_ref())?);
    }
    Ok(hint)
}

fn single_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<Option<&'a str>, AppError> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(origin_rejected());
    }
    value.to_str().map(Some).map_err(|_| origin_rejected())
}

fn validated_hint(value: &str) -> Result<String, AppError> {
    normalize_subdomain(value).ok_or_else(origin_rejected)
}

fn parse_authoritative_url(value: &str, strict_origin: bool) -> Result<Url, AppError> {
    let parsed = Url::parse(value).map_err(|_| origin_rejected())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || (strict_origin && (parsed.path() != "/" || parsed.query().is_some()))
    {
        return Err(origin_rejected());
    }
    Ok(parsed)
}

fn normalized_origin(value: &str) -> Option<String> {
    parse_authoritative_url(value, true)
        .ok()
        .map(|url| url.origin().ascii_serialization())
}

fn origin_rejected() -> AppError {
    AppError::Forbidden("origin_rejected".to_string())
}

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
    if subdomain.is_empty()
        || subdomain.trim() != subdomain
        || subdomain.len() > 63
        || !subdomain.is_ascii()
        || subdomain.starts_with('-')
        || subdomain.ends_with('-')
    {
        return None;
    }
    let subdomain = subdomain.to_ascii_lowercase();

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

    #[test]
    fn strict_origin_policy_rejects_noncanonical_production_origins() {
        let policy = TenantOriginPolicy::for_tests("schoolorbit.app", []);

        for origin in [
            "http://demo.schoolorbit.app",
            "https://demo.schoolorbit.app:444",
            "https://user@demo.schoolorbit.app",
            "https://demo.schoolorbit.app/path",
            "https://demo.schoolorbit.app?query=value",
            "https://demo.schoolorbit.app#fragment",
            "https://nested.demo.schoolorbit.app",
            "null",
        ] {
            let headers = headers_with("origin", origin);
            assert!(policy.resolve_tenant(&headers, None).is_err(), "{origin}");
        }
    }

    #[test]
    fn strict_origin_policy_accepts_referer_path_and_checks_production_hints() {
        let policy = TenantOriginPolicy::for_tests("schoolorbit.app", []);
        let mut headers = headers_with(
            "referer",
            "https://demo.schoolorbit.app/account/security?tab=sessions",
        );

        assert_eq!(policy.resolve_tenant(&headers, None).unwrap(), "demo");
        headers.insert(SCHOOL_SUBDOMAIN_HEADER, HeaderValue::from_static("demo"));
        assert_eq!(policy.resolve_tenant(&headers, None).unwrap(), "demo");
        headers.insert(SCHOOL_SUBDOMAIN_HEADER, HeaderValue::from_static("other"));
        assert!(policy.resolve_tenant(&headers, None).is_err());
    }

    #[test]
    fn production_hostname_remains_authoritative_even_if_allowlisted_for_development() {
        let policy =
            TenantOriginPolicy::for_tests("schoolorbit.app", ["https://demo.schoolorbit.app"]);
        let headers = headers_with("origin", "https://demo.schoolorbit.app");

        assert_eq!(policy.resolve_tenant(&headers, None).unwrap(), "demo");
    }

    #[test]
    fn development_tenant_hints_are_exact_ascii_labels() {
        let policy = TenantOriginPolicy::for_tests("schoolorbit.app", ["http://localhost:5173"]);

        for hint in [
            " demo",
            "demo ",
            "-demo",
            "demo-",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            let mut headers = headers_with("origin", "http://localhost:5173");
            headers.insert(
                SCHOOL_SUBDOMAIN_HEADER,
                HeaderValue::from_str(hint).unwrap(),
            );
            assert!(policy.resolve_tenant(&headers, None).is_err(), "{hint}");
        }

        assert!(parse_realtime_tenant_hint(Some("school_subdomain=%2564emo")).is_err());
    }
}
