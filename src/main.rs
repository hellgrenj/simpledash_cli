mod cli;
mod client;
mod models;

use cli::clear_screen;
use cli_table::{format::Justify, Cell, Style, Table};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use models::{ClusterInfo, Payload};

fn main() {
    ctrlc::set_handler(move || {
        cli::clean_exit();
    })
    .expect("Error setting Ctrl-C handler");
    clear_screen();
    println!("{}", "..::simpledash CLI::..".green().on_black().bold());
    let host = cli::parse_args();
    let mut socket = client::connect_to_host(&host).expect("Error connecting to host");
    let cluster_info = client::get_cluster_info(&host).expect("Failed to fetch Simpledash Context");
    let ns = select_namespace(&cluster_info);
    loop {
        if !socket.can_read() {
            println!("Socket is not readable");
            println!("trying to reconnect");
            socket = client::connect_to_host(&host).expect("Error re-connecting to host");
        }
        let read_result = socket.read();

        let msg = match read_result {
            Ok(msg) => msg,
            Err(e) => {
                println!("Error reading message: {:?}", e);
                continue;
            }
        };
        if msg.len() <= 0 {
            continue; // a ping that tungstenite will reply to with a pong automatically
        }
        clear_screen();
        let payload = match serde_json::from_str(&msg.to_string()) {
            Ok(payload) => payload,
            Err(e) => {
                println!("Error parsing payload: {:?}", e);
                continue;
            }
        };
        visualize_payload(payload, &ns, &cluster_info);
    }
}

fn select_namespace(cluster_info: &ClusterInfo) -> String {
    let namespaces = &cluster_info.namespaces;
    let selections = &namespaces[..];
    println!("");
    println!("select namespace:({})", selections.len());
    println!("");
    let selection_result = Select::with_theme(&ColorfulTheme::default())
        .default(0)
        .items(&selections[..])
        .interact();

    let selection = match selection_result {
        Ok(selection) => selection,
        Err(e) => {
            eprintln!("Error selecting namespace: {:?}", e);
            return "default_namespace".to_string();
        }
    };
    namespaces[selection].to_string()
}

fn visualize_payload(payload: Payload, namespace: &str, cluster_info: &ClusterInfo) {
    print_endpoints(&payload, namespace);
    print_deployments(&payload, namespace, cluster_info);
    print_pods_table(&payload, namespace, cluster_info);
}

fn print_endpoints(payload: &Payload, namespace: &str) {
    println!("{}", "Endpoints:".magenta().bold());
    println!("{}", get_endpoints_visualization(payload, namespace));
}
fn get_endpoints_visualization(payload: &Payload, namespace: &str) -> String {
    let mut result = String::new();
    if let Some(ingresses) = &payload.ingresses {
        for ingress in ingresses {
            if ingress.namespace != *namespace {
                continue;
            }
            result.push_str(&format!(
                "{}{} ({})\n",
                "https://".bold().blue(),
                ingress.endpoint.bold().blue(),
                ingress.ip
            ));
        }
    }
    result
}
fn print_deployments(payload: &Payload, namespace: &str, cluster_info: &ClusterInfo) {
    println!("{}", "Deployments:".magenta().bold());
    println!(
        "{}",
        get_deployments_visualization(payload, namespace, cluster_info)
    );
}
fn get_deployments_visualization(
    payload: &Payload,
    namespace: &str,
    cluster_info: &ClusterInfo,
) -> String {
    let mut result = String::new();
    for deployment in payload.deployments.iter() {
        if deployment.namespace != *namespace {
            continue;
        }
        if cluster_info.deployment_logs_link_enabled {
            let link_url = cluster_info
                .deployment_logs_link
                .replace("DEPLOYMENT_NAME_PLACEHOLDER", &deployment.name)
                .replace("DEPLOYMENT_NAMESPACE_PLACEHOLDER", &deployment.namespace);
            result.push_str(&format!(
                "{} ({}/{}) {}\n",
                deployment.name,
                deployment.ready_replicas,
                deployment.replicas,
                cli::make_link(link_url, "view logs".to_string())
                    .bold()
                    .blue()
            ));
        } else {
            result.push_str(&format!(
                "{} ({}/{})\n",
                deployment.name, deployment.ready_replicas, deployment.replicas
            ));
        }
    }
    result
}
fn print_pods_table(payload: &Payload, namespace: &str, cluster_info: &ClusterInfo) {
    println!("{}", "Pods:".magenta().bold());
    println!("{}", get_pods_visualization(payload, namespace));
    println!(
        "{} in {} as per {}",
        namespace.bold().magenta().on_black(),
        cluster_info.cluster_name.bold().magenta().on_black(),
        payload.timestamp.bold().yellow()
    );
}
fn get_pods_visualization(payload: &Payload, namespace: &str) -> String {
    let mut pod_rows = vec![vec![
        "node".cell().bold(true),
        "pod name".cell().bold(true),
        "status".cell().bold(true),
        "tag".cell().bold(true),
    ]];
    for (key, value) in payload.nodes.iter() {
        for pod in value.iter() {
            if pod.namespace != *namespace {
                continue;
            }

            let colored_status: ColoredString;
            if pod.status == "Running" || pod.status == "Succeeded" || pod.status == "Completed" {
                colored_status = pod.status.green();
            } else if pod.status == "Pending"
                || pod.status == "ContainerCreating"
                || pod.status == "PodInitializing"
            {
                colored_status = pod.status.yellow();
            } else {
                colored_status = pod.status.red();
            }

            let pod_image_tag = match pod.image.split(":").last() {
                Some(tag) => tag,
                None => "unknown",
            };

            pod_rows.push(vec![
                key.cell(),
                pod.name.clone().cell(),
                colored_status.cell().justify(Justify::Right),
                pod_image_tag.cell(),
            ]);
        }
    }
    let pod_table = pod_rows.table().bold(true);
    let pod_table_display = match pod_table.display() {
        Ok(display) => display,
        Err(e) => {
            eprintln!("Error displaying pod table: {:?}", e);
            return "could not visualize pods".to_string();
        }
    };
    pod_table_display.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{ClusterInfo, Deployment};

    #[test]
    fn test_get_deployments_visualization_visualizing_only_selected_namespace() {
        // Arrange
        let payload = Payload {
            deployments: vec![
                Deployment {
                    name: "deployment1".to_string(),
                    namespace: "namespace1".to_string(),
                    ready_replicas: 2,
                    replicas: 3,
                    ..Default::default()
                },
                Deployment {
                    name: "deployment2".to_string(),
                    namespace: "namespace2".to_string(),
                    ready_replicas: 1,
                    replicas: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let cluster_info = ClusterInfo {
            deployment_logs_link_enabled: false,
            ..Default::default()
        };

        // Act
        let visualization = get_deployments_visualization(&payload, "namespace1", &cluster_info);

        // Assert
        // contains this
        assert!(visualization.contains("deployment1 (2/3)"));
        // and not this...
        assert!(!visualization.contains("deployment2 (1/1)"));
    }
}
