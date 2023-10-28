use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct ClusterInfo {
    #[serde(rename = "ClusterName")]
    pub cluster_name: String,
    #[serde(rename = "Namespaces")]
    pub namespaces: Vec<String>,
    #[serde(rename = "DeploymentLogsLinkEnabled")]
    pub deployment_logs_link_enabled: bool,
    #[serde(rename = "DeploymentLogsLink")]
    pub deployment_logs_link: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Payload {
    #[serde(rename = "Nodes")]
    pub nodes: std::collections::HashMap<String, Vec<Pods>>,
    #[serde(rename = "Ingresses")]
    pub ingresses: Option<Vec<Ingress>>,
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    #[serde(rename = "Deployments")]
    pub deployments: Vec<Deployment>,
}

#[derive(Debug, Deserialize)]
pub struct Pods {
    #[serde(rename = "Namespace")]
    pub namespace: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct Ingress {
    #[serde(rename = "Endpoint")]
    pub endpoint: String,
    #[serde(rename = "Ip")]
    pub ip: String,
    #[serde(rename = "Namespace")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Deployment {
    #[serde(rename = "Namespace")]
    pub namespace: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Replicas")]
    pub replicas: u32,
    #[serde(rename = "ReadyReplicas")]
    pub ready_replicas: u32,
}
