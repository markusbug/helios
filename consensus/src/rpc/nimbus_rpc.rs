use std::cmp;

use async_trait::async_trait;
use eyre::Result;
use retri::{retry, BackoffSettings};
use serde::de::DeserializeOwned;

use super::ConsensusRpc;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::types::*;
use common::errors::RpcError;

use std::process::Command;

#[derive(Debug)]
pub struct NimbusRpc {
    rpc: String,
}

async fn get<R: DeserializeOwned>(req: &str) -> Result<R> {
    // Execute curl to fetch the data asynchronously
    let output = Command::new("curl")
        .arg("-s")  // Silent mode to not include progress meter or error messages
        .arg(req)
        .output()
        .map_err(|e| {
            let error_message = format!("Failed to execute curl command: {}", e);
            println!("{}", error_message);
            eyre::Report::new(e)
        })?;

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr)
            .unwrap_or("Failed to decode error message from stderr");
        println!("Curl command failed: {}", error_message);
        return Err(eyre::Report::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Curl failed: {}", error_message),
        )));
    }

    // Parse the output, assuming the body is directly usable as the bytes of the JSON response
    let bytes = &output.stdout;

    // Deserialize JSON to the specified type
    let result: R = serde_json::from_slice(bytes).map_err(|e| {
        let error_message = format!("Failed to deserialize JSON response: {}", e);
        println!("{}", error_message);
        eyre::Report::new(e)
    })?;

    Ok(result)
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ConsensusRpc for NimbusRpc {
    fn new(rpc: &str) -> Self {
        NimbusRpc {
            rpc: rpc.to_string(),
        }
    }

    async fn get_bootstrap(&self, block_root: &'_ [u8]) -> Result<Bootstrap> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v1/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );

        let res: BootstrapResponse = get(&req).await.map_err(|e| RpcError::new("bootstrap", e))?;

        Ok(res.data)
    }

    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>> {
        let count = cmp::min(count, MAX_REQUEST_LIGHT_CLIENT_UPDATES);
        let req = format!(
            "{}/eth/v1/beacon/light_client/updates?start_period={}&count={}",
            self.rpc, period, count
        );

        let res: UpdateResponse = get(&req).await.map_err(|e| RpcError::new("updates", e))?;

        Ok(res.into_iter().map(|d| d.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/finality_update", self.rpc);
        let res: FinalityUpdateResponse = get(&req)
            .await
            .map_err(|e| RpcError::new("finality_update", e))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res: OptimisticUpdateResponse = get(&req)
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res: BeaconBlockResponse = get(&req).await.map_err(|e| RpcError::new("blocks", e))?;

        Ok(res.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        let req = format!("{}/eth/v1/config/spec", self.rpc);
        let res: SpecResponse = get(&req).await.map_err(|e| RpcError::new("spec", e))?;

        Ok(res.data.chain_id.into())
    }
}

#[derive(serde::Deserialize, Debug)]
struct BeaconBlockResponse {
    data: BeaconBlockData,
}

#[derive(serde::Deserialize, Debug)]
struct BeaconBlockData {
    message: BeaconBlock,
}

type UpdateResponse = Vec<UpdateData>;

#[derive(serde::Deserialize, Debug)]
struct UpdateData {
    data: Update,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct OptimisticUpdateResponse {
    data: OptimisticUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapResponse {
    data: Bootstrap,
}

#[derive(serde::Deserialize, Debug)]
struct SpecResponse {
    data: Spec,
}

#[derive(serde::Deserialize, Debug)]
struct Spec {
    #[serde(rename = "DEPOSIT_NETWORK_ID")]
    chain_id: primitives::U64,
}
