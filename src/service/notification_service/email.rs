use super::Result;
use async_trait::async_trait;
use lettre::{
    transport::stub::AsyncStubTransport, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};

#[async_trait]
pub trait NotificationEmailTransportApi: Send + Sync {
    async fn send(&self, event: Message) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
}

impl TryFrom<EmailMessage> for Message {
    type Error = super::Error;
    fn try_from(message: EmailMessage) -> Result<Self> {
        let m = Message::builder()
            .from(message.from.parse()?)
            .to(message.to.parse()?)
            .subject(message.subject)
            .body(message.body)?;
        Ok(m)
    }
}

/// A wrapper around lettre's async transport that implements the NotificationEmailTransportApi.
pub struct LettreSmtpTransport {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl LettreSmtpTransport {
    pub fn new(relay: &str) -> Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(relay)?.build();
        Ok(Self { transport })
    }
}

#[async_trait]
impl NotificationEmailTransportApi for LettreSmtpTransport {
    async fn send(&self, message: Message) -> Result<()> {
        self.transport.send(message).await?;
        Ok(())
    }
}

/// A stub email transport that always succeeds or fails sending the message.
/// Will log sent messages to the console and requires no configuration.
pub struct StubEmailTransport {
    transport: AsyncStubTransport,
}

impl StubEmailTransport {
    /// Creates a new instance of the stub transport that always
    /// succeeds sending the message.
    pub fn new() -> Self {
        Self {
            transport: AsyncStubTransport::new_ok(),
        }
    }

    /// Creates a new instance of the stub transport that always
    /// fails sending the message.
    pub fn new_error() -> Self {
        Self {
            transport: AsyncStubTransport::new_error(),
        }
    }
}

#[async_trait]
impl NotificationEmailTransportApi for StubEmailTransport {
    async fn send(&self, message: Message) -> Result<()> {
        self.transport.send(message).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_email_message() -> EmailMessage {
        EmailMessage {
            from: "sender@example.com".to_string(),
            to: "recipient@example.com".to_string(),
            subject: "Hello World".to_string(),
            body: "This is a test email.".to_string(),
        }
    }

    #[test]
    fn test_email_message_conversion() {
        let message = get_test_email_message();
        let _: Message = message.try_into().expect("Failed to convert email message");
    }

    #[tokio::test]
    async fn test_smtp_transport() {
        LettreSmtpTransport::new("smtp.example.com:587").expect("Failed to create smtp transport");
    }

    #[tokio::test]
    async fn test_stub_transport() {
        let email = get_test_email_message();

        let fail = StubEmailTransport::new_error();
        assert!(fail.send(email.clone().try_into().unwrap()).await.is_err());

        let success = StubEmailTransport::new();
        assert!(success.send(email.try_into().unwrap()).await.is_ok());
    }
}
