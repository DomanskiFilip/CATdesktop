use chrono::{Local, NaiveDate};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyWeather {
    pub date: String,
    pub weather: String,
    pub temperature_2m_max: f32,
    pub wind_speed_10m_max: f32,
}

// Function to fetch weekly weather data from Open-Meteo API and cache it //
#[tauri::command]
pub async fn get_weekly_weather(
    app_handle: AppHandle,
    latitude: f64,
    longitude: f64,
) -> Result<HashMap<String, DailyWeather>, String> {
    let cache_path = get_weather_cache_path(&app_handle);
    let today = Local::now().date_naive();

    // Try to load cached data
    if let Ok(cached) = fs::read_to_string(&cache_path) {
        if let Ok(weather_data) = serde_json::from_str::<HashMap<String, DailyWeather>>(&cached) {
            // If cache is for today return it
            if let Some(first_date) = weather_data.keys().next() {
                if NaiveDate::parse_from_str(first_date, "%Y-%m-%d").unwrap_or(today) >= today {
                    return Ok(weather_data);
                }
            }
        }
    }

    // Fetch new data from Open-Meteo
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&daily=weathercode,temperature_2m_max,wind_speed_10m_max&timezone=auto",
        latitude, longitude
    );
    let resp = Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    let codes = parsed["daily"]["weathercode"]
        .as_array()
        .ok_or("Failed to parse daily weather codes")?;
    let dates = parsed["daily"]["time"]
        .as_array()
        .ok_or("Failed to parse daily dates")?;
    let temps = parsed["daily"]["temperature_2m_max"]
        .as_array()
        .ok_or("Failed to parse daily temperatures")?;
    let winds = parsed["daily"]["wind_speed_10m_max"]
        .as_array()
        .ok_or("Failed to parse daily wind speeds")?;

    let mut result = HashMap::new();
    for i in 0..dates.len() {
        let date = dates
            .get(i)
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string();
        let code_val = codes.get(i).and_then(|c| c.as_u64()).unwrap_or(0) as u8;
        let description = weather_code_to_string(code_val);
        let temp = temps.get(i).and_then(|t| t.as_f64()).unwrap_or(0.0) as f32;
        let wind = winds.get(i).and_then(|w| w.as_f64()).unwrap_or(0.0) as f32;

        result.insert(
            date.clone(),
            DailyWeather {
                date,
                weather: description,
                temperature_2m_max: temp,
                wind_speed_10m_max: wind,
            },
        );
    }

    // Save to cache
    fs::write(&cache_path, serde_json::to_string(&result).unwrap()).map_err(|e| e.to_string())?;
    Ok(result)
}

// helper function to convert weather codes to human-readable strings //
fn weather_code_to_string(code: u8) -> String {
    match code {
        0 => "Clear sky",
        1 | 2 | 3 => "Mainly clear",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow fall",
        77 => "Snow grains",
        80 | 81 | 82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
    .to_string()
}

// Function to get the path for the weather cache file //
fn get_weather_cache_path(app_handle: &AppHandle) -> PathBuf {
    let app_data_dir = app_handle.path().app_data_dir().unwrap();
    app_data_dir.join("weather_cache.txt")
}
