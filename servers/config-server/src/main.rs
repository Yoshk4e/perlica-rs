use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    response::Json,
    routing::{get, post},
};
use common::logging::init_tracing;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct NetworkConfig {
    addr: String,
    port: u16,
    sdkenv: u8,
    channel: String,
    asset: String,
    eventlogid: String,
    netlogurl: String,
    netlogid: String,
    hglic: String,
    hgage: String,
    gatebltin: String,
    gamebltin: String,
    appver: String,
    srvclose: String,
    appudt: String,
    gameclose: String,
}

impl NetworkConfig {
    fn alpha() -> Self {
        Self {
            addr: "127.0.0.1".to_string(),
            port: 1337,
            sdkenv: 2,
            channel: "prod".to_string(),
            asset: "https://beyond.hg-cdn.com/asset/".to_string(),
            eventlogid: "event_log_id_xyz".to_string(),
            netlogurl: "http://127.0.0.1:3000/log".to_string(),
            netlogid: "net_log_id_xyz".to_string(),
            hglic: "some_license".to_string(),
            hgage: "18+".to_string(),
            gatebltin: "127.0.0.1:21041".to_string(),
            gamebltin: "127.0.0.1:21041".to_string(),
            appver: "0.1.4".to_string(),
            srvclose: String::new(),
            appudt: String::new(),
            gameclose: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct GameConfig {
    #[serde(rename = "mockLogin")]
    mock_login: bool,
    #[serde(rename = "selectSrv")]
    select_srv: bool,
    #[serde(rename = "enableHotUpdate")]
    enable_hot_update: bool,
    #[serde(rename = "enableEntitySpawnLog")]
    enable_entity_spawn_log: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            mock_login: true,
            select_srv: false,
            enable_hot_update: false,
            enable_entity_spawn_log: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct PackageInfo {
    packs: Vec<String>,
    total_size: String,
    file_path: String,
    url: String,
    md5: String,
    package_size: String,
    file_id: String,
    sub_channel: String,
}

impl Default for PackageInfo {
    fn default() -> Self {
        Self {
            packs: Vec::new(),
            total_size: "0".to_string(),
            file_path: "https://beyond.hg-cdn.com/uXUuLlNbIYmMMTlN/0.5/update/6/1/Windows/0.5.28_U1mgxrslUitdn3hb/files".to_string(),
            url: String::new(),
            md5: String::new(),
            package_size: "0".to_string(),
            file_id: "0".to_string(),
            sub_channel: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct LatestVersionData {
    action: u8,
    version: String,
    request_version: Option<String>,
    pkg: PackageInfo,
    patch: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccountToken {
    token: String,
}

impl Default for AccountToken {
    fn default() -> Self {
        Self {
            token: "yrn7R4g6IO0njm7VO96Uazwj".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TokenInfo {
    hgid: String,
    email: String,
    #[serde(rename = "realEmail")]
    real_email: String,
    #[serde(rename = "isLatestUserAgreement")]
    is_latest_user_agreement: bool,
    #[serde(rename = "nickName")]
    nick_name: String,
}

impl Default for TokenInfo {
    fn default() -> Self {
        Self {
            hgid: "1337".to_string(),
            email: "whatever*****@proton.com".to_string(),
            real_email: "whatever50503@proton.com".to_string(),
            is_latest_user_agreement: true,
            nick_name: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChannelTokenData {
    token: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "appId")]
    app_id: u8,
    #[serde(rename = "channelMasterId")]
    channel_master_id: u8,
}

impl Default for ChannelTokenData {
    fn default() -> Self {
        Self {
            token: "yrn7R4g6IO0njm7VO96Uazwj".to_string(),
            user_id: "1234567890".to_string(),
            app_id: 3,
            channel_master_id: 6,
        }
    }
}

#[derive(Debug, Serialize)]
struct VerifiedData {
    #[serde(rename = "userId")]
    user_id: String,
    token: String,
    #[serde(rename = "channelMasterId")]
    channel_master_id: u8,
    email: String,
    hgid: String,
    #[serde(rename = "typeReceived")]
    type_received: i64,
}

#[derive(Debug, Deserialize)]
struct VerifiedRequest {
    #[serde(rename = "type")]
    req_type: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    addr: String,
    port: u16,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "Perlica-rs".to_string(),
            addr: "0.0.0.0".to_string(),
            port: 1337,
        }
    }
}

#[derive(Debug, Serialize)]
struct ServerListResponse {
    servers: ServerInfo,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    data: T,
    msg: String,
    status: u8,
    #[serde(rename = "type")]
    response_type: String,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            data,
            msg: "OK".to_string(),
            status: 0,
            response_type: "A".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EmptyData {}

#[tokio::main]
async fn main() {
    init_tracing(tracing::Level::DEBUG);

    let app = Router::new()
        .route(
            "/api/remote_config/get_remote_config/1003/prod-cbt1oversea/default/default/network_config",
            get(network_config_alpha),
        )
        .route(
            "/api/game/get_latest",
            get(get_latest),
        )
        .route(
            "/api/remote_config/get_remote_config/1003/prod-cbt1oversea/default/Windows/game_config",
            get(game_config),
        )
        .route(
            "/app/v1/config",
            get(appcode_info)
        )
        .route(
            "/user/auth/v1/token_by_email_password",
            post(account_token),
        )
        .route(
            "/user/info/v1/basic",
            get(token_info),
        )
        .route(
            "/user/oauth2/v2/grant",
            post(verified),
        )
        .route(
            "/u8/user/auth/v2/token_by_channel_token",
            post(channel_token),
        )
        .route(
            "/u8/user/auth/v2/grant",
            post(is_verified),
        )
        .route(
            "/get_server_list",
            post(server_list),
        )
        .fallback(fallback);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:21041")
        .await
        .unwrap();

    tracing::debug!(
        "Config server listening on {}",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn network_config_alpha() -> Json<NetworkConfig> {
    Json(NetworkConfig::alpha())
}

async fn appcode_info(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let default_resp: &str = r#"{"data":{"agreementUrl":{"register":"https://user.gryphline.com/{language}/protocol/plain/terms_of_service","privacy":"https://user.gryphline.com/{language}/protocol/plain/privacy_policy","unbind":"https://user.gryphline.com/{language}/protocol/plain/endfield/privacy_policy","account":"https://user.gryphline.com/{language}/protocol/plain/terms_of_service","game":"https://user.gryphline.com/{language}/protocol/plain/endfield/privacy_policy"},"app":{"googleAndroidClientId":"","googleIosClientId":"","enableAutoLogin":true,"enablePayment":true,"enableGuestRegister":true,"needShowName":true,"displayName":{"en-us":"Arknights: Endfield","ja-jp":"アークナイツ：エンドフィールド","ko-kr":"명일방주：엔드필드","zh-cn":"明日方舟：终末地","zh-tw":"明日方舟：終末地"},"unbindAgreement":[],"unbindLimitedDays":30,"unbindCoolDownDays":14,"customerServiceUrl":"https://gryphline.helpshift.com/hc/{language}/4-arknights-endfield","enableUnbindGrant":false},"customerServiceUrl":"https://gryphline.helpshift.com/hc/{language}/4-arknights-endfield","thirdPartyRedirectUrl":"https://web-api.gryphline.com/callback/thirdPartyAuth.html","scanUrl":{"login":"yj://scan_login"},"loginChannels":[],"userCenterUrl":"https://user.gryphline.com/pcSdk/userInfo?language={language}"},"msg":"OK","status":0,"type":"A"}"#;

    let special_resp: &str = r#"{"data":{"antiAddiction":{"minorPeriodEnd":21,"minorPeriodStart":20},"payment":[{"key":"alipay","recommend":true},{"key":"wechat","recommend":false},{"key":"pcredit","recommend":false}],"customerServiceUrl":"https://chat.hypergryph.com/chat/h5/v2/index.html?sysnum=889ee281e3564ddf883942fe85764d44&channelid=2","cancelDeactivateUrl":"https://user-stable.hypergryph.com/cancellation","agreementUrl":{"game":"https://hg-protocol-static-web-stable.hypergryph.net/protocol/plain/ak/index","unbind":"https://hg-protocol-static-web-stable.hypergryph.net/protocol/plain/ak/cancellation","gameService":"https://hg-protocol-static-web-stable.hypergryph.net/protocol/plain/ak/service","account":"https://user.hypergryph.com/protocol/plain/index","privacy":"https://user.hypergryph.com/protocol/plain/privacy","register":"https://user.hypergryph.com/protocol/plain/registration","updateOverview":"https://user.hypergryph.com/protocol/plain/overview_of_changes","childrenPrivacy":"https://user.hypergryph.com/protocol/plain/children_privacy"},"app":{"enablePayment":true,"enableAutoLogin":true,"enableAuthenticate":true,"enableAntiAddiction":true,"enableUnbindGrant":true,"wechatAppId":"wxeea7cc50e03edb28","alipayAppId":"2021004129658342","oneLoginAppId":"496b284079be97612a46266a9fdbfbd7","enablePaidApp":false,"appName":"明日方舟终末地","appAmount":600,"needShowName":true,"customerServiceUrl":"https://web-biz-platform-cs-center-stable.hypergryph.net/hg/?hg_token={hg_token}&source_from=sdk","needAntiAddictionAlert":true,"enableScanLogin":false,"deviceCheckMode":0,"enableGiftCode":false},"scanUrl":{"login":"hypergryph://scan_login"},"userCenterUrl":"https://user-center-account-stable.hypergryph.net/pcSdk/userInfo"},"msg":"OK","status":0,"type":"A"}"#;

    match params.get("appCode") {
        None => (
            StatusCode::BAD_REQUEST,
            [("content-type", "text/plain")],
            "invalid appCode/platform",
        )
            .into_response(),
        Some(code) if code == "a65356244d22261b" => (
            StatusCode::OK,
            [("content-type", "application/json")],
            special_resp,
        )
            .into_response(),
        Some(_) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            default_resp,
        )
            .into_response(),
    }
}

async fn game_config() -> Json<GameConfig> {
    Json(GameConfig::default())
}

async fn get_latest(
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<LatestVersionData>> {
    let data = LatestVersionData {
        action: 0,
        version: "0.5.28".to_string(),
        request_version: params.get("version").cloned(),
        pkg: PackageInfo::default(),
        patch: None,
    };
    Json(ApiResponse::success(data))
}

async fn account_token() -> Json<AccountToken> {
    Json(AccountToken::default())
}

async fn token_info() -> Json<ApiResponse<TokenInfo>> {
    let data = TokenInfo::default();
    Json(ApiResponse::success(data))
}

async fn verified() -> Json<ApiResponse<EmptyData>> {
    Json(ApiResponse::success(EmptyData {}))
}

async fn channel_token() -> Json<ApiResponse<ChannelTokenData>> {
    Json(ApiResponse::success(ChannelTokenData::default()))
}

async fn is_verified(Json(body): Json<VerifiedRequest>) -> Json<ApiResponse<VerifiedData>> {
    let data = VerifiedData {
        user_id: "1234567890".to_string(),
        token: "yrn7R4g6IO0njm7VO96Uazwj".to_string(),
        channel_master_id: 6,
        email: "whatever50503@proton.com".to_string(),
        hgid: "1337".to_string(),
        type_received: body.req_type.unwrap_or(-1),
    };

    Json(ApiResponse::success(data))
}

async fn server_list() -> Json<ServerListResponse> {
    Json(ServerListResponse {
        servers: ServerInfo::default(),
    })
}

async fn fallback(req: axum::http::Request<axum::body::Body>) -> impl IntoResponse {
    tracing::debug!("no route matched: {} {}", req.method(), req.uri());
    (StatusCode::NOT_FOUND, "no route")
}
