use lifec::{
    prelude::{BlockObject, BlockProperties, Plugin, Process, ThunkContext},
    state::AttributeIndex,
};
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

/// Plugin to process an azure guest,
///
/// Example blob metadata:
///
/// ```norun
/// "container": "test",
/// "name": "status/frames",
/// "properties": {
///     "etag": "0x8DAC0EE11A6A987",
/// ```
///
pub struct AzureGuest;

impl AzureGuest {
    /// Fetches guest commands,
    ///
    pub async fn fetch_guest_commands(
        cancel_source: Receiver<()>,
        tc: &mut ThunkContext,
    ) -> Option<ThunkContext> {
        Self::execute_script("fetch-guest-commands.sh", cancel_source, tc).await
    }

    /// Fetches guest state,
    ///
    pub async fn fetch_guest_state(
        cancel_source: Receiver<()>,
        tc: &mut ThunkContext,
    ) -> Option<ThunkContext> {
        Self::execute_script("fetch-guest-state.sh", cancel_source, tc).await
    }

    /// Monitors guest and uplaods state,
    ///
    pub async fn monitor_guest(
        cancel_source: Receiver<()>,
        tc: &mut ThunkContext,
    ) -> Option<ThunkContext> {
        Self::execute_script("monitor-guest.sh", cancel_source, tc).await
    }

    /// Sends guest commands,
    ///
    pub async fn send_guest_commands(
        cancel_source: Receiver<()>,
        tc: &mut ThunkContext,
    ) -> Option<ThunkContext> {
        Self::execute_script("send-guest-commands.sh", cancel_source, tc).await
    }

    /// Executes a script,
    ///
    async fn execute_script(
        script: impl AsRef<str>,
        cancel_source: Receiver<()>,
        tc: &mut ThunkContext,
    ) -> Option<ThunkContext> {
        tc.with_symbol("process", format!("sh {}", script.as_ref()));

        lifec::plugins::await_plugin::<Process>(cancel_source, tc, |result| Some(result)).await
    }
}

impl Plugin for AzureGuest {
    fn symbol() -> &'static str {
        "azure_guest"
    }

    fn description() -> &'static str {
        "Processes guest state in azure storage."
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                let cancel_source = cancel_source;

                let mut objects = vec![];

                if let Some(output) = tc.find_text("output") {
                    match serde_json::de::from_str::<serde_json::Value>(&output) {
                        Ok(value) => {
                            // Value should be an array of json objects
                            assert!(value.is_array());

                            match value.as_array() {
                                Some(value) => {
                                    for blob in value.iter() {
                                        let blob = blob.as_object().expect("should be an object");
                                        let name = blob
                                            .get("name")
                                            .expect("should have a name")
                                            .as_str()
                                            .expect("should be a string")
                                            .to_string();

                                        let etag = blob
                                            .get("properties")
                                            .expect("should have properties")
                                            .as_object()
                                            .expect("should be an object")
                                            .get("etag")
                                            .expect("should have an etag");

                                        let container = blob
                                            .get("container")
                                            .expect("should have a name")
                                            .as_str()
                                            .expect("should be a string")
                                            .to_string();
                                        
                                        objects.push((container, etag, name));
                                    }
                                }
                                None => {
                                    unreachable!("Should be an array");
                                }
                            }
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error deserializing cached output, {err}");
                        }
                    }
                }


                if let Some(result) = Self::monitor_guest(cancel_source, &mut tc).await {
                    
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for AzureGuest {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
