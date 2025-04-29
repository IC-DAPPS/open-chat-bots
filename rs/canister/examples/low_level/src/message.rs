use oc_bots_sdk::api::command::{
    Command, CommandArgValue, EphemeralMessageBuilder, Message, SuccessResult,
};
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;

pub fn send_message(
    text: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> SuccessResult {
    // Send the message to OpenChat but don't wait for the response
    SuccessResult {
        message: oc_client
            .send_text_message(text)
            .with_block_level_markdown(true)
            .execute_then_return_message(|args, response| match response {
                Ok(send_message::Response::Success(_)) => {}
                error => {
                    ic_cdk::println!("send_text_message: {args:?}, {error:?}");
                }
            }),
    }
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

pub fn send_message_with_visibility_option(
    reply: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> SuccessResult {
    let visibility: String = oc_client
        .context()
        .command
        .maybe_arg("Visibility")
        .unwrap_or_default();

    match visibility.as_str() {
        "Only to me" => send_ephemeral_message(reply, &oc_client.context().scope),
        "Everyone" => send_message(reply, &oc_client),

        // default case
        _ => send_ephemeral_message(reply, &oc_client.context().scope),
    }
}

// because maybe_arg method implemented in Command is still failing to
// give Option Value even if the argument is not provided by user
pub trait CommandExt {
    fn maybe_arg_opt(&self, name: &str) -> Option<String>;
}

// impl CommandExt for Command {
// fn maybe_arg_opt(&self, name: &str) -> Option<String> {
//     let value = self
//         .args
//         .iter()
//         .find(|arg| arg.name == name)
//         .map(|a| a.value.clone())?;

//     match value {
//         CommandArgValue::String(s) => Some(s),
//         _ => None,
//     }
// }
//
// fn maybe_arg_opt(&self, name: &str) -> Option<String> {
//     // let value = self.args;

//     for arg in &self.args {
//         if arg.name == name {
//             match &arg.value {
//                 CommandArgValue::String(s) => return Some(s.clone()),
//                 _ => return None,
//             }
//         }
//     }

//     None
// }
//
// fn maybe_arg_opt<T>(&self, name: &str) -> Option<T> {
//     // let value = self.args;

//     for arg in &self.args {
//         if arg.name == name {
//             match &arg.value {
//                 CommandArgValue::String(s) => return Some(s.clone()),
//                 CommandArgValue::Integer(i) => return Some(*i),
//                 CommandArgValue::Decimal(d) => return Some(*d),
//                 CommandArgValue::Boolean(b) => return Some(*b),
//                 CommandArgValue::User(u) => return Some(*u),
//                 CommandArgValue::DateTime(t) => return Some(*t),
//                 _ => return None,
//             }
//         }
//     }

//     None
// }
// }
