use kube::CustomResource;
use lazy_static::lazy_static;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

lazy_static! {
    pub static ref RE_ENV_TYPE: regex::Regex =
        regex::Regex::new(r"^(dev|qa|test|stage|prod)$").unwrap();
}

#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Validate)]
#[kube(
    group = "kyotu.tech",
    version = "v1",
    kind = "Project",
    plural = "projects",
    derive = "PartialEq",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSpec {
    pub project_id: String,
    #[validate(regex = "RE_ENV_TYPE")]
    pub environment_type: String,
    pub google_group: String,
}
