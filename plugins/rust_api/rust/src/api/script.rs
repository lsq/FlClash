use flutter_rust_bridge::frb;

/// Runs `main(config)` from a profile override script and returns the JSON the
/// script produced. `config` is the profile as JSON; the result replaces it.
///
/// `proxy` is `Some("http://127.0.0.1:<mixed-port>")` whenever the core's TUN
/// is active, so `fetch()` calls inside the script go through mihomo's own
/// (already VPN-protected) listener instead of opening a raw socket that the
/// device's own TUN routes would otherwise capture and abort.
#[frb]
pub fn evaluate_script(
    script: String,
    config: String,
    proxy: Option<String>,
) -> Result<String, String> {
    crate::script::evaluate(&script, &config, proxy.as_deref())
}
