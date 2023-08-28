//! Nats implementation for wasmcloud:messaging.

use std::collections::HashMap;
use std::sync::Arc;

use blobstore_vault::error::VaultError;
use futures::FutureExt;
use tokio::sync::{OwnedRwLockReadGuard, RwLock};
use tracing::{debug, error, instrument};
use wasmcloud_provider_sdk::error::ProviderInvocationError;
use wasmcloud_provider_sdk::ProviderHandler;
use wasmcloud_provider_sdk::{core::LinkDefinition, start_provider, Context};

use blobstore_vault::wasmcloud_interface_blobstore::*;
use blobstore_vault::{client::Client, config::Config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // handle lattice control messages and forward rpc to the provider dispatch
    // returns when provider receives a shutdown control message
    start_provider(
        VaultBlobstoreProvider::default(),
        Some("NATS Messaging Provider".to_string()),
    )?;

    eprintln!("Vault Blobstore provider exiting");
    Ok(())
}

/// Nats implementation for wasmcloud:messaging
#[derive(Default, Clone)]
struct VaultBlobstoreProvider {
    // TODO: Make this an actual vault client type
    actors: Arc<RwLock<HashMap<String, Client>>>,
}

impl VaultBlobstoreProvider {
    /// Get a vault client for the actor
    async fn get_client(
        &self,
        ctx: &Context,
    ) -> Result<OwnedRwLockReadGuard<HashMap<String, Client>, Client>, String> {
        let actors = self.actors.clone().read_owned().await;
        OwnedRwLockReadGuard::try_map(actors, |a| a.get(ctx.actor.as_deref().unwrap_or_default()))
            .map_err(|_| "Actor is not linked".to_string())
    }
}

/// Handle provider control commands
/// put_link (new actor link command), del_link (remove link command), and shutdown
#[async_trait::async_trait]
impl ProviderHandler for VaultBlobstoreProvider {
    /// Provider should perform any operations needed for a new link,
    /// including setting up per-actor resources, and checking authorization.
    /// If the link is allowed, return true, otherwise return false to deny the link.
    #[instrument(level = "debug", skip(self, ld), fields(actor_id = %ld.actor_id))]
    async fn put_link(&self, ld: &LinkDefinition) -> bool {
        let config = match Config::from_values(&ld.values) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to parse values: {e:?}");
                return false;
            }
        };
        let client = match Client::new(config) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to Vault: {e:?}");
                return false;
            }
        };

        self.actors
            .write()
            .await
            .insert(ld.actor_id.clone(), client);

        true
    }

    /// Handle notification that a link is dropped: close the connection
    #[instrument(level = "info", skip(self))]
    async fn delete_link(&self, actor_id: &str) {
        let mut aw = self.actors.write().await;

        if let Some(_client) = aw.remove(actor_id) {
            // Note: subscriptions will be closed via Drop on the NatsClientBundle
            debug!(
                %actor_id,
                "unlinking actor",
            );
        }
    }

    /// Handle shutdown request by closing all connections
    async fn shutdown(&self) {
        let mut aw = self.actors.write().await;
        // empty the actor link data
        aw.clear();
        // dropping all connections should send unsubscribes and close the connections, so no need
        // to handle that here
    }
}

#[async_trait::async_trait]
impl Blobstore for VaultBlobstoreProvider {
    /// Returns whether the container exists
    async fn container_exists(&self, _ctx: Context, _arg: ContainerId) -> Result<bool, String> {
        // If we were doing this for real, we'd probably make this tied to a secrets namespace
        Ok(true)
    }
    /// Creates a container by name, returning success if it worked
    /// Note that container names may not be globally unique - just unique within the
    /// "namespace" of the connecting actor and linkdef
    async fn create_container(&self, _ctx: Context, _arg: ContainerId) -> Result<(), String> {
        // We don't actually need to create a container because it is purely contained in the path
        // name of the secret, so just noop here
        Ok(())
    }
    /// Retrieves information about the container.
    /// Returns error if the container id is invalid or not found.
    async fn get_container_info(
        &self,
        _ctx: Context,
        arg: ContainerId,
    ) -> Result<ContainerMetadata, String> {
        Ok(ContainerMetadata {
            container_id: arg,
            created_at: Some(Timestamp::now()),
        })
    }

