use rquickjs::function::Opt;
use rquickjs::{Ctx, Function, Object, Result, Value};
use std::sync::OnceLock;
use std::time::Duration;

// Scripts run inside a hard wall-clock budget (see mod.rs::TIMEOUT); fetch's
// own timeout only needs to keep a hung socket from outliving the interrupt
// handler by an unreasonable margin.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// Certificate verifier that accepts anything. The scripting sandbox targets
/// hosts the user already trusts (their own subscription/profile endpoints)
/// and should not fail on self-signed or misconfigured certificates.
#[derive(Debug)]
struct NoVerify;

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(NoVerify))
            .with_no_client_auth();

        ureq::AgentBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            .tls_config(std::sync::Arc::new(tls_config))
            .build()
    })
}

/// Exposes a synchronous, blocking `fetch(url, options?)` to profile scripts.
/// Unlike the browser API this returns a plain result object rather than a
/// Promise, since the caller already blocks on the whole script.
pub fn install(ctx: &Ctx<'_>) -> Result<()> {
    let fetch = Function::new(ctx.clone(), get)?;
    ctx.globals().set("fetch", fetch)
}

fn get<'js>(ctx: Ctx<'js>, url: String, options: Opt<Object<'js>>) -> Result<Object<'js>> {
    let options = options.0; // Option<Object<'js>>
    let mut request = agent().get(&url);

    if let Some(options) = &options {
        if let Ok(referer) = options.get::<_, String>("referer") {
            request = request.set("Referer", &referer);
        }
        if let Ok(headers) = options.get::<_, Object>("headers") {
            for entry in headers.props::<String, String>() {
                let (key, value) = entry?;
                request = request.set(&key, &value);
            }
        }
    }

    let response = request.call().map_err(|error| describe(&ctx, error))?;
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|error| rquickjs::Exception::throw_message(&ctx, &error.to_string()))?;

    let result = Object::new(ctx.clone())?;
    result.set("status", status)?;
    result.set("text", text.clone())?;

    let parse_json = Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> Result<Value<'js>> {
        ctx.json_parse(text.clone())
    })?;
    result.set("json", parse_json)?;

    Ok(result)
}

fn describe(ctx: &Ctx<'_>, error: ureq::Error) -> rquickjs::Error {
    rquickjs::Exception::throw_message(ctx, &error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{CatchResultExt, Context, Runtime};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    // Avoid colliding with the `rquickjs::Result<T>` alias pulled in by `use super::*`.
    type TestResult<T> = std::result::Result<T, String>;

    fn serve_once(response: &'static str) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/");

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    break;
                }
                headers.push(line);
            }
            let mut stream = stream;
            stream.write_all(response.as_bytes()).unwrap();
            headers
        });

        (url, handle)
    }

    fn eval(script: &str) -> TestResult<String> {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx).catch(&ctx).map_err(super::super::describe)?;
            let value: Value = ctx
                .eval(script.as_bytes())
                .catch(&ctx)
                .map_err(super::super::describe)?;
            let json = ctx
                .json_stringify(value)
                .catch(&ctx)
                .map_err(super::super::describe)?
                .ok_or_else(|| "result is not JSON".to_owned())?;
            json.to_string().map_err(|e| e.to_string())
        })
    }

    #[test]
    fn returns_status_and_text_for_a_successful_get() {
        let (url, handle) = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");

        let result = eval(&format!(
            "(function() {{ const r = fetch('{url}'); return {{ status: r.status, text: r.text }}; }})()"
        ))
        .unwrap();

        handle.join().unwrap();
        assert_eq!(result, r#"{"status":200,"text":"hello"}"#);
    }

    #[test]
    fn sends_custom_headers_and_referer() {
        let (url, handle) = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");

        eval(&format!(
            "fetch('{url}', {{ headers: {{ 'X-Test': 'abc' }}, referer: 'https://example.com' }})"
        ))
        .unwrap();

        let headers = handle.join().unwrap();
        assert!(headers
            .iter()
            .any(|h| h.eq_ignore_ascii_case("x-test: abc")));
        assert!(headers.iter().any(|h| h
            .to_ascii_lowercase()
            .starts_with("referer: https://example.com")));
    }

    #[test]
    fn parses_a_json_response_body() {
        let body = r#"{"ok":true,"value":42}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, handle) = serve_once(Box::leak(response.into_boxed_str()));

        let result = eval(&format!(
            "(function() {{ const r = fetch('{url}'); return r.json(); }})()"
        ))
        .unwrap();

        handle.join().unwrap();
        assert_eq!(result, r#"{"ok":true,"value":42}"#);
    }

    #[test]
    fn a_connection_failure_is_a_catchable_error_not_a_panic() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let error = eval(&format!("fetch('http://127.0.0.1:{port}/')")).unwrap_err();

        assert!(!error.is_empty());
    }

    #[test]
    fn an_invalid_url_is_a_catchable_error() {
        let error = eval("fetch('not a url')").unwrap_err();

        assert!(!error.is_empty());
    }
}
