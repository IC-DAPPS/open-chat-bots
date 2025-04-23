use oc_bots_sdk::api::command::{EphemeralMessageBuilder, Message, SuccessResult};
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;

pub fn send_message(
    text: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> Option<Message> {
    // Send the message to OpenChat but don't wait for the response
    oc_client
        .send_text_message(text)
        .with_block_level_markdown(true)
        .execute_then_return_message(|args, response| match response {
            Ok(send_message::Response::Success(_)) => {}
            error => {
                ic_cdk::println!("send_text_message: {args:?}, {error:?}");
            }
        })
}

pub fn send_ephemeral_message(reply: String, scope: &BotCommandScope) -> SuccessResult {
    // Reply to the initiator with an ephemeral message
    EphemeralMessageBuilder::new(
        MessageContentInitial::from_text(reply),
        scope.message_id().unwrap(),
    )
    .build()
    .into()
}
