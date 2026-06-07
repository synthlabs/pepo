use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};

use reqwest::StatusCode;
use serde::de::DeserializeOwned;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_PREVIEW_BYTES: usize = 512;

#[derive(Debug)]
pub(crate) enum ProviderFetchError {
    Request {
        provider: String,
        scope: String,
        url: String,
        elapsed: Duration,
        source: reqwest::Error,
    },
    Status {
        provider: String,
        scope: String,
        url: String,
        status: StatusCode,
        elapsed: Duration,
        body_preview: String,
    },
    Decode {
        provider: String,
        scope: String,
        url: String,
        status: StatusCode,
        elapsed: Duration,
        body_preview: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for ProviderFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderFetchError::Request {
                provider,
                scope,
                url,
                elapsed,
                source,
            } => write!(
                f,
                "provider={provider} scope={scope} url={url} request failed after {:.1}ms: {source}",
                duration_ms(*elapsed)
            ),
            ProviderFetchError::Status {
                provider,
                scope,
                url,
                status,
                elapsed,
                body_preview,
            } => write!(
                f,
                "provider={provider} scope={scope} url={url} status={status} after {:.1}ms body_preview={body_preview:?}",
                duration_ms(*elapsed)
            ),
            ProviderFetchError::Decode {
                provider,
                scope,
                url,
                status,
                elapsed,
                body_preview,
                source,
            } => write!(
                f,
                "provider={provider} scope={scope} url={url} status={status} decode failed after {:.1}ms: {source}; body_preview={body_preview:?}",
                duration_ms(*elapsed)
            ),
        }
    }
}

impl Error for ProviderFetchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProviderFetchError::Request { source, .. } => Some(source),
            ProviderFetchError::Decode { source, .. } => Some(source),
            ProviderFetchError::Status { .. } => None,
        }
    }
}

pub(crate) fn provider_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("valid emote provider HTTP client")
}

pub(crate) async fn fetch_json<T>(
    client: &reqwest::Client,
    provider: &str,
    scope: impl Into<String>,
    url: &str,
) -> Result<T, ProviderFetchError>
where
    T: DeserializeOwned,
{
    let scope = scope.into();
    let start = Instant::now();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| ProviderFetchError::Request {
            provider: provider.to_owned(),
            scope: scope.clone(),
            url: url.to_owned(),
            elapsed: start.elapsed(),
            source,
        })?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|source| ProviderFetchError::Request {
            provider: provider.to_owned(),
            scope: scope.clone(),
            url: url.to_owned(),
            elapsed: start.elapsed(),
            source,
        })?;
    let elapsed = start.elapsed();
    let body_preview = preview_body(&body);

    if !status.is_success() {
        return Err(ProviderFetchError::Status {
            provider: provider.to_owned(),
            scope,
            url: url.to_owned(),
            status,
            elapsed,
            body_preview,
        });
    }

    serde_json::from_slice::<T>(&body).map_err(|source| ProviderFetchError::Decode {
        provider: provider.to_owned(),
        scope,
        url: url.to_owned(),
        status,
        elapsed,
        body_preview,
        source,
    })
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn preview_body(body: &[u8]) -> String {
    let len = body.len().min(BODY_PREVIEW_BYTES);
    String::from_utf8_lossy(&body[..len])
        .replace(['\n', '\r'], " ")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestResponse {
        value: String,
    }

    #[tokio::test]
    async fn fetch_json_decodes_successful_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/ok");
            then.status(200).json_body_obj(&serde_json::json!({
                "value": "ok"
            }));
        });

        let client = reqwest::Client::new();
        let response: TestResponse =
            fetch_json(&client, "TestProvider", "global", &server.url("/ok"))
                .await
                .unwrap();

        assert_eq!(
            response,
            TestResponse {
                value: "ok".to_owned()
            }
        );
        mock.assert();
    }

    #[tokio::test]
    async fn fetch_json_reports_non_success_status_with_body_preview() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/bad");
            then.status(503).body("service unavailable");
        });

        let client = reqwest::Client::new();
        let err =
            fetch_json::<TestResponse>(&client, "TestProvider", "global", &server.url("/bad"))
                .await
                .unwrap_err();

        match err {
            ProviderFetchError::Status {
                status,
                body_preview,
                ..
            } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
                assert_eq!(body_preview, "service unavailable");
            }
            other => panic!("expected status error, got {other:?}"),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn fetch_json_reports_decode_error_with_body_preview() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/invalid-json");
            then.status(200).body("not json");
        });

        let client = reqwest::Client::new();
        let err = fetch_json::<TestResponse>(
            &client,
            "TestProvider",
            "global",
            &server.url("/invalid-json"),
        )
        .await
        .unwrap_err();

        match err {
            ProviderFetchError::Decode {
                status,
                body_preview,
                ..
            } => {
                assert_eq!(status, StatusCode::OK);
                assert_eq!(body_preview, "not json");
            }
            other => panic!("expected decode error, got {other:?}"),
        }
        mock.assert();
    }
}
