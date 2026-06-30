/********************************************************************************
 * Copyright (c) 2026 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License Version 2.0 which is available at
 * https://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

use common::Result as DiagResult;
use common::{KeyValueAttributes, ReplyMessagePayload};
use futures::future::BoxFuture;

// Input payload for announcing an entity endpoint to a diagnostics-facing registry.
#[derive(Clone, Debug, PartialEq)]
pub struct RegisterEntityArgs {
    // Unique entity identifier exposed through discovery.
    pub entity_id: String,
    // Human-readable entity name.
    pub entity_name: String,
    // Optional topology placement metadata used by adapters that require it.
    pub hosting_component: Option<String>,
    // Transport endpoint used by a bridge or server to access entity diagnostics.
    pub endpoint: String,
    // Optional entity-specific metadata.
    pub additional_attrs: Option<KeyValueAttributes>,
}

// Result payload returned after entity registration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegisterEntityReply {
    // Opaque registration handle, if the backend issues one.
    pub registration_id: Option<String>,
    // Optional lease window in milliseconds.
    pub lease_ms: Option<u64>,
}

// Input payload for removing an entity endpoint from a diagnostics-facing registry.
#[derive(Clone, Debug, PartialEq)]
pub struct DeregisterEntityArgs {
    // Entity identifier to remove.
    pub entity_id: String,
    // Optional registration handle returned by [`RegisterEntityReply`].
    pub registration_id: Option<String>,
}

/*
    Registry contract used by applications or bridges to register and deregister diagnostic entities.
    Implementations can use REST, IPC, message buses, or in-process runtime calls.
*/
pub trait EntityRegistrar {
    // Registers an entity endpoint and returns optional lease information.
    fn register_entity(
        &self,
        args: RegisterEntityArgs,
    ) -> BoxFuture<'_, DiagResult<RegisterEntityReply>>;

    // Removes a previously registered entity endpoint.
    fn deregister_entity(&self, args: DeregisterEntityArgs) -> BoxFuture<'_, DiagResult<()>>;
}

// Optional lookup contract for bridges that need to resolve entity endpoints.
pub trait EntityRegistryQuery {
    // Resolves the latest endpoint for a registered entity ID.
    fn resolve_endpoint(&self, entity_id: &str) -> BoxFuture<'_, DiagResult<ReplyMessagePayload>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::sovd::{ErrorCode, GenericError};
    use common::ReplyMessagePayload;
    use futures::FutureExt;

    struct InMemoryRegistrar;

    impl EntityRegistrar for InMemoryRegistrar {
        fn register_entity(
            &self,
            args: RegisterEntityArgs,
        ) -> BoxFuture<'_, DiagResult<RegisterEntityReply>> {
            async move {
                if args.entity_id.is_empty() {
                    return Err(common::Error::from_error(GenericError::from_code(
                        ErrorCode::IncompleteRequest,
                        "entity_id must not be empty".to_string(),
                    )));
                }

                Ok(RegisterEntityReply {
                    registration_id: Some("reg-1".to_string()),
                    lease_ms: Some(30_000),
                })
            }
            .boxed()
        }

        fn deregister_entity(&self, _args: DeregisterEntityArgs) -> BoxFuture<'_, DiagResult<()>> {
            async move { Ok(()) }.boxed()
        }
    }

    impl EntityRegistryQuery for InMemoryRegistrar {
        fn resolve_endpoint(
            &self,
            _entity_id: &str,
        ) -> BoxFuture<'_, DiagResult<ReplyMessagePayload>> {
            async move {
                Ok(ReplyMessagePayload::UTF8(
                    "http://127.0.0.1:8081/api".to_string(),
                ))
            }
            .boxed()
        }
    }

    #[tokio::test]
    async fn register_entity_returns_registration_id() {
        let registrar = InMemoryRegistrar;
        let reply = registrar
            .register_entity(RegisterEntityArgs {
                entity_id: "APP01".to_string(),
                entity_name: "Diagnostics App".to_string(),
                hosting_component: Some("HPC".to_string()),
                endpoint: "http://127.0.0.1:8081/api".to_string(),
                additional_attrs: None,
            })
            .await
            .expect("registration should succeed");

        assert_eq!(reply.registration_id, Some("reg-1".to_string()));
        assert_eq!(reply.lease_ms, Some(30_000));
    }

    #[tokio::test]
    async fn register_entity_rejects_empty_id() {
        let registrar = InMemoryRegistrar;
        let err = registrar
            .register_entity(RegisterEntityArgs {
                entity_id: "".to_string(),
                entity_name: "Diagnostics App".to_string(),
                hosting_component: Some("HPC".to_string()),
                endpoint: "http://127.0.0.1:8081/api".to_string(),
                additional_attrs: None,
            })
            .await
            .expect_err("registration should fail");

        match err.code {
            common::ErrorCode::SOVD(inner) => {
                assert_eq!(inner.sovd_error, ErrorCode::IncompleteRequest.to_string());
            }
            _ => panic!("expected SOVD error code"),
        }
    }
}
