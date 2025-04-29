use crate::message::{send_ephemeral_message, send_message_with_visibility_option};
use crate::utils::date_time::format_timestamp;
use async_trait::async_trait;
use candid::types::principal;
use candid::Principal;
use ic_cdk::api::management_canister::main::{
    canister_info, CanisterChange, CanisterChangeDetails, CanisterChangeOrigin,
    CanisterInfoRequest, CodeDeploymentMode,
};
use ic_cdk::call;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(CanisterInfo::definition);

pub struct CanisterInfo;

#[async_trait]
impl CommandHandler<CanisterRuntime> for CanisterInfo {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let canisterid_string: String = oc_client.context().command.arg("CanisterID");
        let num_of_req_changes: Option<i64> =
            oc_client.context().command.maybe_arg("Number_of_Changes");
        let timezone = oc_client.context().command.timezone();

        match get_canister_info(canisterid_string, num_of_req_changes, timezone).await {
            Ok(info) => Ok(send_message_with_visibility_option(info, &oc_client)),
            Err(err) => Ok(send_ephemeral_message(err, &oc_client.context().scope)),
        }
    }
}

async fn get_canister_info(
    canisterid_string: String,
    num_of_req_changes: Option<i64>,
    timezone: &str,
) -> Result<String, String> {
    let canister_id = get_principal_from_string(&canisterid_string)?;

    let arg = CanisterInfoRequest {
        canister_id,
        num_requested_changes: get_num_of_req_changes(num_of_req_changes),
    };

    let (canister_response,) = canister_info(arg)
        .await
        .map_err(|e| format!("Failed to Canister Status, {:?}", e))?;

    Ok(format!(
        "Total number of changes: {}\nControllers: {}\nModule Hash: {}\nCanister Changes: \n\n---\n{}",
        canister_response.total_num_changes,
        get_controllers_in_string(canister_response.controllers),
        get_module_hash(canister_response.module_hash),
        get_recent_changes(canister_response.recent_changes, timezone)
    ))
}

fn get_recent_changes(changes: Vec<CanisterChange>, timezone: &str) -> String {
    changes
        .iter().rev()
        .map(|change| {
            format!(
                "  - At: {}\n  - Canister Version: {}\n  - Change's Origin: {}\n  - Change's Details: {}",
                format_timestamp(change.timestamp_nanos, timezone)
                    .unwrap_or_else(|_| format!("{} (timestamp)", change.timestamp_nanos)),
                change.canister_version,
                get_origin(&change.origin),
                get_details(&change.details, timezone)
            )
        })
        .collect::<Vec<String>>()
        .join("\n---\n")
}

fn get_controllers_in_string(principals: Vec<Principal>) -> String {
    principals
        .iter()
        .map(|principal| format!("`{}`", principal.to_string()))
        .collect::<Vec<String>>()
        .join(", ")
}

fn get_origin(origin: &CanisterChangeOrigin) -> String {
    match origin {
        CanisterChangeOrigin::FromUser(user) => {
            format!("From User: `{}`", user.user_id.to_string())
        }
        CanisterChangeOrigin::FromCanister(canister) => match canister.canister_version {
            Some(version) => format!(
                "From Canister: `{}` (CanisterVersion: {})",
                canister.canister_id.to_string(),
                version
            ),
            None => format!("From Canister: `{}`", canister.canister_id.to_string()),
        },
    }
}

fn get_details(details: &CanisterChangeDetails, timezone: &str) -> String {
    match details {
        CanisterChangeDetails::Creation(creation) => {
            format!(
                "Creation: Controllers: {}\n",
                get_controllers_in_string(creation.controllers.clone())
            )
        }
        CanisterChangeDetails::CodeUninstall => "Code Uninstall".to_string(),
        CanisterChangeDetails::CodeDeployment(deployment) => {
            format!(
                "Code Deployment: \nMode: {}\nModule Hash: {}",
                get_mode(&deployment.mode),
                hex::encode(deployment.module_hash.clone())
            )
        }
        CanisterChangeDetails::LoadSnapshot(snapshot) => {
            format!(
                "Load Snapshot: \nCanister Version: {}\nSnapshot ID: {}\nTaken at: {}",
                snapshot.canister_version,
                hex::encode(snapshot.snapshot_id.clone()),
                format_timestamp(snapshot.taken_at_timestamp, timezone)
                    .unwrap_or_else(|_| format!("{} (timestamp)", snapshot.taken_at_timestamp))
            )
        }
        CanisterChangeDetails::ControllersChange(controllers) => {
            format!(
                "Controllers Change: {}",
                get_controllers_in_string(controllers.controllers.clone())
            )
        }
    }
}

fn get_mode(mode: &CodeDeploymentMode) -> String {
    match mode {
        CodeDeploymentMode::Install => "Install".to_string(),
        CodeDeploymentMode::Reinstall => "Reinstall".to_string(),
        CodeDeploymentMode::Upgrade => "Upgrade".to_string(),
    }
}
fn get_module_hash(hash_bytes: Option<Vec<u8>>) -> String {
    match hash_bytes {
        Some(bytes) => hex::encode(bytes),
        None => "None".to_string(),
    }
}

fn get_num_of_req_changes(num: Option<i64>) -> Option<u64> {
    match num {
        Some(num) => {
            if num > 0 {
                Some(num as u64)
            } else {
                None
            }
        }
        None => None,
    }
}

fn get_principal_from_string(str: &str) -> Result<Principal, String> {
    match Principal::from_text(str) {
        Ok(principal) => Ok(principal),
        Err(err) => Err(format!("{str} is invalid, {}", err.to_string())),
    }
}

impl CanisterInfo {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "canister_info".to_string(),
            description: Some("Provides the history of the canister, its current module SHA-256 hash, and its current controllers.".to_string()),
            placeholder: None,
            params: vec![
                BotCommandParam {
                    name: "CanisterID".to_string(),
                    description: Some("The canister ID of the canister to retrieve information about".to_string()),
                    param_type: BotCommandParamType::StringParam(StringParam{
                        min_length: 1,
                        max_length: 64,
                        multi_line: false,
                        choices: vec![]
                    }),
                    required: true,
                    placeholder: Some("Enter a Canister ID".to_string()),
                },
                BotCommandParam {
                    name: "Number_of_Changes".to_string(),
                    description: Some("Optional, specifies the number of requested canister changes.".to_string()),
                    param_type: BotCommandParamType::IntegerParam(IntegerParam{
                        min_value: 0,
                        max_value: 1000,
                        choices: vec![]
                    }),
                    required: false,
                    placeholder: Some("Enter number of requested canister changes".to_string()),
                },
                BotCommandParam {
                    name: "Visibility".to_string(),
                    description: Some("The visibility of the message".to_string()),
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 1,
                        max_length: 1000,
                        choices: vec![
                            BotCommandOptionChoice {
                                name: "Only to me".to_string(),
                                value: "Only to me".to_string(),
                            },
                            BotCommandOptionChoice {
                                name: "Everyone".to_string(),
                                value: "Everyone".to_string(),
                            },
                        ],
                        multi_line: false,
                    }),
                    required: true,
                    placeholder: Some("Message visibility".to_string()),
                },
            ],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: Some(false),
        }
    }
}
