/// Lettre allows us to use a custom smtp email server directly which could
/// be the preferred option for business customers. They often have a dedicated
/// smtp server that they use for sending emails. With some additions to config
/// and connect code we can allow users to send via their Gmail account or other
/// smtp servers.
use super::{NotificationEmailTransportApi, Result, email::EmailMessage};
use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::stub::AsyncStubTransport,
};

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
    #[allow(dead_code)]
    pub fn new(relay: &str) -> Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(relay)?.build();
        Ok(Self { transport })
    }
}

#[async_trait]
impl NotificationEmailTransportApi for LettreSmtpTransport {
    async fn send(&self, message: EmailMessage) -> Result<()> {
        self.transport.send(message.try_into()?).await?;
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
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            transport: AsyncStubTransport::new_ok(),
        }
    }

    /// Creates a new instance of the stub transport that always
    /// fails sending the message.
    #[allow(dead_code)]
    pub fn new_error() -> Self {
        Self {
            transport: AsyncStubTransport::new_error(),
        }
    }
}

#[async_trait]
impl NotificationEmailTransportApi for StubEmailTransport {
    async fn send(&self, message: EmailMessage) -> Result<()> {
        self.transport.send(message.try_into()?).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::get_test_email_message;
    use super::*;

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
        assert!(fail.send(email.clone()).await.is_err());

        let success = StubEmailTransport::new();
        assert!(success.send(email).await.is_ok());
    }
}
