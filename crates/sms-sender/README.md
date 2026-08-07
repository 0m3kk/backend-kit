# sms-sender

Core traits, domain models, and in-memory provider for SMS sending in `backend-kit`.

## Overview

`sms-sender` defines the unified, async [`SmsSender`](src/lib.rs) trait and domain models (`SmsMessage`, `SmsError`). It enables pluggable SMS delivery across different cloud providers (Twilio, AWS SNS, Vonage, custom gateways).

## Key Features

- **Generic Sender Trait**: [`SmsSender`](src/lib.rs) defining `send_sms(&SmsMessage)`.
- **Domain Models**: `SmsMessage` struct and `SmsError` enum.
- **In-Memory Provider**: `MemorySmsSender` behind the default `memory` feature flag for instant unit testing.

## Usage

```rust
use sms_sender::{MemorySmsSender, SmsMessage, SmsSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sender = MemorySmsSender::new();
    let msg = SmsMessage::new("+15551234567", "Hello from backend-kit!");

    sender.send_sms(&msg).await?;
    assert_eq!(sender.sent_messages().await.len(), 1);

    Ok(())
}
```
