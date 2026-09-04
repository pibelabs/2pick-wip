//! E-Mail Service. See [`VerificationService`].

use std::{ffi::OsStr, sync::Arc};

use email_address::EmailAddress;

use minijinja::Environment;
use resend_rs::{Resend, types::CreateEmailBaseOptions};
use tokio::{
    spawn,
    sync::mpsc::{Receiver, Sender, channel, error::SendError},
};

/// Information about an E-Mail to be sent.
pub struct EmailPacket {
    tmpl: String,
    ctx: minijinja::Value,
    to: EmailAddress,
    subject: String,
}

impl EmailPacket {
    pub fn new(tmpl: String, ctx: minijinja::Value, to: EmailAddress, subject: String) -> Self {
        Self {
            tmpl,
            ctx,
            to,
            subject,
        }
    }
}

/// An interface for sending verification E-Mails.
///
/// Added as an `Extension` automagically to the router.
#[derive(Clone)]
pub struct EmailService(Sender<EmailPacket>);

impl EmailService {
    pub async fn new(api_key: String, self_email: EmailAddress) -> anyhow::Result<Self> {
        let mut env = Environment::<'static>::new();
        let mut template_dir = tokio::fs::read_dir("templates").await?;

        while let Some(file) = template_dir.next_entry().await? {
            let path = file.path();
            if !file.file_type().await?.is_file() {
                continue;
            }

            let contents = tokio::fs::read_to_string(&path).await?;
            let Some(stem) = path.file_stem().map(OsStr::to_owned) else {
                return Err(anyhow::Error::msg(format!(
                    "{} does not have a name",
                    path.display()
                )));
            };

            env.add_template_owned(stem.to_string_lossy().into_owned(), contents)?;
        }

        let (tx, rx) = channel(1024);
        tracing::info!("Starting user verification email service...");
        let resend = Resend::new(&api_key);

        spawn(service(self_email.clone(), resend, env, rx));

        Ok(Self(tx))
    }

    /// Queue an E-Mail. This does not guarantee that the E-Mail will be sent, even if this returns `Ok(())`.
    ///
    /// # Errors
    /// Errors if the service has shut down.
    pub async fn send(&self, packet: EmailPacket) -> Result<(), SendError<EmailPacket>> {
        self.0.send(packet).await
    }
}

async fn service(
    self_email: EmailAddress,
    resend: Resend,
    env: Environment<'static>,
    mut rx: Receiver<EmailPacket>,
) {
    tracing::info!("Started user verification email service");
    let resend = Arc::new(resend);
    let env = Arc::new(env);

    while let Some(v) = rx.recv().await {
        spawn(send_mail(
            v,
            resend.clone(),
            env.clone(),
            self_email.clone(),
        ));
    }
}

async fn send_mail(
    packet: EmailPacket,
    resend: Arc<Resend>,
    env: Arc<Environment<'static>>,
    self_email: EmailAddress,
) {
    let tmpl = match env.get_template(&packet.tmpl) {
        Ok(v) => v,
        Err(e) => {
            tracing::info!(err = ?e, template = packet.tmpl, "Failed to get template");
            return;
        }
    };

    let rendered = match tmpl.render(packet.ctx) {
        Ok(v) => v,
        Err(e) => {
            tracing::info!(err = ?e, "Failed to render template");
            return;
        }
    };

    let email = CreateEmailBaseOptions::new(
        format!("2pick <{}>", self_email.as_str()),
        [packet.to.email()],
        packet.subject,
    )
    .with_html(&rendered);

    if let Err(e) = resend.emails.send(email).await {
        tracing::error!(
            "Failed to send verification email to user with email address '{}': {e:#?}",
            packet.to.as_str()
        );
    }
}
