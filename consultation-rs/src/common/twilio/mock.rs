use crate::common::twilio::{TwilioClient, TwilioError};
#[cfg(test)]
use common_rs::twilio::{CreateRoomResponse, RoomLinks};

#[cfg(test)]
#[derive(Clone)]
pub struct MockTwilioClient;

#[cfg(test)]
#[async_trait::async_trait]
impl TwilioClient for MockTwilioClient {
    async fn create_video_room(
        &self,
        name: String,
        _record: bool,
    ) -> Result<CreateRoomResponse, TwilioError> {
        Ok(CreateRoomResponse {
            unique_name: name.clone(),
            date_updated: "2024-02-11T00:00:00Z".to_string(),
            media_region: Some("us1".to_string()),
            max_participant_duration: 14400,
            duration: Some(3600),
            video_codecs: vec!["VP8".to_string(), "H264".to_string()],
            large_room: false,
            enable_turn: true,
            empty_room_timeout: 60,
            sid: format!("RM{}", name.chars().skip(17).collect::<String>()),
            room_type: "peer-to-peer".to_string(),
            status_callback_method: "POST".to_string(),
            status: "in-progress".to_string(),
            audio_only: false,
            unused_room_timeout: 60,
            max_participants: 50,
            max_concurrent_published_tracks: 10,
            url: format!("https://video.twilio.com/v2/Rooms/{}", name),
            record_participants_on_connect: false,
            account_sid: "ACtestaccountsid".to_string(),
            end_time: None,
            date_created: "2024-02-11T00:00:00Z".to_string(),
            status_callback: format!("https://example.com/twilio/callback/{}", name),
            links: RoomLinks {
                participants: format!("https://video.twilio.com/v2/Rooms/{}/Participants", name),
                recordings: format!("https://video.twilio.com/v2/Rooms/{}/Recordings", name),
                recording_rules: format!(
                    "https://video.twilio.com/v2/Rooms/{}/RecordingRules",
                    name
                ),
            },
        })
    }

    async fn fetch_room(&self, name: String) -> Result<CreateRoomResponse, TwilioError> {
        Ok(CreateRoomResponse {
            unique_name: name.clone(),
            date_updated: "2024-02-11T00:00:00Z".to_string(),
            media_region: Some("us1".to_string()),
            max_participant_duration: 14400,
            duration: Some(3600),
            video_codecs: vec!["VP8".to_string(), "H264".to_string()],
            large_room: false,
            enable_turn: true,
            empty_room_timeout: 60,
            sid: format!("RM{}", name.chars().skip(17).collect::<String>()),
            room_type: "peer-to-peer".to_string(),
            status_callback_method: "POST".to_string(),
            status: "in-progress".to_string(),
            audio_only: false,
            unused_room_timeout: 60,
            max_participants: 50,
            max_concurrent_published_tracks: 10,
            url: format!("https://video.twilio.com/v2/Rooms/{}", name),
            record_participants_on_connect: false,
            account_sid: "ACtestaccountsid".to_string(),
            end_time: None,
            date_created: "2024-02-11T00:00:00Z".to_string(),
            status_callback: format!("https://example.com/twilio/callback/{}", name),
            links: RoomLinks {
                participants: format!("https://video.twilio.com/v2/Rooms/{}/Participants", name),
                recordings: format!("https://video.twilio.com/v2/Rooms/{}/Recordings", name),
                recording_rules: format!(
                    "https://video.twilio.com/v2/Rooms/{}/RecordingRules",
                    name
                ),
            },
        })
    }

    async fn complete_room(&self, _room_sid: String) -> Result<(), TwilioError> {
        Ok(())
    }

    async fn create_voice_room(
        &self,
        name: String,
        _record: bool,
    ) -> Result<CreateRoomResponse, TwilioError> {
        Ok(CreateRoomResponse {
            unique_name: name.clone(),
            date_updated: "2024-02-11T00:00:00Z".to_string(),
            media_region: Some("us1".to_string()),
            max_participant_duration: 14400,
            duration: Some(3600),
            video_codecs: vec!["VP8".to_string()],
            large_room: false,
            enable_turn: true,
            empty_room_timeout: 60,
            sid: format!("VM{}", name.chars().skip(17).collect::<String>()),
            room_type: "group".to_string(),
            status_callback_method: "POST".to_string(),
            status: "in-progress".to_string(),
            audio_only: true,
            unused_room_timeout: 60,
            max_participants: 50,
            max_concurrent_published_tracks: 1,
            url: format!("https://video.twilio.com/v2/Rooms/{}", name),
            record_participants_on_connect: false,
            account_sid: "ACtestaccountsid".to_string(),
            end_time: None,
            date_created: "2024-02-11T00:00:00Z".to_string(),
            status_callback: format!("https://example.com/twilio/callback/{}", name),
            links: RoomLinks {
                participants: format!("https://video.twilio.com/v2/Rooms/{}/Participants", name),
                recordings: format!("https://video.twilio.com/v2/Rooms/{}/Recordings", name),
                recording_rules: format!(
                    "https://video.twilio.com/v2/Rooms/{}/RecordingRules",
                    name
                ),
            },
        })
    }

    async fn create_conversation(
        &self,
        name: String,
    ) -> Result<common_rs::twilio::CreateConversationResponse, TwilioError> {
        let sid = format!("CH{}", name.chars().skip(17).collect::<String>());
        Ok(common_rs::twilio::CreateConversationResponse {
            sid: sid.clone(),
            account_sid: "ACtestaccountsid".to_string(),
            chat_service_sid: "IStestserviceid".to_string(),
            messaging_service_sid: "MGtestsid".to_string(),
            friendly_name: Some(name.clone()),
            unique_name: name,
            attributes: "{}".to_string(),
            date_created: "2024-02-11T00:00:00Z".to_string(),
            date_updated: "2024-02-11T00:00:00Z".to_string(),
            state: "active".to_string(),
            timers: None,
            bindings: None,
            url: format!("https://chat.twilio.com/v2/Services/IStestserviceid/Conversations/{}", sid),
            links: common_rs::twilio::ConversationLinks {
                participants: "https://chat.twilio.com/v2/Services/IStestserviceid/Conversations/CHtest/Participants".to_string(),
                messages: "https://chat.twilio.com/v2/Services/IStestserviceid/Conversations/CHtest/Messages".to_string(),
                webhooks: "https://chat.twilio.com/v2/Services/IStestserviceid/Conversations/CHtest/Webhooks".to_string(),
            },
        })
    }

    async fn join_conversation(
        &self,
        _conversation_sid: String,
        _identity: String,
    ) -> Result<(), TwilioError> {
        Ok(())
    }

    async fn close_conversation(&self, _conversation_sid: String) -> Result<(), TwilioError> {
        Ok(())
    }

    async fn create_access_token(
        &self,
        room_name: String,
        _chat_service_sid: Option<String>,
        _identity: String,
        _expires_at: Option<i64>,
    ) -> Result<String, common_rs::twilio::JwtError> {
        // Return a mock JWT token
        // In real implementation, this would be signed with Twilio credentials
        Ok(format!("mock_jwt_token_{}", room_name))
    }
}
