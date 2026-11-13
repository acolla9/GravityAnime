//! AniList integration (https://anilist.gitbook.io/anilist-apiv2-docs/).
//! Login is a pasted access token; progress is pushed when an episode finishes.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;

const API_URL: &str = "https://graphql.anilist.co";
const MEDIA_FIELDS: &str = "id episodes mediaListEntry { progress status }";

/// AniList said the token is not valid (wrong, revoked or expired).
#[derive(Debug)]
pub struct TokenRejected;

impl fmt::Display for TokenRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AniList rejected the access token (it may be invalid or expired).")
    }
}

impl std::error::Error for TokenRejected {}

pub fn is_token_rejected(error: &anyhow::Error) -> bool {
    error.downcast_ref::<TokenRejected>().is_some()
}

/// Tidy up whatever the user pasted: whitespace, quotes, a "Bearer " prefix, or a whole
/// redirect URL containing `access_token=...`.
pub fn clean_token(raw: &str) -> String {
    let mut t = raw.trim();
    if let Some(pos) = t.find("access_token=") {
        t = &t[pos + "access_token=".len()..];
        t = t.split('&').next().unwrap_or(t);
    }
    let t = t.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    let t = t.strip_prefix("Bearer ").unwrap_or(t);
    t.trim().to_string()
}

async fn gql(token: &str, query: &str, variables: Value) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("GravityAnime/1.0")
        .build()
        .context("creating HTTP client")?;
    let response = client
        .post(API_URL)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(&json!({ "query": query, "variables": variables }))
        .send()
        .await
        .context("could not reach AniList")?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("reading the AniList response")?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);

    let invalid_token = body["errors"].as_array().is_some_and(|errors| {
        errors.iter().any(|e| {
            e["message"]
                .as_str()
                .is_some_and(|m| m.to_lowercase().contains("invalid token"))
        })
    });
    if status.as_u16() == 401 || invalid_token {
        return Err(TokenRejected.into());
    }
    if let Some(message) = body["errors"][0]["message"].as_str() {
        bail!("AniList error: {message}");
    }
    if !status.is_success() {
        bail!("AniList returned HTTP {status}");
    }
    Ok(body["data"].clone())
}

/// The name of the account the token belongs to. Also works as a token check.
pub async fn viewer(token: &str) -> Result<String> {
    let data = gql(token, "query { Viewer { name } }", json!({})).await?;
    data["Viewer"]["name"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("AniList did not return an account for this token"))
}

struct Found {
    media_id: i64,
    total: Option<i64>,
    progress: i64,
    status: Option<String>,
}

fn parse_found(media: &Value) -> Option<Found> {
    Some(Found {
        media_id: media["id"].as_i64()?,
        total: media["episodes"].as_i64(),
        progress: media["mediaListEntry"]["progress"].as_i64().unwrap_or(0),
        status: media["mediaListEntry"]["status"]
            .as_str()
            .map(str::to_string),
    })
}

/// Find the AniList entry for a show: by MAL id when we have one, otherwise by title.
async fn find_media(token: &str, mal_id: Option<&str>, title: &str) -> Result<Found> {
    if let Some(mal) = mal_id.and_then(|m| m.trim().parse::<i64>().ok()) {
        let query = format!(
            "query ($mal: Int) {{ Media(idMal: $mal, type: ANIME) {{ {} }} }}",
            MEDIA_FIELDS
        );
        if let Ok(data) = gql(token, &query, json!({ "mal": mal })).await {
            if let Some(found) = parse_found(&data["Media"]) {
                return Ok(found);
            }
        }
    }

    let query = format!(
        "query ($search: String) {{ Media(search: $search, type: ANIME) {{ {} }} }}",
        MEDIA_FIELDS
    );
    let data = gql(token, &query, json!({ "search": title }))
        .await
        .with_context(|| format!("could not find \"{title}\" on AniList"))?;
    parse_found(&data["Media"]).ok_or_else(|| anyhow!("could not find \"{title}\" on AniList"))
}

/// Whole episode numbers only: specials like "5.5" or "OVA" are not tracked.
fn parse_episode(number: &str) -> Option<i64> {
    let value: f64 = number.trim().parse().ok()?;
    if value >= 1.0 && value.fract() == 0.0 {
        Some(value as i64)
    } else {
        None
    }
}

/// Set the show's progress to `episode_number` on the logged-in account's list.
/// Never lowers progress, so rewatching an old episode changes nothing. Marks the show
/// COMPLETED on its last episode, otherwise CURRENT (or leaves REPEATING alone).
pub async fn sync_progress(
    token: &str,
    mal_id: Option<&str>,
    title: &str,
    episode_number: &str,
) -> Result<String> {
    let episode = parse_episode(episode_number)
        .ok_or_else(|| anyhow!("episode \"{episode_number}\" is not a whole number, skipping"))?;
    let found = find_media(token, mal_id, title).await?;

    if found.progress >= episode {
        return Ok(format!(
            "AniList already has episode {} or later for \"{title}\".",
            found.progress
        ));
    }

    let is_last = found.total.is_some_and(|total| total > 0 && episode >= total);
    let status = if is_last {
        "COMPLETED"
    } else if found.status.as_deref() == Some("REPEATING") {
        "REPEATING"
    } else {
        "CURRENT"
    };

    gql(
        token,
        "mutation ($id: Int, $progress: Int, $status: MediaListStatus) { \
           SaveMediaListEntry(mediaId: $id, progress: $progress, status: $status) { id } }",
        json!({ "id": found.media_id, "progress": episode, "status": status }),
    )
    .await?;
    Ok(format!(
        "Synced episode {episode} of \"{title}\" to AniList ({status})."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_numbers_must_be_whole() {
        assert_eq!(parse_episode("5"), Some(5));
        assert_eq!(parse_episode(" 12 "), Some(12));
        assert_eq!(parse_episode("5.0"), Some(5));
        assert_eq!(parse_episode("5.5"), None);
        assert_eq!(parse_episode("0"), None);
        assert_eq!(parse_episode("OVA"), None);
    }

    #[test]
    fn token_is_cleaned() {
        assert_eq!(clean_token("  abc.def  "), "abc.def");
        assert_eq!(clean_token("\"abc.def\""), "abc.def");
        assert_eq!(clean_token("Bearer abc.def"), "abc.def");
        assert_eq!(
            clean_token("https://anilist.co/api/v2/oauth/pin#access_token=abc.def&token_type=Bearer"),
            "abc.def"
        );
    }
}