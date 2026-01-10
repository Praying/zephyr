//! Tenant management handlers.
//!
//! This module provides API handlers for multi-tenant management including:
//! - Tenant CRUD operations
//! - Tenant configuration management
//! - Resource quota management

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::{ApiResponse, CreatedResponse, EmptyResponse, PaginatedResponse};
use crate::state::AppState;

use zephyr_security::tenant::{ResourceQuota, Tenant, TenantConfig, TenantManager, TenantStatus};

/// Tenant list item.
#[derive(Debug, Serialize)]
pub struct TenantListItem {
    /// Tenant ID
    pub id: String,
    /// Tenant name
    pub name: String,
    /// Tenant slug
    pub slug: String,
    /// Tenant status
    pub status: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<&Tenant> for TenantListItem {
    fn from(tenant: &Tenant) -> Self {
        Self {
            id: tenant.id().to_string(),
            name: tenant.name().to_string(),
            slug: tenant.slug().to_string(),
            status: tenant.status().to_string(),
            created_at: tenant.created_at(),
            updated_at: tenant.updated_at(),
        }
    }
}

/// Tenant detail response.
#[derive(Debug, Serialize)]
pub struct TenantDetail {
    /// Tenant ID
    pub id: String,
    /// Tenant name
    pub name: String,
    /// Tenant slug
    pub slug: String,
    /// Tenant status
    pub status: String,
    /// Resource quota
    pub quota: QuotaResponse,
    /// Current usage
    pub usage: UsageResponse,
    /// Metadata
    pub metadata: MetadataResponse,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<&Tenant> for TenantDetail {
    fn from(tenant: &Tenant) -> Self {
        Self {
            id: tenant.id().to_string(),
            name: tenant.name().to_string(),
            slug: tenant.slug().to_string(),
            status: tenant.status().to_string(),
            quota: QuotaResponse::from(tenant.quota()),
            usage: UsageResponse::from(tenant.usage()),
            metadata: MetadataResponse::from(tenant.metadata()),
            created_at: tenant.created_at(),
            updated_at: tenant.updated_at(),
        }
    }
}

/// Resource quota response.
#[derive(Debug, Serialize)]
pub struct QuotaResponse {
    /// Maximum strategies
    pub max_strategies: u64,
    /// Maximum positions
    pub max_positions: u64,
    /// Maximum orders per second
    pub max_orders_per_second: u64,
    /// Maximum API requests per minute
    pub max_api_requests_per_minute: u64,
    /// Maximum WebSocket connections
    pub max_websocket_connections: u64,
    /// Maximum storage bytes
    pub max_storage_bytes: u64,
    /// Maximum exchange connections
    pub max_exchange_connections: u64,
    /// Maximum users
    pub max_users: u64,
}

impl From<&ResourceQuota> for QuotaResponse {
    fn from(quota: &ResourceQuota) -> Self {
        Self {
            max_strategies: quota.max_strategies,
            max_positions: quota.max_positions,
            max_orders_per_second: quota.max_orders_per_second,
            max_api_requests_per_minute: quota.max_api_requests_per_minute,
            max_websocket_connections: quota.max_websocket_connections,
            max_storage_bytes: quota.max_storage_bytes,
            max_exchange_connections: quota.max_exchange_connections,
            max_users: quota.max_users,
        }
    }
}

/// Resource usage response.
#[derive(Debug, Serialize)]
pub struct UsageResponse {
    /// Current strategies
    pub strategies: u64,
    /// Current positions
    pub positions: u64,
    /// Current orders per second
    pub orders_per_second: u64,
    /// Current API requests per minute
    pub api_requests_per_minute: u64,
    /// Current WebSocket connections
    pub websocket_connections: u64,
    /// Current storage bytes
    pub storage_bytes: u64,
    /// Current exchange connections
    pub exchange_connections: u64,
    /// Current users
    pub users: u64,
}

impl From<&zephyr_security::tenant::QuotaUsage> for UsageResponse {
    fn from(usage: &zephyr_security::tenant::QuotaUsage) -> Self {
        Self {
            strategies: usage.strategies,
            positions: usage.positions,
            orders_per_second: usage.orders_per_second,
            api_requests_per_minute: usage.api_requests_per_minute,
            websocket_connections: usage.websocket_connections,
            storage_bytes: usage.storage_bytes,
            exchange_connections: usage.exchange_connections,
            users: usage.users,
        }
    }
}

/// Metadata response.
#[derive(Debug, Serialize)]
pub struct MetadataResponse {
    /// Contact email
    pub contact_email: Option<String>,
    /// Company name
    pub company_name: Option<String>,
    /// Custom fields
    pub custom_fields: std::collections::HashMap<String, String>,
}

impl From<&zephyr_security::tenant::TenantMetadata> for MetadataResponse {
    fn from(metadata: &zephyr_security::tenant::TenantMetadata) -> Self {
        Self {
            contact_email: metadata.contact_email.clone(),
            company_name: metadata.company_name.clone(),
            custom_fields: metadata.custom_fields.clone(),
        }
    }
}

/// Create tenant request.
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    /// Tenant name
    pub name: String,
    /// Tenant slug (URL-safe identifier)
    pub slug: String,
    /// Contact email
    pub contact_email: Option<String>,
    /// Company name
    pub company_name: Option<String>,
    /// Quota tier (free, professional, enterprise)
    #[serde(default = "default_quota_tier")]
    pub quota_tier: String,
    /// Custom quota (overrides tier)
    pub custom_quota: Option<QuotaRequest>,
}

fn default_quota_tier() -> String {
    "free".to_string()
}

/// Quota request for custom quotas.
#[derive(Debug, Deserialize)]
pub struct QuotaRequest {
    /// Maximum strategies
    pub max_strategies: Option<u64>,
    /// Maximum positions
    pub max_positions: Option<u64>,
    /// Maximum orders per second
    pub max_orders_per_second: Option<u64>,
    /// Maximum API requests per minute
    pub max_api_requests_per_minute: Option<u64>,
}

/// Update tenant request.
#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    /// Tenant name
    pub name: Option<String>,
    /// Contact email
    pub contact_email: Option<String>,
    /// Company name
    pub company_name: Option<String>,
}

