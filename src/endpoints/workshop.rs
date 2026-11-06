use crate::models::responses::ErrorResponse;
use crate::models::workshop::AssetDetail;
use crate::models::workshop::AssetDetailResponse;
use crate::models::workshop::AssetScenariosResponse;
use crate::models::workshop::AssetVersionDetail;
use crate::models::workshop::AssetsPage;
use crate::models::workshop::Sort;
use crate::models::workshop::UPSTREAM_HOST;
use crate::models::workshop::WorkshopQuery;
use crate::models::workshop::WorkshopResponse;
use crate::models::workshop::upstream_detail_base;
use crate::models::workshop::upstream_workshop_json_url;
use axum::extract::Path;
use axum::extract::Query;
use reqwest::Client;
use std::sync::LazyLock;
use tokio::sync::RwLock;

use crate::shared::AppJson;
use crate::shared::ArsaError;

/// Cache for the upstream Next.js build ID (see `upstream_workshop_json_url`
/// for why this can't just be a constant).
static BUILD_ID: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

async fn cached_build_id(client: &Client) -> Result<String, ArsaError> {
    if let Some(id) = BUILD_ID.read().await.clone() {
        return Ok(id);
    }
    refresh_build_id(client).await
}

/// Fetches the live Workshop page and pulls the current build ID out of its
/// embedded `__NEXT_DATA__` payload, then caches it for subsequent requests.
async fn refresh_build_id(client: &Client) -> Result<String, ArsaError> {
    let html = client
        .get(format!("{UPSTREAM_HOST}/workshop"))
        .send()
        .await?
        .text()
        .await?;

    let build_id = html
        .split("\"buildId\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .ok_or_else(|| {
            ArsaError::UnknownError("could not resolve upstream workshop build id".into())
        })?
        .to_string();

    *BUILD_ID.write().await = Some(build_id.clone());

    Ok(build_id)
}

/// Search the Arma Reforger Workshop
///
/// Proxies requests to the upstream Arma Platform workshop API with optional
/// filtering by search term, tags, sort order, and page.
#[utoipa::path(
    get,
    path = "/workshop",
    params(WorkshopQuery),
    responses(
        (status = OK, description = "Workshop assets returned successfully", body = WorkshopResponse),
        (status = BAD_REQUEST, description = "Upstream request failed",      body = ErrorResponse),
    ),
    tag = "workshop"
)]
pub async fn get_workshop(
    Query(params): Query<WorkshopQuery>,
) -> Result<AppJson<WorkshopResponse>, ArsaError> {
    let client = Client::new();

    // Build query string manually so we can repeat `tags`
    let mut qs: Vec<(String, String)> = Vec::new();

    if let Some(ref s) = params.search {
        qs.push(("search".into(), s.clone()));
    }

    for tag in &params.tags {
        qs.push(("tags".into(), tag.clone()));
    }

    let sort_str = match params.sort {
        Sort::Newest => "newest",
        Sort::Popularity => "popularity",
        Sort::Subscribers => "subscribers",
        Sort::VersionSize => "version_size",
    };
    qs.push(("sort".into(), sort_str.into()));
    qs.push(("page".into(), params.page.to_string()));

    let build_id = cached_build_id(&client).await?;
    let mut response = client
        .get(upstream_workshop_json_url(&build_id))
        .query(&qs)
        .send()
        .await?;

    // A stale cached build id (Bohemia redeployed) 404s here — refresh once and retry.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let build_id = refresh_build_id(&client).await?;
        response = client
            .get(upstream_workshop_json_url(&build_id))
            .query(&qs)
            .send()
            .await?;
    }

    if !response.status().is_success() {
        return Err(ArsaError::UnknownError(format!(
            "Upstream returned status {}",
            response.status()
        )));
    }

    // The upstream wraps data in { pageProps: { assets: { count, rows } } }
    let raw: serde_json::Value = response.json().await?;

    let assets: AssetsPage = serde_json::from_value(raw["pageProps"]["assets"].clone())?;

    Ok(AppJson(WorkshopResponse { assets }))
}

