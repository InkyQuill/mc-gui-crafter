use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnimationType {
    #[serde(alias = "Fill")]
    Fill,
    #[serde(alias = "Cycle")]
    Cycle,
    #[serde(alias = "Pulse")]
    Pulse,
    #[serde(alias = "Toggle")]
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Animation {
    pub id: String,
    #[serde(rename = "type")]
    pub animation_type: AnimationType,
    pub data_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<crate::project::FillDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggers_on: Option<String>,
}