/// Update quota request.
#[derive(Debug, Deserialize)]
pub struct UpdateQuotaRequest {
    /// Quota tier (free, professional, enterprise)
    pub quota_tier: Option<String>,
    /// Custom quota values
    pub custom_quota: Option<QuotaRequest>,
}

/// List tenants query parameters.
#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    /// Filter by status
    pub status: Option<String>,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// List all tenants.
///
/// GET /api/v1/admin/tenants
pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Query(query): Query<ListTenantsQuery>,
) -> ApiResult<PaginatedResponse<TenantListItem>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();

    // Get all tenants
    let all_tenants = if let Some(status_str) = &query.status {
        let status = parse_tenant_status(status_str)?;
        tenant_manager.list_tenants_by_status(status)
    } else {
        tenant_manager.list_tenants()
    };

    let total = all_tenants.len() as u64;

    // Paginate
    let start = ((query.page - 1) * query.per_page) as usize;
    let tenants: Vec<TenantListItem> = all_tenants
        .iter()
        .skip(start)
        .take(query.per_page as usize)
        .map(|t| TenantListItem::from(t.as_ref()))
        .collect();

    Ok(PaginatedResponse::new(
        tenants,
        query.page,
        query.per_page,
        total,
    ))
}

/// Create a new tenant.
///
/// POST /api/v1/admin/tenants
pub async fn create_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Json(request): Json<CreateTenantRequest>,
) -> ApiResult<CreatedResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    // Validate request
    if request.name.is_empty() {
        return Err(ApiError::BadRequest("Tenant name is required".to_string()));
    }

    if request.slug.is_empty() {
        return Err(ApiError::BadRequest("Tenant slug is required".to_string()));
    }

    // Determine quota
    let quota = if let Some(custom) = request.custom_quota {
        let mut q = ResourceQuota::free_tier();
        if let Some(v) = custom.max_strategies {
            q.max_strategies = v;
        }
        if let Some(v) = custom.max_positions {
            q.max_positions = v;
        }
        if let Some(v) = custom.max_orders_per_second {
            q.max_orders_per_second = v;
        }
        if let Some(v) = custom.max_api_requests_per_minute {
            q.max_api_requests_per_minute = v;
        }
        q
    } else {
        match request.quota_tier.as_str() {
            "professional" => ResourceQuota::professional_tier(),
            "enterprise" => ResourceQuota::enterprise_tier(),
            "unlimited" => ResourceQuota::unlimited(),
            _ => ResourceQuota::free_tier(),
        }
    };

    // Build config
    let mut config = TenantConfig::new(&request.name, &request.slug, quota);
    config.contact_email = request.contact_email;
    config.company_name = request.company_name;

    // Create tenant
    let tenant_manager = state.tenant_manager();
    let tenant = tenant_manager
        .create_tenant(config)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(CreatedResponse::with_message(
        detail,
        "Tenant created successfully",
    ))
}

/// Get tenant by ID.
///
/// GET /api/v1/admin/tenants/{id}
pub async fn get_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<ApiResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    let tenant = tenant_manager
        .get_tenant(&tenant_id)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(ApiResponse::success(detail))
}

/// Update tenant.
///
/// PATCH /api/v1/admin/tenants/{id}
pub async fn update_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
    Json(request): Json<UpdateTenantRequest>,
) -> ApiResult<ApiResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    let tenant = tenant_manager
        .update_tenant(&tenant_id, |t| {
            if let Some(name) = &request.name {
                t.set_name(name);
            }
            // Note: metadata updates would require additional methods on Tenant
        })
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(ApiResponse::success_with_message(
        detail,
        "Tenant updated successfully",
    ))
}

/// Delete tenant.
///
/// DELETE /api/v1/admin/tenants/{id}
pub async fn delete_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<EmptyResponse> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    tenant_manager
        .delete_tenant(&tenant_id)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    Ok(EmptyResponse::success_with_message(
        "Tenant deleted successfully",
    ))
}

