use common_rs::twilio::{CreateConversationRequest, CreateRoomRequest, TwilioConfig};

#[test]
fn test_twilio_config_basic_auth() {
    let config = TwilioConfig {
        base_url: "example.com".to_string(),
        account_sid: "AC123".to_string(),
        api_key_sid: "SK123".to_string(),
        api_key_secret: "secret123".to_string(),
        auth_token: "token123".to_string(),
        callback_url: "https://example.com/callback".to_string(),
        video_base_url: "example.com".to_string(),
        chat_base_url: "example.com".to_string(),
    };
    assert_eq!(config.basic_auth(), "AC123:token123");
}

#[test]
fn test_create_room_request() {
    let request = CreateRoomRequest::new(
        "test-room".to_string(),
        "https://example.com/callback".to_string(),
        true,
        "peer-to-peer".to_string(),
    );
    assert_eq!(request.unique_name, "test-room");
    assert_eq!(request.room_type, "peer-to-peer");
    assert_eq!(request.record_participants_on_connect, true);
    assert_eq!(request.audio_only, false);
}

#[test]
fn test_create_room_request_voice() {
    let request = CreateRoomRequest::voice(
        "voice-room".to_string(),
        "https://example.com/callback".to_string(),
        false,
    );
    assert_eq!(request.unique_name, "voice-room");
    assert_eq!(request.room_type, "group");
    assert_eq!(request.record_participants_on_connect, false);
    assert_eq!(request.audio_only, true);
}

#[test]
fn test_create_conversation_request() {
    let request = CreateConversationRequest::new("test-conversation".to_string());
    assert_eq!(request.unique_name, "test-conversation");
    assert_eq!(request.timers_closed, Some("PT1H".to_string()));
}
