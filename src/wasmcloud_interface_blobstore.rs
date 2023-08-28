use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wasmcloud_provider_sdk::Context;

pub type ContainerId = String;
pub type ContainerIds = Vec<ContainerId>;
pub type ContainersInfo = Vec<ContainerMetadata>;
pub type MultiResult = Vec<ItemResult>;
pub type ObjectId = String;
pub type ObjectIds = Vec<ObjectId>;
pub type ObjectsInfo = Vec<ObjectMetadata>;

// This is a copy of the timestamp type from wasmbus_rpc for compatibility purposes, we should
// probably move to unix timestamp only (e.g. u64) in a wit world
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Timestamp {
    // Made this a u64 instead because that made more sense than a negative time
    pub sec: u64,
    pub nsec: u32,
}

impl Timestamp {
    pub fn now() -> Timestamp {
        let now = std::time::SystemTime::now();
        let since_epoch = now
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
        Timestamp {
            sec: since_epoch.as_secs(),
            nsec: since_epoch.subsec_nanos(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Chunk {
    #[serde(rename = "objectId")]
    pub object_id: ObjectId,
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    /// bytes in this chunk
    #[serde(with = "serde_bytes")]
    #[serde(default)]
    pub bytes: Vec<u8>,
    /// The byte offset within the object for this chunk
    #[serde(default)]
    pub offset: u64,
    /// true if this is the last chunk
    #[serde(rename = "isLast")]
    #[serde(default)]
    pub is_last: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChunkResponse {
    /// If set and `true`, the sender will stop sending chunks,
    #[serde(rename = "cancelDownload")]
    #[serde(default)]
    pub cancel_download: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ContainerMetadata {
    /// Container name
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    /// Creation date, if available
    #[serde(rename = "createdAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ContainerObject {
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    #[serde(rename = "objectId")]
    pub object_id: ObjectId,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GetObjectRequest {
    /// object to download
    #[serde(rename = "objectId")]
    pub object_id: ObjectId,
    /// object's container
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    /// Requested start of object to retrieve.
    /// The first byte is at offset 0. Range values are inclusive.
    /// If rangeStart is beyond the end of the file,
    /// an empty chunk will be returned with isLast == true
    #[serde(rename = "rangeStart")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_start: Option<u64>,
    /// Requested end of object to retrieve. Defaults to the object's size.
    /// It is not an error for rangeEnd to be greater than the object size.
    /// Range values are inclusive.
    #[serde(rename = "rangeEnd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_end: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GetObjectResponse {
    /// indication whether the request was successful
    #[serde(default)]
    pub success: bool,
    /// If success is false, this may contain an error
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The provider may begin the download by returning a first chunk
    #[serde(rename = "initialChunk")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_chunk: Option<Chunk>,
    /// Length of the content. (for multi-part downloads, this may not
    /// be the same as the length of the initial chunk)
    #[serde(rename = "contentLength")]
    #[serde(default)]
    pub content_length: u64,
    /// A standard MIME type describing the format of the object data.
    #[serde(rename = "contentType")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Specifies what content encodings have been applied to the object
    /// and thus what decoding mechanisms must be applied to obtain the media-type
    #[serde(rename = "contentEncoding")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ItemResult {
    #[serde(default)]
    pub key: String,
    /// whether the item succeeded or failed
    #[serde(default)]
    pub success: bool,
    /// optional error message for failures
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ListObjectsRequest {
    /// Name of the container to search
    #[serde(rename = "containerId")]
    #[serde(default)]
    pub container_id: String,
    /// Request object names starting with this value. (Optional)
    #[serde(rename = "startWith")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_with: Option<String>,
    /// Continuation token passed in ListObjectsResponse.
    /// If set, `startWith` is ignored. (Optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
    /// Last item to return (inclusive terminator) (Optional)
    #[serde(rename = "endWith")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_with: Option<String>,
    /// Optionally, stop returning items before returning this value.
    /// (exclusive terminator)
    /// If startFrom is "a" and endBefore is "b", and items are ordered
    /// alphabetically, then only items beginning with "a" would be returned.
    /// (Optional)
    #[serde(rename = "endBefore")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_before: Option<String>,
    /// maximum number of items to return. If not specified, provider
    /// will return an initial set of up to 1000 items. if maxItems > 1000,
    /// the provider implementation may return fewer items than requested.
    /// (Optional)
    #[serde(rename = "maxItems")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ListObjectsResponse {
    /// set of objects returned
    pub objects: ObjectsInfo,
    /// Indicates if the item list is complete, or the last item
    /// in a multi-part response.
    #[serde(rename = "isLast")]
    #[serde(default)]
    pub is_last: bool,
    /// If `isLast` is false, this value can be used in the `continuation` field
    /// of a `ListObjectsRequest`.
    /// Clients should not attempt to interpret this field: it may or may not
    /// be a real key or object name, and may be obfuscated by the provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ObjectMetadata {
    /// Object identifier that is unique within its container.
    /// Naming of objects is determined by the capability provider.
    /// An object id could be a path, hash of object contents, or some other unique identifier.
    #[serde(rename = "objectId")]
    pub object_id: ObjectId,
    /// container of the object
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    /// size of the object in bytes
    #[serde(rename = "contentLength")]
    #[serde(default)]
    pub content_length: u64,
    /// date object was last modified
    #[serde(rename = "lastModified")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<Timestamp>,
    /// A MIME type of the object
    /// see http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.17
    /// Provider implementations _may_ return None for this field for metadata
    /// returned from ListObjects
    #[serde(rename = "contentType")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Specifies what content encodings have been applied to the object
    /// and thus what decoding mechanisms must be applied to obtain the media-type
    /// referenced by the contentType field. For more information,
    /// see http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.11.
    /// Provider implementations _may_ return None for this field for metadata
    /// returned from ListObjects
    #[serde(rename = "contentEncoding")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PutChunkRequest {
    /// upload chunk from the file.
    /// if chunk.isLast is set, this will be the last chunk uploaded
    pub chunk: Chunk,
    /// This value should be set to the `streamId` returned from the initial PutObject.
    #[serde(rename = "streamId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
    /// If set, the receiving provider should cancel the upload process
    /// and remove the file.
    #[serde(rename = "cancelAndRemove")]
    #[serde(default)]
    pub cancel_and_remove: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PutObjectRequest {
    /// File path and initial data
    pub chunk: Chunk,
    /// A MIME type of the object
    /// see http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.17
    #[serde(rename = "contentType")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Specifies what content encodings have been applied to the object
    /// and thus what decoding mechanisms must be applied to obtain the media-type
    /// referenced by the contentType field. For more information,
    /// see http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.11.
    #[serde(rename = "contentEncoding")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RemoveObjectsRequest {
    /// name of container
    #[serde(rename = "containerId")]
    pub container_id: ContainerId,
    /// list of object names to be removed
    pub objects: ObjectIds,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PutObjectResponse {
    /// If this is a multipart upload, `streamId` must be returned
    /// with subsequent PutChunk requests
    #[serde(rename = "streamId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

#[async_trait]
pub trait Blobstore {
    /// returns the capability contract id for this interface
    fn contract_id() -> &'static str {
        "wasmcloud:blobstore"
    }
    /// Returns whether the container exists
    async fn container_exists(&self, ctx: Context, arg: ContainerId) -> Result<bool, String>;
    /// Creates a container by name, returning success if it worked
    /// Note that container names may not be globally unique - just unique within the
    /// "namespace" of the connecting actor and linkdef
    async fn create_container(&self, ctx: Context, arg: ContainerId) -> Result<(), String>;
    /// Retrieves information about the container.
    /// Returns error if the container id is invalid or not found.
    async fn get_container_info(
        &self,
        ctx: Context,
        arg: ContainerId,
    ) -> Result<ContainerMetadata, String>;
    /// Returns list of container ids
    async fn list_containers(&self, ctx: Context) -> Result<ContainersInfo, String>;
    /// Empty and remove the container(s)
    /// The MultiResult list contains one entry for each container
    /// that was not successfully removed, with the 'key' value representing the container name.
    /// If the MultiResult list is empty, all container removals succeeded.
    async fn remove_containers(
        &self,
        ctx: Context,
        arg: ContainerIds,
    ) -> Result<MultiResult, String>;
    /// Returns whether the object exists
    async fn object_exists(&self, ctx: Context, arg: ContainerObject) -> Result<bool, String>;
    /// Retrieves information about the object.
    /// Returns error if the object id is invalid or not found.
    async fn get_object_info(
        &self,
        ctx: Context,
        arg: ContainerObject,
    ) -> Result<ObjectMetadata, String>;
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
    ) -> Result<ListObjectsResponse, String>;
    /// Removes the objects. In the event any of the objects cannot be removed,
    /// the operation continues until all requested deletions have been attempted.
    /// The MultiRequest includes a list of errors, one for each deletion request
    /// that did not succeed. If the list is empty, all removals succeeded.
    async fn remove_objects(
        &self,
        ctx: Context,
        arg: RemoveObjectsRequest,
    ) -> Result<MultiResult, String>;
    /// Requests to start upload of a file/blob to the Blobstore.
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn put_object(
        &self,
        ctx: Context,
        arg: PutObjectRequest,
    ) -> Result<PutObjectResponse, String>;
    /// Requests to retrieve an object. If the object is large, the provider
    /// may split the response into multiple parts
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn get_object(
        &self,
        ctx: Context,
        arg: GetObjectRequest,
    ) -> Result<GetObjectResponse, String>;
    /// Uploads a file chunk to a blobstore. This must be called AFTER PutObject
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    async fn put_chunk(&self, ctx: Context, arg: PutChunkRequest) -> Result<(), String>;
}
