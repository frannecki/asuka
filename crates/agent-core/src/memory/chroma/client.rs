use std::sync::Mutex;

use hyper::{body::to_bytes, Body, Client, Method, Request, StatusCode};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    domain::MemorySearchHit,
    error::{CoreError, CoreResult},
};

use super::{
    embeddings::{chroma_disabled_via_env, embed_text},
    types::{
        ChromaCollection, ChromaQueryRequest, ChromaQueryResponse, ChromaRecord,
        ChromaUpsertRequest,
    },
};

pub(crate) struct ChromaClient {
    base_url: String,
    tenant: String,
    database: String,
    collection_name: String,
    collection_id: Mutex<String>,
    http_client: Client<HttpsConnector<hyper::client::HttpConnector>, Body>,
}

impl ChromaClient {
    pub(crate) async fn connect() -> anyhow::Result<Option<Self>> {
        if chroma_disabled_via_env() {
            return Ok(None);
        }

        let base_url =
            std::env::var("CHROMA_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
        let tenant =
            std::env::var("CHROMA_TENANT").unwrap_or_else(|_| "default_tenant".to_string());
        let database =
            std::env::var("CHROMA_DATABASE").unwrap_or_else(|_| "default_database".to_string());
        let collection_name =
            std::env::var("CHROMA_COLLECTION").unwrap_or_else(|_| "asuka-memory".to_string());

        let connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();
        let http_client = Client::builder().build(connector);

        let client = Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            tenant,
            database,
            collection_name,
            collection_id: Mutex::new(String::new()),
            http_client,
        };
        client.ensure_collection().await?;
        Ok(Some(client))
    }

