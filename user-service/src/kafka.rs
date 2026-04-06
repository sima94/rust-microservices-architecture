use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use serde::Deserialize;
use tokio::sync::watch;

use crate::cache::{self, RedisPool};
use crate::db::{DbPool, DbPools};
use crate::models::NewUser;
use crate::repositories::user_repository::UserRepository;

#[derive(Deserialize)]
struct UserEvent {
    event_type: String,
    #[allow(dead_code)]
    user_id: i32,
    email: String,
}

pub fn create_consumer(broker: &str, group_id: &str) -> StreamConsumer {
    ClientConfig::new()
        .set("bootstrap.servers", broker)
        .set("group.id", group_id)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create Kafka consumer")
}

pub async fn start_consumer(
    consumer: StreamConsumer,
    pools: DbPools,
    redis: RedisPool,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let pool = &pools.write;
    consumer
        .subscribe(&["user.events"])
        .expect("Failed to subscribe to topic");

    println!("Kafka consumer started, listening on 'user.events'...");

    loop {
        tokio::select! {
            result = consumer.recv() => {
                match result {
                    Ok(msg) => {
                        if let Some(Ok(payload)) = msg.payload_view::<str>() {
                            handle_message(payload, pool, &redis).await;
                        }
                        if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                            eprintln!("Failed to commit offset: {}", e);
                        }
                    }
                    Err(e) => eprintln!("Kafka consumer error: {}", e),
                }
            }
            _ = shutdown_rx.changed() => {
                println!("Shutdown signal received, committing offsets...");
                if let Err(e) = consumer.commit_consumer_state(CommitMode::Sync) {
                    eprintln!("Failed to commit on shutdown: {}", e);
                }
                println!("Kafka consumer stopped gracefully.");
                break;
            }
        }
    }
}

async fn handle_message(payload: &str, pool: &DbPool, redis: &RedisPool) {
    let event: UserEvent = match serde_json::from_str(payload) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to parse event: {}", e);
            return;
        }
    };

    match event.event_type.as_str() {
        "user.registered" => {
            let name = event
                .email
                .split('@')
                .next()
                .unwrap_or("unknown")
                .to_string();
            let new_user = NewUser {
                name,
                email: event.email.clone(),
            };

            match UserRepository::create(pool, new_user).await {
                Ok(user) => {
                    println!("Auto-created user profile: {} ({})", user.name, user.email);
                    // Cache the new user + invalidate list
                    cache::set_cached(redis, &cache::user_cache_key(user.id), &user).await;
                    cache::invalidate(redis, &cache::users_list_cache_key()).await;
                }
                Err(e) => eprintln!("Failed to create user from event: {}", e),
            }
        }
        other => println!("Unknown event type: {}", other),
    }
}
