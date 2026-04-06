use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
pub struct UserEvent {
    pub event_type: String,
    pub user_id: i32,
    pub email: String,
}

pub fn create_producer(broker: &str) -> FutureProducer {
    ClientConfig::new()
        .set("bootstrap.servers", broker)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create Kafka producer")
}

pub async fn publish_user_event(
    producer: &FutureProducer,
    topic: &str,
    event: &UserEvent,
) -> Result<(), String> {
    let payload =
        serde_json::to_string(event).map_err(|e| format!("Serialization error: {}", e))?;

    let key = event.user_id.to_string();
    let record = FutureRecord::to(topic).key(&key).payload(&payload);

    producer
        .send(record, Duration::from_secs(5))
        .await
        .map_err(|(err, _)| format!("Kafka send error: {}", err))?;

    println!(
        "Published event: {} for user {}",
        event.event_type, event.email
    );
    Ok(())
}