/// Activate tenant.
///
/// POST /api/v1/admin/tenants/{id}/activate
pub async fn activate_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<ApiResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    let tenant = tenant_manager
        .activate_tenant(&tenant_id)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(ApiResponse::success_with_message(
        detail,
        "Tenant activated successfully",
    ))
}

/// Suspend tenant.
///
/// POST /api/v1/admin/tenants/{id}/suspend
pub async fn suspend_tenant(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<ApiResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    let tenant = tenant_manager
        .suspend_tenant(&tenant_id)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(ApiResponse::success_with_message(
        detail,
        "Tenant suspended successfully",
    ))
}

/// Update tenant quota.
///
/// PUT /api/v1/admin/tenants/{id}/quota
pub async fn update_tenant_quota(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
    Json(request): Json<UpdateQuotaRequest>,
) -> ApiResult<ApiResponse<TenantDetail>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();
    let tenant_id = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID format".to_string()))?;

    // Get current tenant to get base quota
    let current = tenant_manager
        .get_tenant(&tenant_id)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    // Determine new quota
    let mut quota = if let Some(tier) = &request.quota_tier {
        match tier.as_str() {
            "professional" => ResourceQuota::professional_tier(),
            "enterprise" => ResourceQuota::enterprise_tier(),
            "unlimited" => ResourceQuota::unlimited(),
            "free" => ResourceQuota::free_tier(),
            _ => current.quota().clone(),
        }
    } else {
        current.quota().clone()
    };

    // Apply custom overrides
    if let Some(custom) = request.custom_quota {
        if let Some(v) = custom.max_strategies {
            quota.max_strategies = v;
        }
        if let Some(v) = custom.max_positions {
            quota.max_positions = v;
        }
        if let Some(v) = custom.max_orders_per_second {
            quota.max_orders_per_second = v;
        }
        if let Some(v) = custom.max_api_requests_per_minute {
            quota.max_api_requests_per_minute = v;
        }
    }

    let tenant = tenant_manager
        .update_quota(&tenant_id, quota)
        .await
        .map_err(|_| ApiError::NotFound(format!("Tenant not found: {id}")))?;

    let detail = TenantDetail::from(tenant.as_ref());

    Ok(ApiResponse::success_with_message(
        detail,
        "Tenant quota updated successfully",
    ))
}

/// Get tenant statistics.
///
/// GET /api/v1/admin/tenants/stats
pub async fn tenant_stats(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<TenantStatsResponse>> {
    // Check admin permission
    if !user.role.is_admin() {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }

    let tenant_manager = state.tenant_manager();

    let stats = TenantStatsResponse {
        total_tenants: tenant_manager.tenant_count() as u64,
        active_tenants: tenant_manager.active_tenant_count() as u64,
        pending_tenants: tenant_manager
            .list_tenants_by_status(TenantStatus::Pending)
            .len() as u64,
        suspended_tenants: tenant_manager
            .list_tenants_by_status(TenantStatus::Suspended)
            .len() as u64,
    };

    Ok(ApiResponse::success(stats))
}

/// Tenant statistics response.
#[derive(Debug, Serialize)]
pub struct TenantStatsResponse {
    /// Total number of tenants
    pub total_tenants: u64,
    /// Number of active tenants
    pub active_tenants: u64,
    /// Number of pending tenants
    pub pending_tenants: u64,
    /// Number of suspended tenants
    pub suspended_tenants: u64,
}

/// Parse tenant status from string.
fn parse_tenant_status(s: &str) -> ApiResult<TenantStatus> {
    match s.to_lowercase().as_str() {
        "active" => Ok(TenantStatus::Active),
        "pending" => Ok(TenantStatus::Pending),
        "suspended" => Ok(TenantStatus::Suspended),
        "deleting" => Ok(TenantStatus::Deleting),
        "deleted" => Ok(TenantStatus::Deleted),
        _ => Err(ApiError::BadRequest(format!("Invalid status: {s}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tenant_status() {
        assert_eq!(parse_tenant_status("active").unwrap(), TenantStatus::Active);
        assert_eq!(
            parse_tenant_status("pending").unwrap(),
            TenantStatus::Pending
        );
        assert_eq!(
            parse_tenant_status("suspended").unwrap(),
            TenantStatus::Suspended
        );
        assert!(parse_tenant_status("invalid").is_err());
    }

    #[test]
    fn test_tenant_list_item_from_tenant() {
        let quota = ResourceQuota::default();
        let tenant = Tenant::new("Test", "test", quota);
        let item = TenantListItem::from(&tenant);

        assert_eq!(item.name, "Test");
        assert_eq!(item.slug, "test");
    }

    #[test]
    fn test_quota_response_from_quota() {
        let quota = ResourceQuota::professional_tier();
        let response = QuotaResponse::from(&quota);

        assert_eq!(response.max_strategies, quota.max_strategies);
        assert_eq!(response.max_positions, quota.max_positions);
    }
}
