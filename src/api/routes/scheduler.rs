use actix_web::web;

/// Register scheduler routes on an actix-web ServiceConfig.
pub fn configure(_cfg: &mut web::ServiceConfig) {
    // Scheduler endpoints are managed through the sync module.
    // Additional scheduler management endpoints may be added here.
}
