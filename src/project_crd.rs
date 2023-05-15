#![allow(non_snake_case)]
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "kyotu.tech",
    version = "v1",
    kind = "Project",
    plural = "projects",
    derive = "PartialEq",
    namespaced
)]

pub struct ProjectSpec {
    pub projectId: String,
    pub environmentType: String,
    pub googleGroup: String,
}
