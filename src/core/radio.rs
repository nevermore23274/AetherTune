use std::time::Duration;
use tokio::time::timeout;

use super::types::QueryKind;

/// Maximum time to wait for any single RadioBrowser API call.
const API_TIMEOUT: Duration = Duration::from_secs(10);

/// Describes what kind of fetch completed, so the main loop can apply it.
pub enum FetchResult {
    /// Replace the station list entirely (startup, category switch, search)
    Replace {
        stations: Vec<radiobrowser::ApiStation>,
        query: QueryKind,
        message: String,
    },
    /// Append to the current station list (load more)
    Append {
        stations: Vec<radiobrowser::ApiStation>,
    },
    /// The fetch failed
    Error(String),
}

/// Fetch blended global+local stations by tag.
pub async fn fetch_blended_by_tag(
    tag: String,
    country_code: String,
) -> Result<Vec<radiobrowser::ApiStation>, String> {
    let client = timeout(API_TIMEOUT, radiobrowser::RadioBrowserAPI::new())
        .await
        .map_err(|_| "RadioBrowser API timed out (DNS discovery)".to_string())?
        .map_err(|e| e.to_string())?;

    let mut global = timeout(API_TIMEOUT, client.get_stations()
        .tag(&tag)
        .order(radiobrowser::StationOrder::Votes)
        .reverse(true)
        .hidebroken(true)
        .limit("175")
        .send())
        .await
        .map_err(|_| "RadioBrowser API timed out (station fetch)".to_string())?
        .map_err(|e| e.to_string())?;
    filter_spam(&mut global);

    if country_code.is_empty() {
        return Ok(global);
    }

    // Local fetch — if it fails, just return global results
    let local_result: Result<Vec<radiobrowser::ApiStation>, String> = async {
        let client2 = timeout(API_TIMEOUT, radiobrowser::RadioBrowserAPI::new())
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;
        let mut local = timeout(API_TIMEOUT, client2.get_stations()
            .tag(&tag)
            .countrycode(&country_code)
            .order(radiobrowser::StationOrder::Votes)
            .reverse(true)
            .hidebroken(true)
            .limit("75")
            .send())
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;
        filter_spam(&mut local);
        Ok(local)
    }.await;

    match local_result {
        Ok(local) => Ok(interleave(global, local)),
        Err(_) => Ok(global),
    }
}

/// Fetch blended global+local stations by name search.
pub async fn fetch_blended_by_name(
    name: String,
    country_code: String,
) -> Result<Vec<radiobrowser::ApiStation>, String> {
    let client = timeout(API_TIMEOUT, radiobrowser::RadioBrowserAPI::new())
        .await
        .map_err(|_| "RadioBrowser API timed out (DNS discovery)".to_string())?
        .map_err(|e| e.to_string())?;

    let mut global = timeout(API_TIMEOUT, client.get_stations()
        .name(&name)
        .order(radiobrowser::StationOrder::Votes)
        .reverse(true)
        .hidebroken(true)
        .limit("175")
        .send())
        .await
        .map_err(|_| "RadioBrowser API timed out (station fetch)".to_string())?
        .map_err(|e| e.to_string())?;
    filter_spam(&mut global);

    if country_code.is_empty() {
        return Ok(global);
    }

    let local_result: Result<Vec<radiobrowser::ApiStation>, String> = async {
        let client2 = timeout(API_TIMEOUT, radiobrowser::RadioBrowserAPI::new())
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;
        let mut local = timeout(API_TIMEOUT, client2.get_stations()
            .name(&name)
            .countrycode(&country_code)
            .order(radiobrowser::StationOrder::Votes)
            .reverse(true)
            .hidebroken(true)
            .limit("75")
            .send())
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;
        filter_spam(&mut local);
        Ok(local)
    }.await;

    match local_result {
        Ok(local) => Ok(interleave(global, local)),
        Err(_) => Ok(global),
    }
}

/// Fetch additional stations for pagination (load more).
pub async fn fetch_more(
    query: QueryKind,
    offset: String,
    limit: String,
) -> Result<Vec<radiobrowser::ApiStation>, String> {
    let client = timeout(API_TIMEOUT, radiobrowser::RadioBrowserAPI::new())
        .await
        .map_err(|_| "RadioBrowser API timed out (DNS discovery)".to_string())?
        .map_err(|e| e.to_string())?;

    let mut stations = match &query {
        QueryKind::Tag(tag) => {
            timeout(API_TIMEOUT, client.get_stations()
                .tag(tag)
                .order(radiobrowser::StationOrder::Votes)
                .reverse(true)
                .hidebroken(true)
                .offset(offset)
                .limit(limit)
                .send())
                .await
                .map_err(|_| "RadioBrowser API timed out (station fetch)".to_string())?
                .map_err(|e| e.to_string())?
        }
        QueryKind::Search(name) => {
            timeout(API_TIMEOUT, client.get_stations()
                .name(name)
                .order(radiobrowser::StationOrder::Votes)
                .reverse(true)
                .hidebroken(true)
                .offset(offset)
                .limit(limit)
                .send())
                .await
                .map_err(|_| "RadioBrowser API timed out (station fetch)".to_string())?
                .map_err(|e| e.to_string())?
        }
    };
    filter_spam(&mut stations);
    Ok(stations)
}

/// Filter out spam stations — anything with an absurdly high vote count
/// is almost certainly botted. Shortwave uses 50K as their threshold.
fn filter_spam(stations: &mut Vec<radiobrowser::ApiStation>) {
    stations.retain(|s| s.votes < 50_000);
}

/// Interleave local stations into a global list, roughly every 3rd-4th position.
/// Deduplicates by URL — if a local station is already in global results, skip it.
pub fn interleave(
    global: Vec<radiobrowser::ApiStation>,
    local: Vec<radiobrowser::ApiStation>,
) -> Vec<radiobrowser::ApiStation> {
    if local.is_empty() {
        return global;
    }

    let global_urls: std::collections::HashSet<String> =
        global.iter().map(|s| s.url.clone()).collect();

    // Filter out locals that already appear in global
    let unique_local: Vec<radiobrowser::ApiStation> = local
        .into_iter()
        .filter(|s| !global_urls.contains(&s.url))
        .collect();

    if unique_local.is_empty() {
        return global;
    }

    // Insert one local station roughly every 3rd position
    let mut result = Vec::with_capacity(global.len() + unique_local.len());
    let mut local_iter = unique_local.into_iter();
    for (i, station) in global.into_iter().enumerate() {
        result.push(station);
        // After every 3rd global station, insert a local one if available
        if (i + 1) % 3 == 0 {
            if let Some(local_station) = local_iter.next() {
                result.push(local_station);
            }
        }
    }
    // Append any remaining local stations at the end
    result.extend(local_iter);

    result
}