    /// Returns list of container ids
    async fn list_containers(&self, _ctx: Context) -> Result<ContainersInfo, String> {
        Ok(Vec::with_capacity(0))
    }
    /// Empty and remove the container(s)
    /// The MultiResult list contains one entry for each container
    /// that was not successfully removed, with the 'key' value representing the container name.
    /// If the MultiResult list is empty, all container removals succeeded.
    async fn remove_containers(
        &self,
        _ctx: Context,
        _arg: ContainerIds,
    ) -> Result<MultiResult, String> {
        Ok(Vec::with_capacity(0))
    }
    /// Returns whether the object exists
    async fn object_exists(&self, ctx: Context, arg: ContainerObject) -> Result<bool, String> {
        let client = self.get_client(&ctx).await?;
        match client.get_metadata(&arg.object_id).await {
            Ok(_) => Ok(true),
            Err(VaultError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
    /// Retrieves information about the object.
    /// Returns error if the object id is invalid or not found.
    async fn get_object_info(
        &self,
        ctx: Context,
        arg: ContainerObject,
    ) -> Result<ObjectMetadata, String> {
        let client = self.get_client(&ctx).await?;
        client
            .get_metadata(&arg.object_id)
            .await
            .map_err(|e| e.to_string())
            .map(|_| ObjectMetadata {
                object_id: arg.object_id,
                container_id: arg.container_id,
                content_length: 0,
                content_type: None,
                content_encoding: None,
                last_modified: None,
            })
    }

    /// Lists the objects in the container.
    /// If the container exists and is empty, the returned `objects` list is empty.
    /// Parameters of the request may be used to limit the object names returned
    /// with an optional start value, end value, and maximum number of items.
    /// The provider may limit the number of items returned. If the list is truncated,
    /// the response contains a `continuation` token that may be submitted in
    /// a subsequent ListObjects request.
    ///
    /// Optional object metadata fields (i.e., `contentType` and `contentEncoding`) may not be
    /// filled in for ListObjects response. To get complete object metadata, use GetObjectInfo.
    async fn list_objects(
        &self,
        ctx: Context,
        arg: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, String> {
        let client = self.get_client(&ctx).await?;
        client
            .list_files(&arg.container_id)
            .await
            .map_err(|e| e.to_string())
            .map(|objs| ListObjectsResponse {
                objects: objs
                    .into_iter()
                    .map(|o| ObjectMetadata {
                        object_id: o,
                        container_id: arg.container_id.clone(),
                        content_length: 0,
                        content_type: None,
                        content_encoding: None,
                        last_modified: None,
                    })
                    .collect(),
                is_last: true,
                continuation: None,
            })
    }
    /// Removes the objects. In the event any of the objects cannot be removed,
    /// the operation continues until all requested deletions have been attempted.
    /// The MultiRequest includes a list of errors, one for each deletion request
    /// that did not succeed. If the list is empty, all removals succeeded.
    async fn remove_objects(
        &self,
        ctx: Context,
        arg: RemoveObjectsRequest,
    ) -> Result<MultiResult, String> {
        let client = self.get_client(&ctx).await?;
        let futs = arg.objects.into_iter().map(|key| {
            let cloned_key = key.clone();
            client.delete_file(key).map(|res| match res {
                Ok(_) => ItemResult {
                    key: cloned_key,
                    error: None,
                    success: true,
                },
                Err(e) => ItemResult {
                    key: cloned_key,
                    error: Some(e.to_string()),
                    success: false,
                },
            })
        });
        let results = futures::future::join_all(futs).await;
        Ok(results)
    }
    /// Requests to start upload of a file/blob to the Blobstore.
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn put_object(
        &self,
        ctx: Context,
        arg: PutObjectRequest,
    ) -> Result<PutObjectResponse, String> {
        let client = self.get_client(&ctx).await?;
        client
            .write_file(arg.chunk.object_id, arg.chunk.bytes)
            .await
            .map_err(|e| e.to_string())
            .map(|_| PutObjectResponse { stream_id: None })
    }
    /// Requests to retrieve an object. If the object is large, the provider
    /// may split the response into multiple parts
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn get_object(
        &self,
        ctx: Context,
        arg: GetObjectRequest,
    ) -> Result<GetObjectResponse, String> {
        let client = self.get_client(&ctx).await?;
        client
            .read_file(&arg.object_id)
            .await
            .map_err(|e| e.to_string())
            .map(|data| GetObjectResponse {
                success: true,
                error: None,
                initial_chunk: Some(Chunk {
                    object_id: arg.object_id,
                    container_id: arg.container_id,
                    bytes: data,
                    is_last: true,
                    offset: 0,
                }),
                ..Default::default()
            })
    }
    /// Uploads a file chunk to a blobstore. This must be called AFTER PutObject
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn put_chunk(&self, _ctx: Context, _arg: PutChunkRequest) -> Result<(), String> {
        Err("Chunking not supported".to_string())
    }
}

#[async_trait::async_trait]
impl wasmcloud_provider_sdk::MessageDispatch for VaultBlobstoreProvider {
    async fn dispatch<'a>(
        &'a self,
        ctx: Context,
        method: String,
        body: std::borrow::Cow<'a, [u8]>,
    ) -> Result<Vec<u8>, ProviderInvocationError> {
        match method.as_str() {
            "Blobstore.ContainerExists" => {
                let input: ContainerId = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.container_exists(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.CreateContainer" => {
                let input: ContainerId = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.create_container(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.GetContainerInfo" => {
                let input: ContainerId = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.get_container_info(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.ListContainers" => {
                let _input: () = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.list_containers(ctx).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.RemoveContainers" => {
                let input: ContainerIds = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.remove_containers(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.ObjectExists" => {
                let input: ContainerObject = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.object_exists(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.GetObjectInfo" => {
                let input: ContainerObject = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.get_object_info(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.ListObjects" => {
                let input: ListObjectsRequest = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.list_objects(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.RemoveObjects" => {
                let input: RemoveObjectsRequest = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.remove_objects(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.PutObject" => {
                let input: PutObjectRequest = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.put_object(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.GetObject" => {
                let input: GetObjectRequest = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.get_object(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            "Blobstore.PutChunk" => {
                let input: PutChunkRequest = ::wasmcloud_provider_sdk::deserialize(&body)?;
                let result = self.put_chunk(ctx, input).await.map_err(|e| {
                    ::wasmcloud_provider_sdk::error::ProviderInvocationError::Provider(
                        e.to_string(),
                    )
                })?;
                Ok(::wasmcloud_provider_sdk::serialize(&result)?)
            }
            _ => Err(
                ::wasmcloud_provider_sdk::error::InvocationError::Malformed(format!(
                    "Invalid method name {method}",
                ))
                .into(),
            ),
        }
    }
}

impl wasmcloud_provider_sdk::Provider for VaultBlobstoreProvider {}
