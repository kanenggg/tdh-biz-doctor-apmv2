use crate::provider_callback::service::ProviderCallbackService;

#[derive(Clone)]
pub struct AppState {
    pub service: ProviderCallbackService,
    pub twilio_auth_token: String,
    pub twilio_callback_url: String,
}
