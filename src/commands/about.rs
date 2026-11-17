use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};

use crate::commands::CabbotCommandError;

pub fn register() -> CreateCommand {
    CreateCommand::new("about").description("Information about the bot")
}

pub async fn run(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CabbotCommandError> {
    interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content())
                    .ephemeral(true),
            ),
        )
        .await
        .map_err(CabbotCommandError::response_failed)?;
    Ok(())
}

fn content() -> String {
    format!(
        "## Cabbot\n\
        -# v{} • made by echolotl\n\
        ",
        env!("CARGO_PKG_VERSION")
    )
}