    async fn ensure_collection(&self) -> anyhow::Result<()> {
        let path = format!(
            "/api/v2/tenants/{}/databases/{}/collections/{}",
            self.tenant, self.database, self.collection_name
        );
        let (status, body) = self
            .request(Method::GET, &path, Option::<&Value>::None)
            .await?;
        match status {
            StatusCode::OK => {
                let collection = serde_json::from_slice::<ChromaCollection>(&body)?;
                self.set_collection_id(collection.id)?;
                Ok(())
            }
            StatusCode::NOT_FOUND => {
                let payload =
                    json!({ "name": self.collection_name, "metadata": { "app": "asuka" } });
                let (status, body) = self
                    .request(
                        Method::POST,
                        &format!(
                            "/api/v2/tenants/{}/databases/{}/collections",
                            self.tenant, self.database
                        ),
                        Some(&payload),
                    )
                    .await?;

                if !status.is_success() {
                    return Err(anyhow::anyhow!(
                        "failed to create chroma collection: {}",
                        String::from_utf8_lossy(&body)
                    ));
                }

                let collection = serde_json::from_slice::<ChromaCollection>(&body)?;
                self.set_collection_id(collection.id)?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "failed to look up chroma collection: {}",
                String::from_utf8_lossy(&body)
            )),
        }
    }

    pub(crate) async fn reset_collection(&self) -> CoreResult<()> {
        let (status, body) = self
            .request(
                Method::DELETE,
                &format!(
                    "/api/v2/tenants/{}/databases/{}/collections/{}",
                    self.tenant, self.database, self.collection_name
                ),
                Option::<&Value>::None,
            )
            .await?;

        if !(status.is_success() || status == StatusCode::NOT_FOUND) {
            return Err(CoreError::new(
                502,
                format!(
                    "failed to reset chroma collection with {}: {}",
                    status,
                    String::from_utf8_lossy(&body)
                ),
            ));
        }

        self.set_collection_id(String::new())?;
        self.ensure_collection().await.map_err(|error| {
            CoreError::new(
                502,
                format!("failed to recreate chroma collection: {error}"),
            )
        })?;
        Ok(())
    }

    pub(crate) async fn upsert_records(&self, records: Vec<ChromaRecord>) -> CoreResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        let payload = ChromaUpsertRequest {
            ids: records.iter().map(|record| record.id.clone()).collect(),
            embeddings: records
                .iter()
                .map(|record| record.embedding.clone())
                .collect(),
            documents: records
                .iter()
                .map(|record| record.document.clone())
                .collect(),
            metadatas: records.into_iter().map(|record| record.metadata).collect(),
        };

        let (status, body) = self
            .request(
                Method::POST,
                &format!(
                    "/api/v2/tenants/{}/databases/{}/collections/{}/upsert",
                    self.tenant,
                    self.database,
                    self.current_collection_id()?
                ),
                Some(&payload),
            )
            .await?;

        if !status.is_success() {
            return Err(CoreError::new(
                502,
                format!(
                    "chroma upsert failed with {}: {}",
                    status,
                    String::from_utf8_lossy(&body)
                ),
            ));
        }

        Ok(())
    }

    pub(crate) async fn query(
        &self,
        query: &str,
        namespace: Option<&str>,
        limit: usize,
    ) -> CoreResult<Vec<MemorySearchHit>> {
        let payload = ChromaQueryRequest {
            query_embeddings: vec![embed_text(query)],
            n_results: limit.max(1),
            include: vec![
                "documents".to_string(),
                "metadatas".to_string(),
                "distances".to_string(),
            ],
            r#where: namespace.map(|value| json!({ "namespace": { "$eq": value } })),
        };

        let (status, body) = self
            .request(
                Method::POST,
                &format!(
                    "/api/v2/tenants/{}/databases/{}/collections/{}/query",
                    self.tenant,
                    self.database,
                    self.current_collection_id()?
                ),
                Some(&payload),
            )
            .await?;

        if !status.is_success() {
            return Err(CoreError::new(
                502,
                format!(
                    "chroma query failed with {}: {}",
                    status,
                    String::from_utf8_lossy(&body)
                ),
            ));
        }

        let response = serde_json::from_slice::<ChromaQueryResponse>(&body).map_err(|error| {
            CoreError::new(
                502,
                format!("failed to parse chroma query response: {error}"),
            )
        })?;
        let ids = response.ids.into_iter().next().unwrap_or_default();
        let documents = response.documents.into_iter().next().unwrap_or_default();
        let metadatas = response.metadatas.into_iter().next().unwrap_or_default();
        let distances = response.distances.into_iter().next().unwrap_or_default();

        let mut hits = Vec::new();
        for index in 0..ids.len() {
            let metadata = metadatas
                .get(index)
                .cloned()
                .flatten()
                .unwrap_or_else(|| json!({}));
            let document = documents.get(index).cloned().flatten().unwrap_or_default();
            let distance = distances.get(index).cloned().flatten().unwrap_or(0.0);

            let document_id = metadata
                .get("document_id")
                .and_then(|value| value.as_str())
                .and_then(|value| Uuid::parse_str(value).ok())
                .unwrap_or_else(Uuid::nil);
            let chunk_id = Uuid::parse_str(&ids[index]).unwrap_or_else(|_| Uuid::nil());
            let document_title = metadata
                .get("document_title")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let namespace = metadata
                .get("namespace")
                .and_then(|value| value.as_str())
                .unwrap_or("global")
                .to_string();

            hits.push(MemorySearchHit {
                document_id,
                chunk_id,
                document_title,
                namespace,
                content: document,
                score: 1.0 / (1.0 + distance.abs()),
            });
        }

        Ok(hits)
    }

    fn current_collection_id(&self) -> CoreResult<String> {
        self.collection_id
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| CoreError::new(500, "failed to lock chroma collection id"))
    }

    fn set_collection_id(&self, id: String) -> CoreResult<()> {
        let mut guard = self
            .collection_id
            .lock()
            .map_err(|_| CoreError::new(500, "failed to lock chroma collection id"))?;
        *guard = id;
        Ok(())
    }

    async fn request<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> CoreResult<(StatusCode, Vec<u8>)> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = Request::builder().method(method).uri(url);
        if body.is_some() {
            request = request.header("Content-Type", "application/json");
        }

        let body = match body {
            Some(body) => Body::from(serde_json::to_vec(body).map_err(|error| {
                CoreError::new(500, format!("invalid chroma payload: {error}"))
            })?),
            None => Body::empty(),
        };

        let request = request.body(body).map_err(|error| {
            CoreError::new(500, format!("failed to build chroma request: {error}"))
        })?;
        let response = self.http_client.request(request).await.map_err(|error| {
            CoreError::new(502, format!("failed to reach chroma server: {error}"))
        })?;
        let status = response.status();
        let bytes = to_bytes(response.into_body()).await.map_err(|error| {
            CoreError::new(502, format!("failed to read chroma response: {error}"))
        })?;
        Ok((status, bytes.to_vec()))
    }
}
