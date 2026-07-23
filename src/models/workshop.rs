use serde::Deserialize;
use serde::Serialize;
use utoipa::IntoParams;
use utoipa::PartialSchema;
use utoipa::ToSchema;

pub const UPSTREAM_HOST: &str = "https://reforger.armaplatform.com";

/// Bohemia's `/_next/data/{buildId}/...` routes are versioned by the site's
/// current Next.js build ID, which rotates on every deploy of
/// reforger.armaplatform.com — a hardcoded ID goes stale silently and every
/// proxied request starts 404ing. The ID is resolved at runtime instead; see
/// `endpoints::workshop::cached_build_id`.
pub fn upstream_workshop_json_url(build_id: &str) -> String {
    format!("{UPSTREAM_HOST}/_next/data/{build_id}/workshop.json")
}

pub fn upstream_detail_base(build_id: &str) -> String {
    format!("{UPSTREAM_HOST}/_next/data/{build_id}/workshop")
}

/// Valid workshop tags
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Tag {
    Vehicles,
    Weapons,
    Structures,
    Characters,
    Animals,
    Vegetation,
    Props,
    Compositions,
    ScenariosSp,
    ScenariosMP,
    Terrains,
    Systems,
    Effects,
    Misc,
}

/// Sort methods available on the upstream API
#[derive(Debug, Deserialize, Serialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Newest,
    Popularity,
    Subscribers,
    VersionSize,
}

/// Inline schema for the sort query parameter — avoids a dangling $ref to Sort
/// in components/schemas (IntoParams inlines params, it does not emit $refs).
fn sort_schema() -> utoipa::openapi::schema::Schema {
    use utoipa::openapi::schema::{ObjectBuilder, SchemaType};
    ObjectBuilder::new()
        .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
        .enum_values(Some([
            "newest",
            "popularity",
            "subscribers",
            "version_size",
        ]))
        .default(Some(serde_json::json!("newest")))
        .build()
        .into()
}

/// Query parameters forwarded to the Arma Reforger workshop
#[derive(Debug, Deserialize, IntoParams)]
pub struct WorkshopQuery {
    /// Free-text search string
    #[param(example = "My Mod")]
    pub search: Option<String>,

    /// One or more tags to filter by (repeat the parameter for multiple tags,
    /// e.g. `?tags=VEHICLES&tags=COMPOSITIONS`)
    #[serde(default)]
    pub tags: Vec<String>,

    /// Result sort order (default: newest)
    #[param(value_type = String, schema_with = sort_schema)]
    #[serde(default)]
    pub sort: Sort,

    /// Page number (1-based, default: 1)
    #[serde(default = "default_page")]
    pub page: u32,
}

fn default_page() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AssetTag {
    pub name: String,
    pub category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyTree {
    pub asset_id: String,
    pub version: String,
    pub game_version: String,
    pub platform_compatibility: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub summary: String,
    pub unlisted: bool,
    pub private: bool,
    pub blocked: bool,
    pub average_rating: f64,
    pub rating_count: u32,
    pub subscriber_count: u32,
    pub current_version_number: String,
    pub current_version_size: u64,
    pub previews: Vec<Preview>,
    pub tags: Vec<AssetTag>,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AssetsPage {
    pub count: u32,
    pub rows: Vec<Asset>,
}

/// Top-level response returned by this proxy
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkshopResponse {
    pub assets: AssetsPage,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetVersion {
    pub version: String,
    pub approved: bool,
    pub published: bool,
    pub game_version: String,
    pub total_file_size: u64,
    pub milestone: bool,
    pub created_at: String,
    pub updated_at: String,
    pub asset_id: String,
    pub scenarios_count: u32,
    pub dependencies_count: u32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ratings {
    pub likes: u32,
    pub dislikes: u32,
    pub rating: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetVersionDetail {
    /// Changelog text for the current version
    pub changelog: Option<String>,
    pub scenarios: Vec<Scenario>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTotal {
    pub total: u64,
}

/// Image with pre-generated thumbnail URLs
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioImage {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub content_type: String,
}

/// A playable scenario bundled with an asset
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub name: String,
    pub image: Option<ScenarioImage>,
    /// Enfusion game ID string, e.g. `{E2EC49F1…}Missions/…/Arland.conf`
    pub game_id: String,
    /// Upstream omits this for some scenarios — not guaranteed present.
    pub game_mode: Option<String>,
    /// Upstream omits this for some scenarios — not guaranteed present.
    pub author_name: Option<String>,
    /// Upstream omits this for some scenarios — not guaranteed present.
    pub description: Option<String>,
    pub player_count: u32,
}

/// Stub asset info embedded inside a dependency entry
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyAssetStub {
    pub id: String,
    pub name: String,
}

/// A resolved workshop dependency (asset + version metadata)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub version: String,
    pub blocked: bool,
    pub private: bool,
    pub published: bool,
    pub total_file_size: u64,
    pub asset: DependencyAssetStub,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

// utoipa 4 cannot derive ToSchema for recursive types — the macro recurses
// infinitely at schema-generation time. We manually implement both PartialSchema
// (which returns the schema shape) and ToSchema (which names it), using a $ref
// for the recursive `dependencies` field to break the cycle.
impl utoipa::PartialSchema for Dependency {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        use utoipa::openapi::{ArrayBuilder, RefOr, Schema, schema::ObjectBuilder};

        ObjectBuilder::new()
            .property("version", String::schema())
            .property("blocked", bool::schema())
            .property("private", bool::schema())
            .property("published", bool::schema())
            .property("totalFileSize", u64::schema())
            .property(
                "asset",
                RefOr::Ref(utoipa::openapi::Ref::from_schema_name(
                    "DependencyAssetStub",
                )),
            )
            // Self-referencing $ref — avoids infinite recursion at schema-build time
            .property(
                "dependencies",
                RefOr::T(Schema::Array(
                    ArrayBuilder::new()
                        .items(RefOr::Ref(utoipa::openapi::Ref::from_schema_name(
                            "Dependency",
                        )))
                        .build(),
                )),
            )
            .build()
            .into()
    }
}

impl utoipa::ToSchema for Dependency {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Dependency")
    }

    // Without this, utoipa never walks into DependencyAssetStub and it ends up
    // missing from #/components/schemas, causing "Ref not found" in generated clients.
    fn schemas(schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::Schema>)>) {
        schemas.push((
            DependencyAssetStub::name().into(),
            DependencyAssetStub::schema(),
        ));
        DependencyAssetStub::schemas(schemas);
    }
}

/// Response for the scenarios endpoint
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AssetScenariosResponse {
    pub asset_id: String,
    pub scenarios: Vec<Scenario>,
    pub dependencies: Vec<Dependency>,
}

/// Full asset detail including all versions, ratings, changelog, and download count
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetail {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub summary: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub unlisted: bool,
    pub private: bool,
    pub blocked: bool,
    pub average_rating: f64,
    pub rating_count: u32,
    pub subscriber_count: u32,
    pub current_version_number: String,
    pub current_version_size: u64,
    pub previews: Vec<Preview>,
    pub screenshots: Vec<Screenshot>,
    pub tags: Vec<AssetTag>,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
    pub game_version: String,
    pub obsolete: bool,
    pub versions: Vec<AssetVersion>,
    pub ratings: Ratings,
}

/// Response for the asset detail endpoint
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AssetDetailResponse {
    pub asset: AssetDetail,
    /// Changelog and dependency info for the current version
    pub version_detail: AssetVersionDetail,
    /// All-time download count
    pub download_total: u64,
}
