use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    #[serde(rename = "RecordParticipantsOnConnect")]
    pub record_participants_on_connect: bool,
    #[serde(rename = "StatusCallback")]
    pub status_callback: String,
    #[serde(rename = "Type")]
    pub room_type: String,
    #[serde(rename = "UniqueName")]
    pub unique_name: String,
    #[serde(rename = "EmptyRoomTimeout", skip_serializing_if = "Option::is_none")]
    pub empty_room_timeout: Option<i32>,
    #[serde(rename = "UnusedRoomTimeout", skip_serializing_if = "Option::is_none")]
    pub unused_room_timeout: Option<i32>,
    #[serde(rename = "AudioOnly")]
    pub audio_only: bool,
}

impl CreateRoomRequest {
    pub fn new(
        room_name: String,
        callback_url: String,
        record_session: bool,
        room_type: String,
    ) -> Self {
        CreateRoomRequest {
            record_participants_on_connect: record_session,
            status_callback: callback_url,
            room_type,
            unique_name: room_name,
            empty_room_timeout: Some(60),
            unused_room_timeout: Some(60),
            audio_only: false,
        }
    }

    pub fn voice(room_name: String, callback_url: String, record_session: bool) -> Self {
        CreateRoomRequest {
            record_participants_on_connect: record_session,
            status_callback: callback_url,
            room_type: "group".to_string(),
            unique_name: room_name,
            empty_room_timeout: Some(60),
            unused_room_timeout: Some(60),
            audio_only: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomResponse {
    pub unique_name: String,
    pub date_updated: String,
    pub media_region: Option<String>,
    pub max_participant_duration: i32,
    pub duration: Option<i32>,
    pub video_codecs: Vec<String>,
    pub large_room: bool,
    pub enable_turn: bool,
    pub empty_room_timeout: i32,
    pub sid: String,
    #[serde(rename = "type")]
    pub room_type: String,
    pub status_callback_method: String,
    pub status: String,
    pub audio_only: bool,
    pub unused_room_timeout: i32,
    pub max_participants: i32,
    pub max_concurrent_published_tracks: i32,
    pub url: String,
    pub record_participants_on_connect: bool,
    pub account_sid: String,
    pub end_time: Option<String>,
    pub date_created: String,
    pub status_callback: String,
    pub links: RoomLinks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomLinks {
    pub participants: String,
    pub recordings: String,
    pub recording_rules: String,
}
