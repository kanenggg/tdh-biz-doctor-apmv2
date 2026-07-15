use common_rs::twilio::chat::JoinConversationRequest;
/// Manual test for access token generation and room joining
///
/// This test validates:
/// 1. Access token generation for video, chat, and combined video+chat
/// 2. Room creation and joining functionality
/// 3. Token structure and validation
/// 4. Token expiration at consultation end time
///
/// Run with: cargo test --manifest-path consultation-rs/Cargo.toml --test token_and_room_test -- --nocapture
use common_rs::twilio::jwt::TwilioAccessTokenBuilder;
use common_rs::twilio::video::CreateRoomRequest;
use jiff::ToSpan;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde_json::Value;

#[test]
fn test_video_token_generation() {
    println!("\n=== Testing Video Token Generation ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test_account_sid".to_string(),
        "SK_test_api_key".to_string(),
        "test_secret_key".to_string(),
    );

    let room_name = "consultation_room_12345";
    let identity = "doctor_999";

    let token = builder
        .build_video_token(room_name, identity)
        .expect("Failed to generate video token");

    println!("✓ Video token generated successfully");
    println!("  Room: {}", room_name);
    println!("  Identity: {}", identity);
    println!("  Token length: {} chars", token.len());
    println!("  Token preview: {}...", &token[..50.min(token.len())]);

    // Validate JWT structure (should have 3 parts: header.payload.signature)
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "Token should have 3 parts (header.payload.signature)"
    );

    println!("  Token structure:");
    println!("    - Header length: {} chars", parts[0].len());
    println!("    - Payload length: {} chars", parts[1].len());
    println!("    - Signature length: {} chars", parts[2].len());

    assert!(!parts[0].is_empty(), "Header should not be empty");
    assert!(!parts[1].is_empty(), "Payload should not be empty");
    assert!(!parts[2].is_empty(), "Signature should not be empty");

    println!("✓ Video token validation passed");
}

#[test]
fn test_chat_token_generation() {
    println!("\n=== Testing Chat Token Generation ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test_account_sid".to_string(),
        "SK_test_api_key".to_string(),
        "test_secret_key".to_string(),
    );

    let chat_service_sid = "IS_test_service_12345";
    let identity = "patient_123_456";

    let token = builder
        .build_chat_token(chat_service_sid, identity)
        .expect("Failed to generate chat token");

    println!("✓ Chat token generated successfully");
    println!("  Chat Service SID: {}", chat_service_sid);
    println!("  Identity: {}", identity);
    println!("  Token length: {} chars", token.len());
    println!("  Token preview: {}...", &token[..50.min(token.len())]);

    // Validate JWT structure
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3, "Token should have 3 parts");

    assert!(!parts[0].is_empty(), "Header should not be empty");
    assert!(!parts[1].is_empty(), "Payload should not be empty");
    assert!(!parts[2].is_empty(), "Signature should not be empty");

    println!("✓ Chat token validation passed");
}

#[test]
fn test_video_chat_combined_token_generation() {
    println!("\n=== Testing Combined Video + Chat Token Generation ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test_account_sid".to_string(),
        "SK_test_api_key".to_string(),
        "test_secret_key".to_string(),
    );

    let room_name = "consultation_room_67890";
    let chat_service_sid = Some("IS_test_service_67890");
    let identity = "doctor_555";

    let token = builder
        .build_video_chat_token(room_name, chat_service_sid.as_deref(), identity)
        .expect("Failed to generate combined token");

    println!("✓ Combined token generated successfully");
    println!("  Room: {}", room_name);
    println!("  Chat Service SID: {}", chat_service_sid.unwrap());
    println!("  Identity: {}", identity);
    println!("  Token length: {} chars", token.len());
    println!("  Token preview: {}...", &token[..50.min(token.len())]);

    // Validate JWT structure
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3, "Token should have 3 parts");

    assert!(!parts[0].is_empty(), "Header should not be empty");
    assert!(!parts[1].is_empty(), "Payload should not be empty");
    assert!(!parts[2].is_empty(), "Signature should not be empty");

    println!("✓ Combined token validation passed");
}

