use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct PubSubDeadLetterMessage {
    pub message: PubSubMessage,
    pub subscription: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubSubMessage {
    pub attributes: Option<PubSubDeadLetterAttributes>,

    pub data: String,

    // #[serde(alias = "messageId")]
    pub message_id: String,

    // #[serde(alias = "publishTime")]
    pub publish_time: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubSubDeadLetterAttributes {
    #[serde(rename = "CloudPubSubDeadLetterSourceDeliveryCount")]
    pub delivery_count: Option<String>,

    #[serde(rename = "CloudPubSubDeadLetterSourceSubscription")]
    pub source_subscription: Option<String>,

    #[serde(rename = "CloudPubSubDeadLetterSourceSubscriptionProject")]
    pub source_subscription_project: Option<String>,

    #[serde(rename = "CloudPubSubDeadLetterSourceTopicPublishTime")]
    pub source_topic_publish_time: Option<String>,
}
