use google_calendar3::{CalendarHub, oauth2, hyper, hyper_rustls};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use std::default::Default;

pub async fn list_upcoming_events() -> Result<(), Box<dyn std::error::Error>> {
    // Set up OAuth2 authenticator (clientsecret.json from Google Cloud Console)
    let secret = yup_oauth2::read_application_secret("clientsecret.json").await?;
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk("tokencache.json")
    .build()
    .await?;

    // Set up the Calendar API hub
    let hub = CalendarHub::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

    // Make the API call
    let result = hub.events().list("primary")
        .max_results(10)
        .single_events(true)
        .order_by("startTime")
        .time_min(chrono::Utc::now().to_rfc3339())
        .doit().await?;

    // Print event summaries
    if let Some(items) = result.1.items {
        for event in items {
            let summary = event.summary.unwrap_or("No Title".to_string());
            let start = event.start.and_then(|s| s.date_time.or(s.date)).unwrap_or("No Start".to_string());
            println!("{} ({})", summary, start);
        }
    } else {
        println!("No events found.");
    }

    Ok(())
}