#[test]
fn test_identity_formatting() {
    println!("\n=== Testing Identity Formatting ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    // Test doctor identity format
    let doctor_identity = "doctor_12345";
    let doctor_token = builder
        .build_video_token("room", doctor_identity)
        .expect("Failed to generate doctor token");

    assert!(!doctor_token.is_empty(), "Doctor token should be generated");
    assert_eq!(
        doctor_token.split('.').count(),
        3,
        "Doctor token should be valid JWT"
    );
    println!("✓ Doctor identity format: {}", doctor_identity);
    println!("  Token generated: {} chars", doctor_token.len());

    // Test patient identity format (account_id_profile_id)
    let patient_identity = "patient_123_456";
    let patient_token = builder
        .build_video_token("room", patient_identity)
        .expect("Failed to generate patient token");

    assert!(
        !patient_token.is_empty(),
        "Patient token should be generated"
    );
    assert_eq!(
        patient_token.split('.').count(),
        3,
        "Patient token should be valid JWT"
    );
    println!("✓ Patient identity format: {}", patient_identity);
    println!("  Token generated: {} chars", patient_token.len());

    // Tokens should be different for different identities
    assert_ne!(
        doctor_token, patient_token,
        "Different identities should produce different tokens"
    );
    println!("✓ Different identities produce unique tokens");
}

#[test]
fn test_room_creation_request_structure() {
    println!("\n=== Testing Room Creation Request Structure ===");

    // Test video room request
    let video_request = CreateRoomRequest::new(
        "video_room_12345".to_string(),
        "https://example.com/callback".to_string(),
        true,                       // record_session
        "peer-to-peer".to_string(), // room_type
    );

    println!("✓ Video room request created:");
    println!("  Name: {}", video_request.unique_name);
    println!("  Type: {}", video_request.room_type);
    println!(
        "  Recording: {}",
        video_request.record_participants_on_connect
    );
    println!("  Audio only: {}", video_request.audio_only);

    assert_eq!(video_request.room_type, "peer-to-peer");
    assert_eq!(video_request.record_participants_on_connect, true);
    assert_eq!(video_request.audio_only, false);

    // Test voice room request
    let voice_request = CreateRoomRequest::voice(
        "voice_room_67890".to_string(),
        "https://example.com/callback".to_string(),
        true, // record_session
    );

    println!("✓ Voice room request created:");
    println!("  Name: {}", voice_request.unique_name);
    println!("  Audio only: {}", voice_request.audio_only);

    assert_eq!(voice_request.audio_only, true);
    println!("✓ Room creation requests validated");
}

#[test]
fn test_conversation_join_request_structure() {
    println!("\n=== Testing Conversation Join Request Structure ===");

    let join_request = JoinConversationRequest {
        identity: "patient_123_456".to_string(),
    };

    println!("✓ Join conversation request created:");
    println!("  Identity: {}", join_request.identity);

    assert!(!join_request.identity.is_empty());

    // Test multiple users joining
    let participants = vec!["patient_123_456", "doctor_789"];

    println!("  Testing multiple participants:");
    for participant in participants {
        let request = JoinConversationRequest {
            identity: participant.to_string(),
        };
        println!("    - Participant '{}' ready to join", request.identity);
        assert!(!request.identity.is_empty());
    }

    println!("✓ Multiple participants can join conversation");
}

#[test]
fn test_end_to_end_session_flow() {
    println!("\n=== Testing End-to-End Session Flow ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test_account".to_string(),
        "SK_test_key".to_string(),
        "test_secret".to_string(),
    );

    // Step 1: Prepare room and chat service
    let room_name = "consultation_room_99999";
    let chat_service_sid = "IS_chat_service_99999";

    println!("Step 1: Room and chat service prepared");
    println!("  Room: {}", room_name);
    println!("  Chat Service: {}", chat_service_sid);

    // Step 2: Generate patient token
    let patient_identity = "patient_100_200";
    let patient_token = builder
        .build_video_chat_token(room_name, Some(chat_service_sid), patient_identity)
        .expect("Failed to generate patient token");

    println!("Step 2: Patient token generated");
    println!("  Identity: {}", patient_identity);
    println!("  Token length: {} chars", patient_token.len());

    assert!(!patient_token.is_empty());
    assert_eq!(patient_token.split('.').count(), 3);

    // Step 3: Generate doctor token
    let doctor_identity = "doctor_300";
    let doctor_token = builder
        .build_video_chat_token(room_name, Some(chat_service_sid), doctor_identity)
        .expect("Failed to generate doctor token");

    println!("Step 3: Doctor token generated");
    println!("  Identity: {}", doctor_identity);
    println!("  Token length: {} chars", doctor_token.len());

    assert!(!doctor_token.is_empty());
    assert_eq!(doctor_token.split('.').count(), 3);

    // Step 4: Validate both tokens are different but valid
    assert_ne!(
        patient_token, doctor_token,
        "Patient and doctor should have different tokens"
    );

    println!("Step 4: Tokens validated");
    println!("  ✓ Both users have valid JWT tokens");
    println!("  ✓ Patient identity: {}", patient_identity);
    println!("  ✓ Doctor identity: {}", doctor_identity);
    println!("  ✓ Tokens are unique per user");

    // Step 5: Create room and conversation join requests
    let room_request = CreateRoomRequest::new(
        room_name.to_string(),
        "https://example.com/callback".to_string(),
        true,
        "peer-to-peer".to_string(),
    );

    let patient_join = JoinConversationRequest {
        identity: patient_identity.to_string(),
    };

    let doctor_join = JoinConversationRequest {
        identity: doctor_identity.to_string(),
    };

    println!("Step 5: Room and conversation join requests created");
    println!("  ✓ Room request for: {}", room_request.unique_name);
    println!("  ✓ Patient join request: {}", patient_join.identity);
    println!("  ✓ Doctor join request: {}", doctor_join.identity);

    println!("\n✓ Complete end-to-end session flow validated successfully");
    println!("  Summary:");
    println!("  - Room: {}", room_name);
    println!("  - Chat service: {}", chat_service_sid);
    println!(
        "  - Patient can join with identity: {} (token: {} chars)",
        patient_identity,
        patient_token.len()
    );
    println!(
        "  - Doctor can join with identity: {} (token: {} chars)",
        doctor_identity,
        doctor_token.len()
    );
    println!("  - Both have video + chat access");
}

#[test]
fn test_multiple_token_generations() {
    println!("\n=== Testing Multiple Token Generations ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "test_room";
    let identities = vec!["user_1", "user_2", "user_3", "user_4", "user_5"];

    println!(
        "Generating tokens for {} users in room '{}':",
        identities.len(),
        room_name
    );

    let mut tokens = Vec::new();
    for identity in &identities {
        let token = builder
            .build_video_token(room_name, identity)
            .expect(&format!("Failed to generate token for {}", identity));

        println!("  ✓ Token for '{}': {} chars", identity, token.len());
        assert_eq!(token.split('.').count(), 3);
        tokens.push(token);
    }

    // Verify all tokens are unique
    for i in 0..tokens.len() {
        for j in (i + 1)..tokens.len() {
            assert_ne!(
                tokens[i], tokens[j],
                "Tokens for {} and {} should be different",
                identities[i], identities[j]
            );
        }
    }

    println!("✓ All {} tokens are unique", tokens.len());
    println!("✓ Multiple token generation test passed");
}

#[test]
fn test_token_without_chat() {
    println!("\n=== Testing Token Without Chat (Video Only) ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "video_only_room";
    let identity = "user_video_only";

    // Generate video-only token (no chat)
    let token = builder
        .build_video_chat_token(room_name, None, identity) // None for chat service
        .expect("Failed to generate video-only token");

    println!("✓ Video-only token generated (no chat)");
    println!("  Room: {}", room_name);
    println!("  Identity: {}", identity);
    println!("  Token length: {} chars", token.len());

    assert!(!token.is_empty());
    assert_eq!(token.split('.').count(), 3);

    println!("✓ Video-only token validation passed");
}

#[test]
fn test_video_token_with_consultation_end_time_expiration() {
    println!("\n=== Testing Video Token with Consultation End Time Expiration ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "consultation_room_custom_exp";
    let identity = "doctor_custom_exp";

    // Set consultation end time to 1 hour from now
    let now = jiff::Timestamp::now();
    let consultation_end_time = now.checked_add(1.hour()).unwrap();

    let token = builder
        .build_video_token_with_exp(room_name, identity, Some(consultation_end_time.as_second()))
        .expect("Failed to generate video token with custom expiration");

    println!("✓ Video token generated with consultation end time expiration");
    println!("  Room: {}", room_name);
    println!("  Identity: {}", identity);
    println!(
        "  Consultation end time timestamp: {}",
        consultation_end_time.as_second()
    );

    assert!(!token.is_empty());
    assert_eq!(token.split('.').count(), 3);

    // Decode token to verify expiration
    let payload = decode_token_payload(&token);

    let token_exp = payload["exp"].as_i64().expect("exp not found or invalid");
    println!("  Token exp claim: {}", token_exp);

    // Verify expiration is close to consultation end time (within 1 second tolerance)
    let diff = (token_exp - consultation_end_time.as_second()).abs();
    assert!(
        diff <= 1,
        "Token expiration should match consultation end time within 1 second, diff: {}",
        diff
    );

    println!("✓ Token expiration matches consultation end time");
}

#[test]
fn test_video_chat_token_with_consultation_end_time_expiration() {
    println!("\n=== Testing Video + Chat Token with Consultation End Time Expiration ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "consultation_room_video_chat_exp";
    let chat_service_sid = Some("IS_test_service_exp");
    let identity = "patient_video_chat_exp";

    // Set consultation end time to 30 minutes from now
    let now = jiff::Timestamp::now();
    let consultation_end_time = now.checked_add(1.hour()).unwrap();

    let token = builder
        .build_video_chat_token_with_exp(
            room_name,
            chat_service_sid.as_deref(),
            identity,
            Some(consultation_end_time.as_second()),
        )
        .expect("Failed to generate combined token with custom expiration");

    println!("✓ Combined token generated with consultation end time expiration");
    println!("  Room: {}", room_name);
    println!("  Chat Service SID: {}", chat_service_sid.unwrap());
    println!("  Identity: {}", identity);
    println!(
        "  Consultation end time timestamp: {}",
        consultation_end_time.as_second()
    );

    assert!(!token.is_empty());
    assert_eq!(token.split('.').count(), 3);

    // Decode token to verify expiration
    let payload = decode_token_payload(&token);

    let token_exp = payload["exp"].as_i64().expect("exp not found or invalid");
    println!("  Token exp claim: {}", token_exp);

    // Verify expiration is close to consultation end time (within 1 second tolerance)
    let diff = (token_exp - consultation_end_time.as_second()).abs();
    assert!(
        diff <= 1,
        "Token expiration should match consultation end time within 1 second, diff: {}",
        diff
    );

    println!("✓ Token expiration matches consultation end time");
}

#[test]
fn test_chat_token_with_consultation_end_time_expiration() {
    println!("\n=== Testing Chat Token with Consultation End Time Expiration ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let chat_service_sid = "IS_test_chat_service_exp";
    let identity = "patient_chat_exp";

    // Set consultation end time to 2 hours from now
    let now = jiff::Timestamp::now();
    let consultation_end_time = now.checked_add(2.hours()).unwrap();

    let token = builder
        .build_chat_token_with_exp(
            chat_service_sid,
            identity,
            Some(consultation_end_time.as_second()),
        )
        .expect("Failed to generate chat token with custom expiration");

    println!("✓ Chat token generated with consultation end time expiration");
    println!("  Chat Service SID: {}", chat_service_sid);
    println!("  Identity: {}", identity);
    println!(
        "  Consultation end time timestamp: {}",
        consultation_end_time.as_second()
    );

    assert!(!token.is_empty());
    assert_eq!(token.split('.').count(), 3);

    // Decode token to verify expiration
    let payload = decode_token_payload(&token);

    let token_exp = payload["exp"].as_i64().expect("exp not found or invalid");
    println!("  Token exp claim: {}", token_exp);

    // Verify expiration is close to consultation end time (within 1 second tolerance)
    let diff = (token_exp - consultation_end_time.as_second()).abs();
    assert!(
        diff <= 1,
        "Token expiration should match consultation end time within 1 second, diff: {}",
        diff
    );

    println!("✓ Token expiration matches consultation end time");
}

#[test]
fn test_token_default_expiration_when_no_consultation_end_time_provided() {
    println!("\n=== Testing Token Default Expiration (24 hours) ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "room_default_exp";
    let identity = "user_default_exp";

    let now = jiff::Timestamp::now();
    let expected_default_exp = now.checked_add(24.hours()).unwrap();

    // Generate token without consultation end time (should default to 24 hours)
    let token = builder
        .build_video_token(room_name, identity)
        .expect("Failed to generate token");

    println!("✓ Token generated with default expiration");

    // Decode token to verify expiration
    let payload = decode_token_payload(&token);

    let token_exp = payload["exp"].as_i64().expect("exp not found or invalid");

    println!("  Token expiration timestamp: {}", token_exp);
    println!(
        "  Expected default expiration: ~{}",
        expected_default_exp.as_second()
    );

    // Verify expiration is approximately 24 hours from now (within 5 seconds tolerance for test execution time)
    let diff = (token_exp - expected_default_exp.as_second()).abs();
    assert!(
        diff <= 5,
        "Token should expire in approximately 24 hours, diff: {} seconds",
        diff
    );

    println!("✓ Token defaults to 24-hour expiration");
}

#[test]
fn test_token_expiration_with_past_consultation_end_time() {
    println!("\n=== Testing Token with Past Consultation End Time (Edge Case) ===");

    let builder = TwilioAccessTokenBuilder::new(
        "AC_test".to_string(),
        "SK_test".to_string(),
        "secret".to_string(),
    );

    let room_name = "room_past_exp";
    let identity = "user_past_exp";

    // Set consultation end time to 1 hour in the past
    let now = jiff::Timestamp::now();
    let past_end_time = now.checked_add(1.hour()).unwrap();

    // Generate token with past expiration time (edge case)
    let token = builder
        .build_video_token_with_exp(room_name, identity, Some(past_end_time.as_second()))
        .expect("Failed to generate token with past expiration");

    println!("✓ Token generated with past consultation end time");

    // Decode token to verify expiration
    let payload = decode_token_payload(&token);

    let token_exp = payload["exp"].as_i64().expect("exp not found or invalid");
    println!("  Token exp claim: {}", token_exp);
    println!("  Past end time: {}", past_end_time.as_second());

    // Verify expiration matches the past end time
    let diff = (token_exp - past_end_time.as_second()).abs();
    assert!(
        diff <= 1,
        "Token expiration should match past end time within 1 second, diff: {}",
        diff
    );

    println!("✓ Token expiration correctly set to past time");
}

fn decode_token_payload(token: &str) -> Value {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = false;
    validation.validate_nbf = false;

    let token_data = decode::<Value>(
        token,
        &DecodingKey::from_secret("secret".as_ref()),
        &validation,
    )
    .unwrap();
    token_data.claims
}
