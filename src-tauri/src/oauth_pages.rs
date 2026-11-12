// Branded HTML pages shown in the browser during Google / Outlook sign-in
// (served by the local OAuth redirect listener).
//
// Preview all pages without signing in:
//   cd src-tauri && cargo test --lib write_oauth_previews -- --ignored
// then open src-tauri/target/oauth-previews/index.html

use base64::{engine::general_purpose, Engine};
use once_cell::sync::Lazy;

static CAT_ICON_DATA_URI: Lazy<String> = Lazy::new(|| {
    format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(include_bytes!("../icons/cat.png"))
    )
});

pub enum OAuthPage<'a> {
    /// Sign-in finished; the app is completing the connection.
    Success,
    /// Provider returned an error (or a security check failed).
    Error(&'a str),
    /// Listener was hit before the provider redirected back.
    Waiting,
    /// Malformed request / wrong path.
    BadRequest,
    /// Non-GET request.
    MethodNotAllowed,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Full HTTP/1.1 response (status line, headers, body) for the given page.
pub fn http_response(provider: &str, page: &OAuthPage) -> String {
    let status = match page {
        OAuthPage::Success | OAuthPage::Waiting => "200 OK",
        OAuthPage::Error(_) | OAuthPage::BadRequest => "400 Bad Request",
        OAuthPage::MethodNotAllowed => "405 Method Not Allowed",
    };
    let body = page_html(provider, page);
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Just the HTML document for the given page.
pub fn page_html(provider: &str, page: &OAuthPage) -> String {
    let provider = escape_html(provider);
    // (kind class, badge, title, message, extra html, script)
    let (kind, badge, title, message, extra, script): (&str, &str, String, String, String, &str) =
        match page {
            OAuthPage::Success => (
                "success",
                "✓",
                format!("Signed in to {provider}!"),
                format!(
                    "CAT Desktop is finishing the connection to your {provider} calendar. \
                     You can close this tab and head back to the app &mdash; your events will start syncing in a moment."
                ),
                String::new(),
                // Only works if the browser lets scripts close this tab; harmless otherwise.
                "setTimeout(function(){ window.close(); }, 4000);",
            ),
            OAuthPage::Error(detail) => (
                "error",
                "!",
                "Something went wrong".to_string(),
                format!("We couldn't connect your {provider} account. Head back to CAT Desktop and try connecting again."),
                if detail.trim().is_empty() {
                    String::new()
                } else {
                    format!("<pre class=\"detail\">{}</pre>", escape_html(detail))
                },
                "",
            ),
            OAuthPage::Waiting => (
                "waiting",
                "",
                format!("Waiting for {provider}&hellip;"),
                "Finish signing in in the other tab. This page refreshes automatically.".to_string(),
                String::new(),
                "setTimeout(function(){ window.location.reload(); }, 2000);",
            ),
            OAuthPage::BadRequest | OAuthPage::MethodNotAllowed => (
                "error",
                "?",
                "This link isn't valid".to_string(),
                format!("To connect {provider}, start again from <strong>Settings &rarr; Connect</strong> in CAT Desktop."),
                String::new(),
                "",
            ),
        };

    let icon = CAT_ICON_DATA_URI.as_str();
    let badge_html = if kind == "waiting" {
        "<span class=\"badge spinner\" aria-hidden=\"true\"></span>".to_string()
    } else {
        format!("<span class=\"badge\" aria-hidden=\"true\">{badge}</span>")
    };
    let title_plain = title.replace("&hellip;", "...");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>CAT Desktop &middot; {title_plain}</title>
<link rel="icon" href="{icon}">
<style>
  :root {{
    color-scheme: light dark;
    --whale: #18C9D9;
    --raspberry: #D91858;
    --success: #27ae60;
    --main: #eeeeea;
    --card: #dcdde8;
    --text: #252526;
  }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --main: #252526; --card: #2d2d30; --text: #eeeeea; }}
  }}
  * {{ box-sizing: border-box; }}
  html, body {{ height: 100%; margin: 0; }}
  body {{
    background: var(--main);
    color: var(--text);
    font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Fira Sans', 'Droid Sans', 'Helvetica Neue', sans-serif;
    line-height: 1.6;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    overflow: hidden;
  }}
  /* same diagonal accent as the app background */
  body::after {{
    content: "";
    position: fixed;
    right: 0; bottom: 0;
    width: 42vw; height: 50vh;
    background: var(--whale);
    opacity: .3;
    clip-path: polygon(100% 100%, 0 100%, 100% 0);
    pointer-events: none;
  }}
  .card {{
    position: relative;
    z-index: 1;
    width: min(32rem, 100%);
    background: var(--card);
    border-radius: 16px;
    box-shadow: 0 10px 40px rgba(0,0,0,.25);
    padding: 2.5rem 2rem 1.75rem;
    text-align: center;
    border-top: 4px solid var(--accent);
    animation: rise .5s cubic-bezier(.2,.8,.2,1) both;
  }}
  .success {{ --accent: var(--whale); }}
  .error   {{ --accent: var(--raspberry); }}
  .waiting {{ --accent: var(--whale); }}
  .icon-wrap {{ position: relative; width: 104px; height: 104px; margin: 0 auto 1.25rem; }}
  .icon-wrap img {{ width: 104px; height: 104px; display: block; }}
  .badge {{
    position: absolute; right: -6px; bottom: -2px;
    width: 34px; height: 34px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    background: var(--accent); color: #fff;
    font-weight: 700; font-size: 18px;
    border: 3px solid var(--card);
  }}
  .success .badge {{ background: var(--success); }}
  .badge.spinner {{
    background: var(--card);
    border: 4px solid rgba(127,127,127,.35);
    border-top-color: var(--whale);
    animation: spin 1s linear infinite;
  }}
  h1 {{ margin: 0 0 .5rem; font-size: 1.6rem; color: var(--accent); font-weight: 700; }}
  p  {{ margin: 0 auto; max-width: 26rem; opacity: .9; }}
  .detail {{
    margin: 1.25rem 0 0;
    padding: .75rem 1rem;
    text-align: left;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: .85rem;
    background: rgba(217, 24, 88, .1);
    border-left: 3px solid var(--raspberry);
    border-radius: 6px;
  }}
  footer {{
    margin-top: 1.75rem;
    padding-top: 1rem;
    border-top: 1px solid rgba(127,127,127,.25);
    font-size: .8rem;
    opacity: .7;
  }}
  footer strong {{ color: var(--whale); font-weight: 600; }}
  @keyframes rise {{ from {{ opacity: 0; transform: translateY(12px); }} to {{ opacity: 1; transform: none; }} }}
  @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
  @media (prefers-reduced-motion: reduce) {{ .card, .badge.spinner {{ animation: none; }} }}
</style>
</head>
<body>
  <main class="card {kind}" role="main">
    <div class="icon-wrap">
      <img src="{icon}" alt="CAT Desktop">
      {badge_html}
    </div>
    <h1>{title}</h1>
    <p>{message}</p>
    {extra}
    <footer><strong>CAT Desktop</strong> &middot; Calendar AssistanT &middot; {provider}</footer>
  </main>
  <script>{script}</script>
</body>
</html>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_error_detail() {
        let html = page_html("Google", &OAuthPage::Error("<script>alert(1)</script>"));
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn content_length_matches_body_bytes() {
        let resp = http_response("Outlook", &OAuthPage::Success);
        let (head, body) = resp.split_once("\r\n\r\n").unwrap();
        assert!(head.contains(&format!("Content-Length: {}", body.len())));
    }

    /// Writes every page to target/oauth-previews/ so they can be checked
    /// in a browser without going through a real sign-in.
    #[test]
    #[ignore]
    fn write_oauth_previews() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/oauth-previews");
        std::fs::create_dir_all(&dir).unwrap();
        let pages: Vec<(&str, &str, OAuthPage)> = vec![
            ("google-success", "Google", OAuthPage::Success),
            ("outlook-success", "Outlook", OAuthPage::Success),
            ("outlook-error", "Outlook", OAuthPage::Error("access_denied: The user has denied access to the scope requested by the client application.")),
            ("outlook-csrf", "Outlook", OAuthPage::Error("Security check failed (invalid state parameter). Please try connecting again.")),
            ("google-waiting", "Google", OAuthPage::Waiting),
            ("outlook-bad-request", "Outlook", OAuthPage::BadRequest),
        ];
        let mut index = String::from("<!DOCTYPE html><meta charset=utf-8><title>OAuth page previews</title><style>body{font-family:sans-serif;padding:2rem}iframe{width:100%;height:560px;border:1px solid #888;border-radius:8px;margin-bottom:2rem}</style><h1>OAuth page previews</h1>");
        for (name, provider, page) in &pages {
            let mut html = page_html(provider, page);
            // Don't auto-close / auto-reload in previews
            html = html.replace("window.close();", "").replace("window.location.reload();", "");
            std::fs::write(dir.join(format!("{name}.html")), html).unwrap();
            index.push_str(&format!("<h2>{name}</h2><iframe src=\"{name}.html\"></iframe>"));
        }
        std::fs::write(dir.join("index.html"), index).unwrap();
        println!("Previews written to {}", dir.display());
    }
}