/// Fetches the upstream asset detail page (`/workshop/{id}-asset.json`) and returns its
/// `pageProps`. Asset IDs are upper-cased because upstream lookups are case-sensitive.
async fn fetch_asset_page_props(id: &str) -> Result<serde_json::Value, ArsaError> {
    let client = Client::new();
    let slug = format!("{}-asset", id.to_ascii_uppercase());

    let build_id = cached_build_id(&client).await?;
    let mut response = client
        .get(format!("{}/{slug}.json", upstream_detail_base(&build_id)))
        .query(&[("id", &slug)])
        .send()
        .await?;

    // A stale cached build id (Bohemia redeployed) 404s here too — refresh once and retry
    // before treating it as a genuine "asset not found".
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let build_id = refresh_build_id(&client).await?;
        response = client
            .get(format!("{}/{slug}.json", upstream_detail_base(&build_id)))
            .query(&[("id", &slug)])
            .send()
            .await?;
    }

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ArsaError::NotFound);
    }

    if !response.status().is_success() {
        return Err(ArsaError::UnknownError(format!(
            "Upstream returned status {}",
            response.status()
        )));
    }

    let mut raw: serde_json::Value = response.json().await?;
    Ok(raw["pageProps"].take())
}

/// Get full details for a single workshop asset, including its changelog
///
/// The `id` path parameter is the asset ID (e.g. `5965550F24A0C152`).
/// The upstream slug format `{id}-{name}` is constructed automatically — just
/// pass the bare hex ID.
#[utoipa::path(
    get,
    path = "/workshop/{id}",
    params(
        ("id" = String, Path, description = "Asset ID (hex), e.g. 5965550F24A0C152", example = "5965550F24A0C152")
    ),
    responses(
        (status = OK, description = "Asset detail returned successfully",           body = AssetDetailResponse),
        (status = NOT_FOUND, description = "Asset not found",                       body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Upstream request failed",   body = ErrorResponse),
    ),
    tag = "workshop"
)]
pub async fn get_workshop_detail(
    Path(id): Path<String>,
) -> Result<AppJson<AssetDetailResponse>, ArsaError> {
    let props = fetch_asset_page_props(&id).await?;

    let asset: AssetDetail = serde_json::from_value(props["asset"].clone())?;

    let version_detail: AssetVersionDetail =
        serde_json::from_value(props["assetVersionDetail"].clone())?;

    let download_total = props["getAssetDownloadTotal"]["total"]
        .as_u64()
        .unwrap_or(0);

    Ok(AppJson(AssetDetailResponse {
        asset,
        version_detail,
        download_total,
    }))
}

/// List scenarios and dependencies for a workshop asset
///
/// Read from the upstream asset detail page (`/workshop/{id}-asset.json`).
/// Returns the scenarios bundled with the asset's current version, plus the
/// full resolved dependency list.
#[utoipa::path(
    get,
    path = "/workshop/{id}/scenarios",
    params(
        ("id" = String, Path, description = "Asset ID (hex), e.g. CAFEBEEFF0CACC1A", example = "CAFEBEEFF0CACC1A")
    ),
    responses(
        (status = OK, description = "Scenarios returned successfully", body = AssetScenariosResponse),
        (status = NOT_FOUND, description = "Asset not found", body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Upstream request failed", body = ErrorResponse),
    ),
    tag = "workshop"
)]
pub async fn get_workshop_scenarios(
    Path(id): Path<String>,
) -> Result<AppJson<AssetScenariosResponse>, ArsaError> {
    // Uses the asset detail page rather than upstream's `/workshop/{id}/scenarios.json`:
    // that page answers 404 `{"notFound":true}` for every asset without scenarios
    // (libraries, effect mods, ...), which is indistinguishable from a missing asset.
    // The detail page carries the same `assetVersionDetail.scenarios`/`dependencies`.
    let props = fetch_asset_page_props(&id).await?;

    let version_detail: AssetVersionDetail =
        serde_json::from_value(props["assetVersionDetail"].clone())?;

    Ok(AppJson(AssetScenariosResponse {
        asset_id: id.to_ascii_uppercase(),
        scenarios: version_detail.scenarios,
        dependencies: version_detail.dependencies,
    }))
}
